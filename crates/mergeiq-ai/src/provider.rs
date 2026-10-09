//! The provider abstraction: configuration, errors and the [`AiProvider`] trait.
//!
//! A provider only has to implement [`AiProvider::stream_text`] (and `test`); `explain` and
//! `suggest` are provided on top of it, including schema validation of structured output and
//! the single retry the spec requires for responses that fail it.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::schema::{self, Suggestion};
use crate::secrets::Secret;
use crate::usage::Usage;

/// The supported providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    /// Anthropic Messages API.
    Anthropic,
    /// OpenAI Chat Completions.
    Openai,
    /// GitHub Models (OpenAI-compatible).
    GithubModels,
    /// A local (or self-hosted) Ollama server.
    Ollama,
    /// Fixture-driven provider for tests; never offered in the UI.
    Mock,
}

impl ProviderKind {
    /// Stable id used in settings and as the credential-store account name.
    pub fn id(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Openai => "openai",
            ProviderKind::GithubModels => "github-models",
            ProviderKind::Ollama => "ollama",
            ProviderKind::Mock => "mock",
        }
    }

    /// Whether the provider needs an API key or token.
    pub fn needs_key(self) -> bool {
        !matches!(self, ProviderKind::Ollama | ProviderKind::Mock)
    }

    /// The default base URL.
    pub fn default_base_url(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "https://api.anthropic.com",
            ProviderKind::Openai => "https://api.openai.com",
            ProviderKind::GithubModels => "https://models.github.ai/inference",
            ProviderKind::Ollama => "http://localhost:11434",
            ProviderKind::Mock => "mock://",
        }
    }

    /// The default model id.
    pub fn default_model(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "claude-opus-5",
            ProviderKind::Openai => "gpt-4.1",
            ProviderKind::GithubModels => "openai/gpt-4.1",
            ProviderKind::Ollama => "qwen2.5-coder",
            ProviderKind::Mock => "mock-1",
        }
    }

    /// Model ids offered as suggestions in the settings form.
    pub fn suggested_models(self) -> &'static [&'static str] {
        match self {
            ProviderKind::Anthropic => &[
                "claude-opus-5",
                "claude-opus-5-5",
                "claude-sonnet-5",
                "claude-sonnet-5-5",
                "claude-haiku-4-5",
                "claude-haiku-5-5",
            ],
            ProviderKind::Openai => &["gpt-4.1", "gpt-4o", "gpt-5", "o4-mini"],
            ProviderKind::GithubModels => &["openai/gpt-4.1", "openai/gpt-4o", "openai/o4-mini"],
            ProviderKind::Ollama => &["qwen2.5-coder", "llama3.1", "deepseek-coder-v2"],
            ProviderKind::Mock => &["mock-1"],
        }
    }
}

/// How hard a model should think (where the provider has such a control).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    /// Fastest and cheapest.
    Low,
    /// Balanced.
    Medium,
    /// Thorough (the default).
    #[default]
    High,
    /// Extra thorough.
    Xhigh,
    /// Maximum.
    Max,
}

impl Effort {
    /// The wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

/// Everything needed to talk to one provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Which provider.
    pub kind: ProviderKind,
    /// Base URL (without a trailing path).
    pub base_url: String,
    /// Model id.
    pub model: String,
    /// Effort, where supported.
    pub effort: Effort,
    /// The credential, if the provider needs one.
    pub api_key: Option<Secret>,
}

/// What a request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    /// Streamed plain-language explanation.
    Explain,
    /// Structured resolution suggestion.
    Suggest,
}

/// A fully built prompt. `file_section` is identical for every chunk of the same file so that
/// providers with prompt caching can reuse it; per-chunk content is only in `chunk_section`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Static instructions.
    pub system: String,
    /// File-level context (path, labels, commits, full files): the cacheable prefix.
    pub file_section: String,
    /// Chunk-level context and the task.
    pub chunk_section: String,
    /// The task, which decides the system prompt and the output format.
    pub task: Task,
    /// Output token limit.
    pub max_tokens: u32,
}

impl Prompt {
    /// The exact text that will be sent, for "Preview request".
    pub fn preview(&self) -> String {
        format!(
            "{}\n\n{}\n\n{}",
            self.system, self.file_section, self.chunk_section
        )
    }
}

