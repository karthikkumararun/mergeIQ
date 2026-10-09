//! Settings, the checks that run before any request, and provider construction.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::context::ContextSettings;
use crate::privacy::{Consent, PrivacySettings};
use crate::provider::{AiError, AiProvider, Effort, ProviderConfig, ProviderKind};
use crate::secrets::Secret;
use crate::usage::PriceTable;

/// Per-provider connection settings (never credentials).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    /// Base URL.
    pub base_url: String,
    /// Model id.
    pub model: String,
    /// Effort, where the provider has one.
    pub effort: Effort,
}

impl ProviderSettings {
    /// The defaults for `kind`.
    pub fn defaults(kind: ProviderKind) -> Self {
        Self {
            base_url: kind.default_base_url().to_string(),
            model: kind.default_model().to_string(),
            effort: Effort::High,
        }
    }
}

/// Everything the user configures for AI. Stored in `settings.json`; contains no key material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    /// The active provider.
    pub provider: ProviderKind,
    /// The user has saved a provider choice (so Ollama, which needs no key, counts as set up).
    pub confirmed: bool,
    /// The one-time data-sharing notice was accepted.
    pub notice_accepted: bool,
    /// Provider id to its settings; missing entries use defaults.
    pub providers: BTreeMap<String, ProviderSettings>,
    /// Exclusion globs and per-repository consent.
    pub privacy: PrivacySettings,
    /// Context limits.
    pub context: ContextSettings,
    /// Editable price table.
    pub prices: PriceTable,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Anthropic,
            confirmed: false,
            notice_accepted: false,
            providers: BTreeMap::new(),
            privacy: PrivacySettings::default(),
            context: ContextSettings::default(),
            prices: PriceTable::default(),
        }
    }
}

impl AiSettings {
    /// Settings of `kind`, falling back to its defaults.
    pub fn for_provider(&self, kind: ProviderKind) -> ProviderSettings {
        self.providers
            .get(kind.id())
            .cloned()
            .unwrap_or_else(|| ProviderSettings::defaults(kind))
    }

    /// Settings of the active provider.
    pub fn active(&self) -> ProviderSettings {
        self.for_provider(self.provider)
    }

    /// Whether the active provider is usable: chosen, and holding a key if it needs one.
    pub fn is_configured(&self, key_present: bool) -> bool {
        self.confirmed && (!self.provider.needs_key() || key_present)
    }

    /// Runs the checks that precede every request, in the order the UI resolves them.
    pub fn gate(&self, key_present: bool, repo: &str, path: &str) -> Result<(), Gate> {
        if !self.is_configured(key_present) {
            return Err(Gate::NotConfigured);
        }
        if !self.notice_accepted {
            return Err(Gate::NoticeRequired);
        }
        match self.privacy.consent(repo) {
            Consent::Unasked => return Err(Gate::RepoUnasked),
            Consent::Declined => return Err(Gate::RepoDeclined),
            Consent::Allowed => {}
        }
        if self.privacy.check_path(path).is_err() {
            return Err(Gate::Excluded);
        }
        Ok(())
    }
}

/// Why a request was not even attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// No provider or key.
    NotConfigured,
    /// The data-sharing notice has not been accepted.
    NoticeRequired,
    /// The repository has not been asked about yet.
    RepoUnasked,
    /// The user declined AI for this repository.
    RepoDeclined,
    /// The path matches an exclusion glob.
    Excluded,
}

/// The error type the UI receives from AI commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AiFailure {
    /// No provider or key is set up.
    #[error("AI is not set up")]
    NotConfigured,
    /// The data-sharing notice must be accepted first.
    #[error("Accept the data-sharing notice to use AI")]
    NoticeRequired,
    /// The repository needs an opt-in decision.
    #[error("AI is not enabled for this repository yet")]
    RepoUnasked,
    /// AI is disabled for this repository.
    #[error("AI is turned off for this repository")]
    RepoDeclined,
    /// The file matches an exclusion glob.
    #[error("File excluded from AI by your settings")]
    Excluded,
    /// The provider rejected the credentials.
    #[error("{message}")]
    Auth {
        /// The provider's message.
        message: String,
    },
    /// Too many requests.
    #[error("rate limited")]
    RateLimited {
        /// Seconds the provider asked us to wait.
        retry_after_secs: Option<u32>,
    },
    /// The model declined.
    #[error("The model declined this request")]
    Refused {
        /// Refusal category.
        category: Option<String>,
        /// Explanation, when given.
        explanation: Option<String>,
    },
    /// The response hit the output limit.
    #[error("The response was cut off at the output limit")]
    Truncated,
    /// The model's answer did not match the schema, even after a retry.
    #[error("{message}")]
    Schema {
        /// What was wrong.
        message: String,
    },
    /// The provider could not be reached.
    #[error("{message}")]
    Network {
        /// What failed.
        message: String,
    },
    /// An error response.
    #[error("{message}")]
    Http {
        /// Status code.
        status: u16,
        /// The provider's message.
        message: String,
    },
    /// The user cancelled.
    #[error("cancelled")]
    Cancelled,
    /// Something local failed.
    #[error("{message}")]
    Internal {
        /// What failed.
        message: String,
    },
}

impl From<Gate> for AiFailure {
    fn from(g: Gate) -> Self {
        match g {
            Gate::NotConfigured => AiFailure::NotConfigured,
            Gate::NoticeRequired => AiFailure::NoticeRequired,
            Gate::RepoUnasked => AiFailure::RepoUnasked,
            Gate::RepoDeclined => AiFailure::RepoDeclined,
            Gate::Excluded => AiFailure::Excluded,
        }
    }
}

