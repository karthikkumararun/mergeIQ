//! What may be sent: path exclusion globs and per-repository consent.

use std::collections::BTreeMap;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::provider::AiError;

/// Files that never leave the machine unless the user removes the pattern.
pub const DEFAULT_EXCLUDES: [&str; 4] = ["**/.env*", "**/*secret*", "**/*.pem", "**/*.key"];

/// The user's decision for one repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum RepoDecision {
    /// AI actions may send this repository's code.
    Allowed,
    /// The user declined; no request is sent.
    Declined,
}

/// Privacy settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct PrivacySettings {
    /// Glob patterns of paths never sent.
    pub exclude_globs: Vec<String>,
    /// Repository root (display form) to decision.
    pub repos: BTreeMap<String, RepoDecision>,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            exclude_globs: DEFAULT_EXCLUDES.iter().map(|s| (*s).to_string()).collect(),
            repos: BTreeMap::new(),
        }
    }
}

/// What consent a repository has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// The user has not been asked yet.
    Unasked,
    /// Allowed.
    Allowed,
    /// Declined.
    Declined,
}

impl PrivacySettings {
    /// Compiles the exclusion patterns. Invalid patterns are returned so the UI can show them.
    pub fn matcher(&self) -> (GlobSet, Vec<String>) {
        let mut builder = GlobSetBuilder::new();
        let mut invalid = Vec::new();
        for pattern in &self.exclude_globs {
            match globset::GlobBuilder::new(pattern)
                .case_insensitive(true)
                .build()
            {
                Ok(g) => {
                    builder.add(g);
                }
                Err(_) => invalid.push(pattern.clone()),
            }
        }
        (
            builder.build().unwrap_or_else(|_| GlobSet::empty()),
            invalid,
        )
    }

    /// Refuses excluded paths with the message the spec requires.
    pub fn check_path(&self, path: &str) -> Result<(), AiError> {
        let (set, _) = self.matcher();
        let normalized = path.replace('\\', "/");
        if set.is_match(&normalized) {
            Err(AiError::NotConfigured(
                "File excluded from AI by your settings".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// The consent recorded for `repo`.
    pub fn consent(&self, repo: &str) -> Consent {
        match self.repos.get(repo) {
            Some(RepoDecision::Allowed) => Consent::Allowed,
            Some(RepoDecision::Declined) => Consent::Declined,
            None => Consent::Unasked,
        }
    }
}

/// Whether `pattern` is a valid glob.
pub fn is_valid_glob(pattern: &str) -> bool {
    Glob::new(pattern).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_exclude_secrets_and_keys() {
        let p = PrivacySettings::default();
        for path in [
            "config/.env.production",
            ".env",
            "a/b/.env.local",
            "src/my_secret_token.rs",
            "deploy/Secrets.yaml",
            "certs/server.pem",
            "certs/SERVER.PEM",
            "keys/id_rsa.key",
        ] {
            let err = p.check_path(path).unwrap_err();
            assert_eq!(
                err.to_string(),
                "File excluded from AI by your settings",
                "{path}"
            );
        }
        for path in [
            "src/main.rs",
            "README.md",
            "docs/environment.md",
            "src/keyboard.ts",
        ] {
            assert!(p.check_path(path).is_ok(), "{path}");
        }
    }

    #[test]
    fn windows_separators_are_normalised() {
        let p = PrivacySettings::default();
        assert!(p.check_path("config\\.env.production").is_err());
    }

    #[test]
    fn custom_patterns_apply_and_bad_ones_are_reported() {
        let p = PrivacySettings {
            exclude_globs: vec!["vendor/**".into(), "[".into()],
            ..PrivacySettings::default()
        };
        assert!(p.check_path("vendor/lib/a.c").is_err());
        assert!(p.check_path("src/a.c").is_ok());
        let (_, invalid) = p.matcher();
        assert_eq!(invalid, vec!["[".to_string()]);
        assert!(!is_valid_glob("["));
        assert!(is_valid_glob("**/*.pem"));
    }

    #[test]
    fn repo_consent_defaults_to_unasked() {
        let mut p = PrivacySettings::default();
        assert_eq!(p.consent("/code/a"), Consent::Unasked);
        p.repos.insert("/code/a".into(), RepoDecision::Declined);
        p.repos.insert("/code/b".into(), RepoDecision::Allowed);
        assert_eq!(p.consent("/code/a"), Consent::Declined);
        assert_eq!(p.consent("/code/b"), Consent::Allowed);
    }
}