/// What the provider should produce.
#[derive(Debug, Clone, Copy)]
pub enum Output<'a> {
    /// Free text, streamed.
    Text,
    /// JSON matching `schema`.
    Json {
        /// A short name for the schema.
        name: &'a str,
        /// The JSON schema.
        schema: &'a serde_json::Value,
    },
}

/// A finished call.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion<T> {
    /// The result.
    pub value: T,
    /// Token usage and latency.
    pub usage: Usage,
}

/// The result of "Test connection".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TestReport {
    /// The model that answered.
    pub model: String,
    /// Round-trip time in milliseconds.
    pub latency_ms: u32,
}

/// Why an AI call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    /// AI features are not set up (no provider, key or consent).
    #[error("{0}")]
    NotConfigured(String),
    /// The provider rejected the credentials.
    #[error("authentication failed: {0}")]
    Auth(String),
    /// Too many requests.
    #[error("rate limited{}", .retry_after.map(|d| format!(" (retry in {}s)", d.as_secs())).unwrap_or_default())]
    RateLimited {
        /// How long the provider asked us to wait.
        retry_after: Option<Duration>,
    },
    /// The model declined the request.
    #[error("The model declined this request")]
    Refused {
        /// The refusal category, when reported.
        category: Option<String>,
        /// An explanation, when reported.
        explanation: Option<String>,
    },
    /// The response hit the output limit.
    #[error("The response was cut off at the output limit")]
    Truncated,
    /// Structured output that does not match the schema, after one retry.
    #[error("the response did not match the expected format: {0}")]
    Schema(String),
    /// A network failure.
    #[error("network error: {0}")]
    Network(String),
    /// An error response from the provider.
    #[error("{message} (HTTP {status})")]
    Http {
        /// The HTTP status.
        status: u16,
        /// The provider's message (never contains credentials).
        message: String,
    },
    /// The user cancelled.
    #[error("cancelled")]
    Cancelled,
}

/// A text callback invoked for every streamed delta.
pub type OnDelta<'a> = &'a mut (dyn FnMut(&str) + Send);

/// An AI backend.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Which provider this is.
    fn kind(&self) -> ProviderKind;

    /// Streams the model's output for `prompt`, calling `on_delta` for each piece of text, and
    /// returns the complete text. Providers retry transient failures themselves.
    async fn stream_text(
        &self,
        prompt: &Prompt,
        output: Output<'_>,
        on_delta: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<Completion<String>, AiError>;

    /// A minimal request that proves the credentials, URL and model work.
    async fn test(&self) -> Result<TestReport, AiError>;

    /// Streams a plain-language explanation.
    async fn explain(
        &self,
        prompt: &Prompt,
        on_delta: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<Completion<String>, AiError> {
        self.stream_text(prompt, Output::Text, on_delta, cancel)
            .await
    }

    /// Requests a structured [`Suggestion`]. A response that fails schema validation is retried
    /// once; a second failure is reported as [`AiError::Schema`].
    async fn suggest(
        &self,
        prompt: &Prompt,
        cancel: &CancellationToken,
    ) -> Result<Completion<Suggestion>, AiError> {
        let schema = schema::suggestion_schema();
        let output = Output::Json {
            name: "suggestion",
            schema: &schema,
        };
        let mut last = String::new();
        let mut total = Usage::default();
        for _ in 0..2 {
            let mut ignore = |_: &str| {};
            let done = self
                .stream_text(prompt, output, &mut ignore, cancel)
                .await?;
            total.add(&done.usage);
            match schema::parse_suggestion(&done.value) {
                Ok(value) => {
                    total.latency_ms = done.usage.latency_ms.max(total.latency_ms);
                    return Ok(Completion {
                        value,
                        usage: total,
                    });
                }
                Err(reason) => last = reason,
            }
        }
        Err(AiError::Schema(last))
    }
}

/// The instruction appended to the user message for providers without native schema output.
pub fn json_instruction(schema: &serde_json::Value) -> String {
    format!(
        "Respond with a single JSON object and nothing else, matching this JSON Schema:\n{}",
        serde_json::to_string(schema).unwrap_or_default()
    )
}
