//! Shared HTTP plumbing: the client, retry with backoff, and error mapping.

use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::RequestBuilder;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::provider::AiError;
use crate::secrets::{scrub, Secret};

/// Retry policy for 429, 5xx and connection failures.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    /// Retries after the first attempt (the spec says at most 2).
    pub max_retries: u32,
    /// First backoff; doubles each retry.
    pub base_delay: Duration,
    /// Upper bound for any single wait, including `retry-after`.
    pub max_delay: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl Retry {
    /// No waiting, for tests.
    pub fn instant() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::ZERO,
            max_delay: Duration::from_millis(50),
        }
    }

    fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        let backoff = self.base_delay.saturating_mul(2u32.saturating_pow(attempt));
        retry_after.unwrap_or(backoff).min(self.max_delay)
    }
}

/// The shared client. Streams may idle for a while while a model thinks, so only connect and
/// per-read timeouts are set, not a total timeout.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(180))
        .user_agent(concat!("MergeIQ/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// `base` + `path`, tolerating a trailing slash and a base that already ends with `suffix`.
pub fn join_url(base: &str, suffix_in_base: &str, path: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let base = base.strip_suffix(suffix_in_base).unwrap_or(base);
    format!("{base}{path}")
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let secs = headers
        .get("retry-after")?
        .to_str()
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()?;
    (secs.is_finite() && secs >= 0.0).then(|| Duration::from_secs_f64(secs))
}

/// The provider's own message out of an error body.
pub fn error_message(body: &str) -> String {
    let from_json = serde_json::from_str::<Value>(body).ok().and_then(|v| {
        v.pointer("/error/message")
            .or_else(|| v.get("message"))
            .or_else(|| v.get("error").filter(|e| e.is_string()))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let text = from_json.unwrap_or_else(|| body.trim().to_string());
    let mut text: String = text.chars().take(300).collect();
    if text.is_empty() {
        text = "no details".to_string();
    }
    text
}

/// Maps an unsuccessful response to an [`AiError`].
pub fn map_status(
    status: u16,
    retry_after: Option<Duration>,
    body: &str,
    secrets: &[&Secret],
) -> AiError {
    let message = scrub(&error_message(body), secrets);
    match status {
        401 | 403 => AiError::Auth(message),
        429 => AiError::RateLimited { retry_after },
        _ => AiError::Http { status, message },
    }
}

async fn sleep_or_cancel(d: Duration, cancel: &CancellationToken) -> Result<(), AiError> {
    tokio::select! {
        () = cancel.cancelled() => Err(AiError::Cancelled),
        () = tokio::time::sleep(d) => Ok(()),
    }
}

/// Sends the request built by `make`, retrying 429, 5xx and connection errors with exponential
/// backoff (honoring `retry-after`). Returns the first successful response.
pub async fn send(
    retry: &Retry,
    cancel: &CancellationToken,
    secrets: &[&Secret],
    make: impl Fn() -> RequestBuilder,
) -> Result<reqwest::Response, AiError> {
    let mut attempt = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        let sent = tokio::select! {
            () = cancel.cancelled() => return Err(AiError::Cancelled),
            r = make().send() => r,
        };
        match sent {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) => {
                let status = resp.status().as_u16();
                let retry_after = parse_retry_after(resp.headers());
                let body = resp.text().await.unwrap_or_default();
                if (status == 429 || status >= 500) && attempt < retry.max_retries {
                    tracing::debug!(status, attempt, "retrying AI request");
                    sleep_or_cancel(retry.delay(attempt, retry_after), cancel).await?;
                    attempt += 1;
                    continue;
                }
                return Err(map_status(status, retry_after, &body, secrets));
            }
            Err(e) => {
                let transient = e.is_connect() || e.is_timeout() || e.is_request();
                if transient && attempt < retry.max_retries {
                    tracing::debug!(attempt, "retrying AI request after a connection error");
                    sleep_or_cancel(retry.delay(attempt, None), cancel).await?;
                    attempt += 1;
                    continue;
                }
                return Err(AiError::Network(scrub(
                    &e.without_url().to_string(),
                    secrets,
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_join_cleanly() {
        assert_eq!(
            join_url("https://api.anthropic.com", "/v1", "/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            join_url("https://api.anthropic.com/", "/v1", "/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            join_url("https://proxy.example/anthropic/v1/", "/v1", "/v1/messages"),
            "https://proxy.example/anthropic/v1/messages"
        );
    }

    #[test]
    fn messages_come_from_the_usual_places() {
        assert_eq!(
            error_message(r#"{"type":"error","error":{"type":"x","message":"bad model"}}"#),
            "bad model"
        );
        assert_eq!(
            error_message(r#"{"error":"model 'x' not found"}"#),
            "model 'x' not found"
        );
        assert_eq!(
            error_message(r#"{"message":"Unauthorized"}"#),
            "Unauthorized"
        );
        assert_eq!(
            error_message("<html>gateway</html>"),
            "<html>gateway</html>"
        );
        assert_eq!(error_message(""), "no details");
        assert_eq!(error_message(&"x".repeat(1000)).len(), 300);
    }

    #[test]
    fn statuses_map_and_secrets_are_scrubbed() {
        let key = Secret::new("sk-secret-value-1234");
        let e = map_status(
            401,
            None,
            r#"{"error":{"message":"invalid key sk-secret-value-1234"}}"#,
            &[&key],
        );
        assert_eq!(e, AiError::Auth("invalid key [redacted]".into()));
        assert!(
            matches!(map_status(429, Some(Duration::from_secs(3)), "", &[]), AiError::RateLimited { retry_after: Some(d) } if d.as_secs() == 3)
        );
        assert!(matches!(
            map_status(400, None, "{}", &[]),
            AiError::Http { status: 400, .. }
        ));
    }

    #[test]
    fn backoff_doubles_honors_retry_after_and_is_capped() {
        let r = Retry::default();
        assert_eq!(r.delay(0, None), Duration::from_millis(500));
        assert_eq!(r.delay(1, None), Duration::from_millis(1000));
        assert_eq!(
            r.delay(0, Some(Duration::from_secs(4))),
            Duration::from_secs(4)
        );
        assert_eq!(
            r.delay(0, Some(Duration::from_secs(900))),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn retry_after_parses_seconds() {
        let mut h = HeaderMap::new();
        h.insert("retry-after", "7".parse().unwrap());
        assert_eq!(parse_retry_after(&h), Some(Duration::from_secs(7)));
        h.insert(
            "retry-after",
            "Wed, 21 Oct 2026 07:28:00 GMT".parse().unwrap(),
        );
        assert_eq!(parse_retry_after(&h), None);
    }
}