impl From<AiError> for AiFailure {
    fn from(e: AiError) -> Self {
        match e {
            AiError::NotConfigured(_) => AiFailure::NotConfigured,
            AiError::Auth(message) => AiFailure::Auth { message },
            AiError::RateLimited { retry_after } => AiFailure::RateLimited {
                retry_after_secs: retry_after
                    .map(|d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX)),
            },
            AiError::Refused {
                category,
                explanation,
            } => AiFailure::Refused {
                category,
                explanation,
            },
            AiError::Truncated => AiFailure::Truncated,
            AiError::Schema(message) => AiFailure::Schema { message },
            AiError::Network(message) => AiFailure::Network { message },
            AiError::Http { status, message } => AiFailure::Http { status, message },
            AiError::Cancelled => AiFailure::Cancelled,
        }
    }
}

/// Builds the provider for `kind`. `key` is ignored by providers that do not use one.
pub fn build_provider(
    kind: ProviderKind,
    settings: &ProviderSettings,
    key: Option<Secret>,
) -> Arc<dyn AiProvider> {
    let config = ProviderConfig {
        kind,
        base_url: settings.base_url.clone(),
        model: settings.model.clone(),
        effort: settings.effort,
        api_key: key,
    };
    match kind {
        ProviderKind::Anthropic => Arc::new(crate::anthropic::AnthropicProvider::new(config)),
        ProviderKind::Openai => Arc::new(crate::openai::ChatProvider::new(
            config,
            crate::openai::Flavor::Openai,
        )),
        ProviderKind::GithubModels => Arc::new(crate::github::provider(config)),
        ProviderKind::Ollama => Arc::new(crate::ollama::OllamaProvider::new(config)),
        ProviderKind::Mock => Arc::new(crate::mock::MockProvider::new(Vec::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy::RepoDecision;

    fn ready() -> AiSettings {
        let mut s = AiSettings {
            confirmed: true,
            notice_accepted: true,
            ..AiSettings::default()
        };
        s.privacy.repos.insert("/r".into(), RepoDecision::Allowed);
        s
    }

    #[test]
    fn the_default_is_anthropic_and_disabled() {
        let s = AiSettings::default();
        assert_eq!(s.provider, ProviderKind::Anthropic);
        assert_eq!(s.active().model, "claude-opus-5");
        assert_eq!(s.gate(true, "/r", "a.rs"), Err(Gate::NotConfigured));
    }

    #[test]
    fn gates_resolve_in_order() {
        let mut s = ready();
        assert_eq!(
            s.gate(false, "/r", "a.rs"),
            Err(Gate::NotConfigured),
            "key missing"
        );
        assert_eq!(s.gate(true, "/r", "a.rs"), Ok(()));
        s.notice_accepted = false;
        assert_eq!(s.gate(true, "/r", "a.rs"), Err(Gate::NoticeRequired));
        s.notice_accepted = true;
        assert_eq!(s.gate(true, "/other", "a.rs"), Err(Gate::RepoUnasked));
        s.privacy.repos.insert("/r".into(), RepoDecision::Declined);
        assert_eq!(s.gate(true, "/r", "a.rs"), Err(Gate::RepoDeclined));
        s.privacy.repos.insert("/r".into(), RepoDecision::Allowed);
        assert_eq!(
            s.gate(true, "/r", "config/.env.production"),
            Err(Gate::Excluded)
        );
    }

    #[test]
    fn ollama_needs_no_key_but_must_be_confirmed() {
        let mut s = ready();
        s.provider = ProviderKind::Ollama;
        assert_eq!(s.gate(false, "/r", "a.rs"), Ok(()));
        s.confirmed = false;
        assert_eq!(s.gate(false, "/r", "a.rs"), Err(Gate::NotConfigured));
    }

    #[test]
    fn provider_settings_fall_back_to_defaults_and_serialize_without_keys() {
        let mut s = AiSettings::default();
        assert_eq!(
            s.for_provider(ProviderKind::Ollama).base_url,
            "http://localhost:11434"
        );
        s.providers.insert(
            "openai".into(),
            ProviderSettings {
                model: "gpt-5".into(),
                ..ProviderSettings::defaults(ProviderKind::Openai)
            },
        );
        assert_eq!(s.for_provider(ProviderKind::Openai).model, "gpt-5");
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.to_lowercase().contains("apikey") && !json.contains("api_key"));
        let back: AiSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // Old or partial files still load.
        let partial: AiSettings = serde_json::from_str(r#"{"provider":"ollama"}"#).unwrap();
        assert_eq!(partial.provider, ProviderKind::Ollama);
        assert_eq!(partial.context.token_budget, 60_000);
    }

    #[test]
    fn failures_serialize_with_a_code() {
        let v = serde_json::to_value(AiFailure::from(AiError::RateLimited {
            retry_after: Some(std::time::Duration::from_secs(7)),
        }))
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"code": "rateLimited", "retryAfterSecs": 7})
        );
        assert_eq!(
            serde_json::to_value(AiFailure::from(Gate::Excluded)).unwrap(),
            serde_json::json!({"code": "excluded"})
        );
        assert_eq!(
            AiFailure::from(Gate::Excluded).to_string(),
            "File excluded from AI by your settings"
        );
    }
}
