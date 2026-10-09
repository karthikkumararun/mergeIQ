//! Credentials live only in the OS credential store; this module never writes them anywhere
//! else and never lets them reach a log.

use std::collections::HashMap;
use std::sync::Mutex;

/// The credential-store service name.
pub const SERVICE: &str = "dev.mergeiq.app";

/// A credential. `Debug` and `Display` never show the value.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a credential.
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// The raw value, for building a request header. Never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The last four characters, the only part the UI shows.
    pub fn last4(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        chars[chars.len().saturating_sub(4)..].iter().collect()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

/// A credential-store failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("credential store: {0}")]
pub struct SecretError(pub String);

/// A place credentials can be kept.
pub trait SecretStore: Send + Sync {
    /// The credential for `account`, if there is one.
    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError>;
    /// Stores (replaces) the credential for `account`.
    fn set(&self, account: &str, value: &str) -> Result<(), SecretError>;
    /// Removes the credential for `account` (not an error if absent).
    fn delete(&self, account: &str) -> Result<(), SecretError>;
    /// Human-readable name of the store, e.g. "macOS Keychain".
    fn name(&self) -> &'static str;
}

/// The operating system's credential store (macOS Keychain, Windows Credential Manager,
/// Secret Service on Linux).
#[derive(Debug, Default, Clone, Copy)]
pub struct OsStore;

impl OsStore {
    fn entry(account: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, account).map_err(|e| SecretError(e.to_string()))
    }
}

impl SecretStore for OsStore {
    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError> {
        match Self::entry(account)?.get_password() {
            Ok(v) => Ok(Some(Secret::new(v))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn set(&self, account: &str, value: &str) -> Result<(), SecretError> {
        Self::entry(account)?
            .set_password(value)
            .map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn name(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macOS Keychain"
        } else if cfg!(windows) {
            "Windows Credential Manager"
        } else {
            "system keyring"
        }
    }
}

/// An in-memory store for tests.
#[derive(Debug, Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| SecretError("poisoned".into()))?
            .get(account)
            .cloned()
            .map(Secret::new))
    }

    fn set(&self, account: &str, value: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|_| SecretError("poisoned".into()))?
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|_| SecretError("poisoned".into()))?
            .remove(account);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "memory"
    }
}

/// Headers whose values must never be logged.
const SENSITIVE_HEADERS: [&str; 5] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "x-goog-api-key",
];

/// Whether `name` carries credentials.
pub fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS
        .iter()
        .any(|h| name.eq_ignore_ascii_case(h))
}

/// A header as it may be logged: credential values are replaced.
pub fn loggable_header(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        format!("{name}: [redacted]")
    } else {
        format!("{name}: {value}")
    }
}

/// Removes every occurrence of the given credentials from `text`.
pub fn scrub(text: &str, secrets: &[&Secret]) -> String {
    let mut out = text.to_string();
    for s in secrets {
        let raw = s.expose();
        if raw.len() >= 4 {
            out = out.replace(raw, "[redacted]");
        }
    }
    out
}

