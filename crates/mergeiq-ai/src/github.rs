//! GitHub Models: OpenAI-compatible chat completions at `models.github.ai`, with JSON mode and
//! client-side schema validation instead of native structured output.

use crate::openai::{ChatProvider, Flavor};
use crate::provider::ProviderConfig;

/// A GitHub Models provider for `config`.
pub fn provider(config: ProviderConfig) -> ChatProvider {
    ChatProvider::new(config, Flavor::Github)
}
