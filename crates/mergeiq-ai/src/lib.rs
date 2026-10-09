//! AI provider abstraction (Claude, OpenAI, GitHub Models, Ollama) for conflict explanation
//! and resolution suggestions.

pub mod anthropic;
pub mod context;
pub mod github;
pub mod http;
pub mod logging;
pub mod mock;
pub mod models;
pub mod ollama;
pub mod openai;
pub mod privacy;
pub mod prompts;
pub mod provider;
pub mod schema;
pub mod secrets;
pub mod service;
pub mod sse;
pub mod usage;
pub mod validate;

pub use provider::{
    AiError, AiProvider, Completion, Effort, OnDelta, Output, Prompt, ProviderConfig, ProviderKind,
    Task, TestReport,
};
pub use schema::{Confidence, Strategy, Suggestion};
pub use secrets::{Secret, SecretStore};
pub use tokio_util::sync::CancellationToken;
pub use usage::Usage;

/// The crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
