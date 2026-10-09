//! OpenAI-compatible Chat Completions (OpenAI itself and GitHub Models).

use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::http::{self, Retry};
use crate::models::{openai_json_schema, openai_takes_reasoning_effort};
use crate::provider::{
    json_instruction, AiError, AiProvider, Completion, OnDelta, Output, Prompt, ProviderConfig,
    ProviderKind, TestReport,
};
use crate::secrets::{scrub, Secret};
use crate::sse::SseParser;
use crate::usage::Usage;

/// Which service speaks this protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    /// api.openai.com.
    Openai,
    /// models.github.ai.
    Github,
}

/// A Chat Completions provider.
pub struct ChatProvider {
    config: ProviderConfig,
    flavor: Flavor,
    client: reqwest::Client,
    retry: Retry,
}

impl ChatProvider {
    /// A provider for `config`; `flavor` selects URL layout and headers.
    pub fn new(config: ProviderConfig, flavor: Flavor) -> Self {
        Self {
            config,
            flavor,
            client: http::client(),
            retry: Retry::default(),
        }
    }

    /// Overrides the retry policy (tests).
    pub fn with_retry(mut self, retry: Retry) -> Self {
        self.retry = retry;
        self
    }

    fn name(&self) -> &'static str {
        match self.flavor {
            Flavor::Openai => "OpenAI",
            Flavor::Github => "GitHub Models",
        }
    }

    fn url(&self) -> String {
        match self.flavor {
            Flavor::Openai => http::join_url(&self.config.base_url, "/v1", "/v1/chat/completions"),
            Flavor::Github => http::join_url(&self.config.base_url, "", "/chat/completions"),
        }
    }

    fn key(&self) -> Result<&Secret, AiError> {
        self.config
            .api_key
            .as_ref()
            .ok_or_else(|| AiError::NotConfigured(format!("No {} token is stored", self.name())))
    }

    /// GitHub Models only offers JSON mode, validated client-side; OpenAI's current models take
    /// strict JSON-schema output.
    fn native_schema(&self) -> bool {
        self.flavor == Flavor::Openai && openai_json_schema(&self.config.model)
    }

    /// The request body. `light` builds the minimal "Test connection" body.
    pub fn request_body(&self, prompt: &Prompt, output: Output<'_>, light: bool) -> Value {
        let mut user = format!("{}\n\n{}", prompt.file_section, prompt.chunk_section);
        let mut body = json!({
            "model": self.config.model,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        let limit = if light { 16 } else { prompt.max_tokens };
        // Reasoning models only accept `max_completion_tokens`; it is also valid for the rest
        // on OpenAI. GitHub Models still documents `max_tokens`.
        let limit_key = if self.flavor == Flavor::Openai {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body[limit_key] = json!(limit);
        if !light {
            if self.flavor == Flavor::Openai && openai_takes_reasoning_effort(&self.config.model) {
                let effort = match self.config.effort {
                    crate::provider::Effort::Low => "low",
                    crate::provider::Effort::Medium => "medium",
                    _ => "high",
                };
                body["reasoning_effort"] = json!(effort);
            }
            if let Output::Json { name, schema } = output {
                if self.native_schema() {
                    body["response_format"] = json!({
                        "type": "json_schema",
                        "json_schema": { "name": name, "strict": true, "schema": schema }
                    });
                } else {
                    body["response_format"] = json!({ "type": "json_object" });
                    user.push_str("\n\n");
                    user.push_str(&json_instruction(schema));
                }
            }
        }
        body["messages"] = json!([
            { "role": "system", "content": prompt.system },
            { "role": "user", "content": user }
        ]);
        body
    }

    async fn run(
        &self,
        prompt: &Prompt,
        output: Output<'_>,
        light: bool,
        on_delta: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<(Completion<String>, Option<String>), AiError> {
        let key = self.key()?;
        let body = self.request_body(prompt, output, light);
        let url = self.url();
        let started = Instant::now();
        let response = http::send(&self.retry, cancel, &[key], || {
            let mut req = self
                .client
                .post(&url)
                .bearer_auth(key.expose())
                .header("accept", "text/event-stream")
                .json(&body);
            if self.flavor == Flavor::Github {
                req = req
                    .header("accept", "application/vnd.github+json")
                    .header("x-github-api-version", "2022-11-28");
            }
            req
        })
        .await?;

        let mut state = ChatStream::default();
        let mut parser = SseParser::new();
        let mut stream = response.bytes_stream();
        'read: loop {
            let next = tokio::select! {
                () = cancel.cancelled() => return Err(AiError::Cancelled),
                n = stream.next() => n,
            };
            match next {
                Some(Ok(bytes)) => {
                    for event in parser.feed(&bytes) {
                        if state.handle(&event.data, on_delta, &[key])? {
                            break 'read;
                        }
                    }
                }
                Some(Err(e)) => {
                    return Err(AiError::Network(scrub(
                        &e.without_url().to_string(),
                        &[key],
                    )))
                }
                None => break,
            }
        }
        if let Some(event) = parser.finish() {
            state.handle(&event.data, on_delta, &[key])?;
        }
        let model = state.model.clone();
        let mut usage = state.usage;
        usage.latency_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        Ok((
            Completion {
                value: state.finish()?,
                usage,
            },
            model,
        ))
    }
}

#[derive(Default)]
struct ChatStream {
    text: String,
    usage: Usage,
    model: Option<String>,
    finish_reason: Option<String>,
    refusal: Option<String>,
}

impl ChatStream {
    /// Returns `true` at `[DONE]`.
    fn handle(
        &mut self,
        data: &str,
        on_delta: OnDelta<'_>,
        secrets: &[&Secret],
    ) -> Result<bool, AiError> {
        let data = data.trim();
        if data.is_empty() {
            return Ok(false);
        }
        if data == "[DONE]" {
            return Ok(true);
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(false);
        };
        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| err.as_str())
                .unwrap_or("the provider reported an error");
            return Err(AiError::Http {
                status: 500,
                message: scrub(message, secrets),
            });
        }
        if let Some(m) = v.get("model").and_then(Value::as_str) {
            self.model.get_or_insert_with(|| m.to_string());
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            let prompt = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
            let cached = u
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(prompt);
            self.usage.input_tokens = u32::try_from(prompt - cached).unwrap_or(u32::MAX);
            self.usage.cache_read_tokens = u32::try_from(cached).unwrap_or(u32::MAX);
            self.usage.output_tokens = u
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX));
        }
        if let Some(choice) = v.pointer("/choices/0") {
            if let Some(t) = choice.pointer("/delta/content").and_then(Value::as_str) {
                if !t.is_empty() {
                    self.text.push_str(t);
                    on_delta(t);
                }
            }
            if let Some(r) = choice.pointer("/delta/refusal").and_then(Value::as_str) {
                self.refusal.get_or_insert_with(String::new).push_str(r);
            }
            if let Some(f) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish_reason = Some(f.to_string());
            }
        }
        Ok(false)
    }

    fn finish(self) -> Result<String, AiError> {
        if let Some(explanation) = self.refusal {
            return Err(AiError::Refused {
                category: Some("refusal".into()),
                explanation: Some(explanation).filter(|e| !e.is_empty()),
            });
        }
        match self.finish_reason.as_deref() {
            Some("length") => Err(AiError::Truncated),
            Some("content_filter") => Err(AiError::Refused {
                category: Some("content_filter".into()),
                explanation: None,
            }),
            Some(_) => Ok(self.text),
            None => Err(AiError::Network(
                "the response ended before it was complete".to_string(),
            )),
        }
    }
}

#[async_trait]
impl AiProvider for ChatProvider {
    fn kind(&self) -> ProviderKind {
        match self.flavor {
            Flavor::Openai => ProviderKind::Openai,
            Flavor::Github => ProviderKind::GithubModels,
        }
    }

    async fn stream_text(
        &self,
        prompt: &Prompt,
        output: Output<'_>,
        on_delta: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<Completion<String>, AiError> {
        Ok(self.run(prompt, output, false, on_delta, cancel).await?.0)
    }

    async fn test(&self) -> Result<TestReport, AiError> {
        let prompt = Prompt {
            system: "Reply with the single word OK.".to_string(),
            file_section: "Connection test.".to_string(),
            chunk_section: "Say OK.".to_string(),
            task: crate::provider::Task::Explain,
            max_tokens: 16,
        };
        let started = Instant::now();
        let mut ignore = |_: &str| {};
        let (_, model) = self
            .run(
                &prompt,
                Output::Text,
                true,
                &mut ignore,
                &CancellationToken::new(),
            )
            .await?;
        Ok(TestReport {
            model: model.unwrap_or_else(|| self.config.model.clone()),
            latency_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
        })
    }
}
