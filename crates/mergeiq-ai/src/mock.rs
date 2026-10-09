//! A scripted provider for tests and offline demos. It never touches the network.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::context::estimate_tokens;
use crate::provider::{
    AiError, AiProvider, Completion, OnDelta, Output, Prompt, ProviderKind, TestReport,
};
use crate::schema::{Confidence, Strategy, Suggestion};
use crate::usage::Usage;

/// What the next request returns.
#[derive(Debug, Clone)]
pub enum Step {
    /// Streamed word by word (explain) or returned as is.
    Text(String),
    /// A structured suggestion, serialized as the model would.
    Suggestion(Suggestion),
    /// Arbitrary text, for malformed-output tests.
    Raw(String),
    /// A failure.
    Fail(AiError),
}

/// The scripted provider. With an empty script it answers deterministically from the prompt:
/// explanations are a fixed sentence and suggestions keep both sides.
pub struct MockProvider {
    steps: Mutex<VecDeque<Step>>,
    prompts: Mutex<Vec<Prompt>>,
    delta_delay: Duration,
}

impl MockProvider {
    /// A mock with the given script.
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: Mutex::new(steps.into()),
            prompts: Mutex::new(Vec::new()),
            delta_delay: Duration::ZERO,
        }
    }

    /// Pauses between streamed words, so cancellation can be exercised.
    pub fn with_delta_delay(mut self, delay: Duration) -> Self {
        self.delta_delay = delay;
        self
    }

    /// Every prompt received, oldest first.
    pub fn prompts(&self) -> Vec<Prompt> {
        self.prompts.lock().unwrap().clone()
    }
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> &'a str {
    let Some(start) = text.find(open) else {
        return "";
    };
    let rest = &text[start + open.len()..];
    // Skip the rest of the opening tag.
    let rest = rest.split_once(">\n").map_or(rest, |(_, r)| r);
    rest.split_once(close).map_or(rest, |(body, _)| body)
}

/// The deterministic answer used when the script is empty.
pub fn default_suggestion(prompt: &Prompt) -> Suggestion {
    let left = between(&prompt.chunk_section, "<left ", "</left>");
    let right = between(&prompt.chunk_section, "<right ", "</right>");
    let clean = |s: &str| {
        if s.trim() == "(empty)" {
            String::new()
        } else {
            s.to_string()
        }
    };
    let (l, r) = (clean(left), clean(right));
    Suggestion {
        resolution: format!("{l}{r}"),
        explanation: "Keeps the changes from both sides, left first.".to_string(),
        confidence: Confidence::Medium,
        strategy: Strategy::Both,
        risks: vec!["Mock provider: not a real analysis.".to_string()],
    }
}

const DEFAULT_EXPLANATION: &str = "The left side and the right side both changed this region. \
Keeping both changes is safe here because they do not depend on each other.";

#[async_trait]
impl AiProvider for MockProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Mock
    }

    async fn stream_text(
        &self,
        prompt: &Prompt,
        output: Output<'_>,
        on_delta: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<Completion<String>, AiError> {
        self.prompts.lock().unwrap().push(prompt.clone());
        let step = self.steps.lock().unwrap().pop_front();
        let text = match (step, output) {
            (Some(Step::Fail(e)), _) => return Err(e),
            (Some(Step::Text(t) | Step::Raw(t)), _) => t,
            (Some(Step::Suggestion(s)), _) => serde_json::to_string(&s).unwrap_or_default(),
            (None, Output::Text) => DEFAULT_EXPLANATION.to_string(),
            (None, Output::Json { .. }) => {
                serde_json::to_string(&default_suggestion(prompt)).unwrap_or_default()
            }
        };
        let mut sent = 0u32;
        if matches!(output, Output::Text) {
            for word in text.split_inclusive(' ') {
                if cancel.is_cancelled() {
                    return Err(AiError::Cancelled);
                }
                if !self.delta_delay.is_zero() {
                    tokio::select! {
                        () = cancel.cancelled() => return Err(AiError::Cancelled),
                        () = tokio::time::sleep(self.delta_delay) => {}
                    }
                }
                on_delta(word);
                sent += 1;
            }
        }
        Ok(Completion {
            usage: Usage {
                input_tokens: estimate_tokens(&prompt.preview()),
                output_tokens: sent.max(estimate_tokens(&text)),
                ..Usage::default()
            },
            value: text,
        })
    }

    async fn test(&self) -> Result<TestReport, AiError> {
        Ok(TestReport {
            model: "mock-1".to_string(),
            latency_ms: 1,
        })
    }
}