/// Replaces credentials in a log line: header values (`x-api-key: …`, `Authorization: Bearer …`
/// in any of the usual debug renderings) and anything shaped like a provider key or token.
pub fn redact(text: &str) -> std::borrow::Cow<'_, str> {
    use std::sync::OnceLock;
    static HEADERS: OnceLock<regex::Regex> = OnceLock::new();
    static TOKENS: OnceLock<regex::Regex> = OnceLock::new();
    let headers = HEADERS.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)\b(authorization|proxy-authorization|x-api-key|api-key|x-goog-api-key)\b(["']?\s*[:=]\s*["']?)(?:(?:bearer|basic|token)\s+)?[^\s"',;}\]]+"#,
        )
        .expect("valid header pattern")
    });
    let tokens = TOKENS.get_or_init(|| {
        regex::Regex::new(
            r"sk-ant-[A-Za-z0-9_\-]{10,}|sk-[A-Za-z0-9_\-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}",
        )
        .expect("valid token pattern")
    });
    let first = headers.replace_all(text, "${1}${2}[redacted]");
    match tokens.replace_all(&first, "[redacted]") {
        std::borrow::Cow::Borrowed(_) => first,
        std::borrow::Cow::Owned(owned) => std::borrow::Cow::Owned(owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_never_show_the_value() {
        let s = Secret::new("sk-ant-api03-supersecret-a9F2");
        assert!(!format!("{s:?}").contains("supersecret"));
        assert!(!format!("{s}").contains("supersecret"));
        assert_eq!(s.last4(), "a9F2");
        assert_eq!(Secret::new("ab").last4(), "ab");
        assert_eq!(Secret::new("").last4(), "");
        assert_eq!(
            Secret::new("ключ-π-ключ").last4(),
            "ключ".chars().collect::<String>()
        );
    }

    #[test]
    fn the_memory_store_round_trips() {
        let store = MemoryStore::default();
        assert_eq!(store.get("anthropic").unwrap(), None);
        store.set("anthropic", "k1").unwrap();
        assert_eq!(store.get("anthropic").unwrap().unwrap().expose(), "k1");
        store.set("anthropic", "k2").unwrap();
        assert_eq!(store.get("anthropic").unwrap().unwrap().expose(), "k2");
        store.delete("anthropic").unwrap();
        store.delete("anthropic").unwrap();
        assert_eq!(store.get("anthropic").unwrap(), None);
    }

    #[test]
    fn credential_headers_are_redacted_for_logs() {
        assert_eq!(
            loggable_header("x-api-key", "sk-1234"),
            "x-api-key: [redacted]"
        );
        assert_eq!(
            loggable_header("Authorization", "Bearer abc"),
            "Authorization: [redacted]"
        );
        assert_eq!(
            loggable_header("anthropic-version", "2023-06-01"),
            "anthropic-version: 2023-06-01"
        );
        assert!(is_sensitive_header("X-API-KEY"));
    }

    #[test]
    fn scrub_removes_keys_from_arbitrary_text() {
        let key = Secret::new("sk-ant-secret-value-1234");
        let text =
            "request failed: key sk-ant-secret-value-1234 rejected (sk-ant-secret-value-1234)";
        let clean = scrub(text, &[&key]);
        assert!(!clean.contains("sk-ant-secret"));
        assert_eq!(clean.matches("[redacted]").count(), 2);
        // Very short values are not scrubbed (they would mangle ordinary text).
        assert_eq!(scrub("abc", &[&Secret::new("abc")]), "abc");
    }

    #[test]
    fn the_service_name_is_fixed() {
        assert_eq!(SERVICE, "dev.mergeiq.app");
    }

    /// Touches the real credential store, so it only runs on request:
    /// `cargo test -p mergeiq-ai os_store -- --ignored`.
    #[test]
    #[ignore = "uses the operating system's credential store"]
    fn os_store_round_trips_a_secret_under_the_fixed_service() {
        let store = OsStore;
        let account = format!("mergeiq-test-{}", std::process::id());
        assert_eq!(store.get(&account).unwrap(), None);
        store.set(&account, "first-value-1234").unwrap();
        assert_eq!(
            store.get(&account).unwrap().unwrap().expose(),
            "first-value-1234"
        );
        store.set(&account, "second-value-5678").unwrap();
        assert_eq!(
            store.get(&account).unwrap().unwrap().expose(),
            "second-value-5678"
        );
        store.delete(&account).unwrap();
        store.delete(&account).unwrap();
        assert_eq!(store.get(&account).unwrap(), None);
    }

    #[test]
    fn log_lines_are_redacted_in_every_header_rendering() {
        let cases = [
            (
                "x-api-key: sk-ant-api03-abcdef123456",
                "x-api-key: [redacted]",
            ),
            ("X-Api-Key: plainlookingvalue", "X-Api-Key: [redacted]"),
            (
                "Authorization: Bearer ghp_abcdefghijklmnopqrstuvwxyz0123",
                "Authorization: [redacted]",
            ),
            (
                r#"{"authorization": "Bearer abc.def.ghi"}"#,
                r#"{"authorization": "[redacted]"}"#,
            ),
            (
                "headers={authorization=Bearer secretvalue, accept=json}",
                "headers={authorization=[redacted], accept=json}",
            ),
            (
                "proxy-authorization: Basic dXNlcjpwYXNz",
                "proxy-authorization: [redacted]",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(redact(input), expected, "{input}");
        }
    }

    #[test]
    fn keys_are_redacted_wherever_they_appear() {
        let line = "request failed for sk-ant-api03-abcdefghijklmnop at url, retrying with github_pat_11ABCDEFG0123456789_abcdefghij";
        let out = redact(line);
        assert!(!out.contains("abcdefghijklmnop") && !out.contains("github_pat_11"));
        assert_eq!(out.matches("[redacted]").count(), 2);
    }

    #[test]
    fn ordinary_lines_are_untouched() {
        for line in [
            "AI request finished provider=anthropic model=claude-opus-5 input=6200 output=410",
            "retrying AI request status=529 attempt=1",
            "the authorization flow is described in the docs",
            "sk-short",
        ] {
            assert_eq!(redact(line), line);
        }
    }
}
