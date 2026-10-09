//! Anthropic Messages API over HTTPS with server-sent events.

use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::http::{self, Retry};
use crate::models::{anthropic_caps, clamp_effort};
use crate::provider::{
    AiError, AiProvider, Completion, OnDelta, Output, Prompt, ProviderConfig, ProviderKind,
    TestReport,
};
use crate::secrets::Secret;
use crate::sse::SseParser;
use crate::usage::Usage;

/// The API version header value.
pub const API_VERSION: &str = "2023-06-01";
/// The beta header that enables server-side fallbacks.
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// The Anthropic provider.
pub struct AnthropicProvider {
    config: ProviderConfig,
    client: reqwest::Client,
    retry: Retry,
}

impl AnthropicProvider {
    /// A provider for `config`.
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: http::client(),
            retry: Retry::default(),
        }
    }

    /// Overrides the retry policy (tests).
    pub fn with_retry(mut self, retry: Retry) -> Self {
        self.retry = retry;
        self
    }

    fn key(&self) -> Result<&Secret, AiError> {
        self.config
            .api_key
            .as_ref()
            .ok_or_else(|| AiError::NotConfigured("No Anthropic API key is stored".to_string()))
    }

    fn url(&self) -> String {
        http::join_url(&self.config.base_url, "/v1", "/v1/messages")
    }

    /// The request body. `light` builds the minimal body used by "Test connection".
    pub fn request_body(&self, prompt: &Prompt, output: Output<'_>, light: bool) -> Value {
        let caps = anthropic_caps(&self.config.model);
        let mut body = json!({
            "model": self.config.model,
            "max_tokens": if light { 16 } else { prompt.max_tokens },
            "stream": true,
            "system": [{ "type": "text", "text": prompt.system }],
            "messages": [{
                "role": "user",
                "content": [
                    // The breakpoint caches everything before it: system + file context.
                    {
                        "type": "text",
                        "text": prompt.file_section,
                        "cache_control": { "type": "ephemeral" }
                    },
                    { "type": "text", "text": prompt.chunk_section }
                ]
            }]
        });
        if light {
            return body;
        }
        if caps.adaptive_thinking {
            body["thinking"] = json!({ "type": "adaptive" });
        }
        let mut output_config = serde_json::Map::new();
        if let Some(effort) = clamp_effort(&self.config.model, self.config.effort) {
            output_config.insert("effort".into(), json!(effort.as_str()));
        }
        if let Output::Json { schema, .. } = output {
            if caps.structured_output {
                output_config.insert(
                    "format".into(),
                    json!({ "type": "json_schema", "schema": schema }),
                );
            }
        }
        if !output_config.is_empty() {
            body["output_config"] = Value::Object(output_config);
        }
        if caps.fallbacks {
            body["fallbacks"] = json!("default");
        }
        body
    }

    fn beta_header(&self, light: bool) -> Option<&'static str> {
        (!light && anthropic_caps(&self.config.model).fallbacks).then_some(FALLBACK_BETA)
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
        let beta = self.beta_header(light);
        let started = Instant::now();

        let response = http::send(&self.retry, cancel, &[key], || {
            // Marked sensitive so the HTTP stack never prints its value, even in debug output.
            let mut api_key = reqwest::header::HeaderValue::from_str(key.expose())
                .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static(""));
            api_key.set_sensitive(true);
            let mut req = self
                .client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", API_VERSION)
                .header("accept", "text/event-stream")
                .json(&body);
            if let Some(beta) = beta {
                req = req.header("anthropic-beta", beta);
            }
            req
        })
        .await?;

        let mut state = StreamState::default();
        let mut parser = SseParser::new();
        let mut stream = response.bytes_stream();
        loop {
            let next = tokio::select! {
                () = cancel.cancelled() => return Err(AiError::Cancelled),
                n = stream.next() => n,
            };
            match next {
                Some(Ok(bytes)) => {
                    for event in parser.feed(&bytes) {
                        state.handle(&event.data, on_delta)?;
                    }
                }
                Some(Err(e)) => {
                    return Err(AiError::Network(crate::secrets::scrub(
                        &e.without_url().to_string(),
                        &[key],
                    )))
                }
                None => break,
            }
            if state.done {
                break;
            }
        }
        if let Some(event) = parser.finish() {
            state.handle(&event.data, on_delta)?;
        }
        let model = state.model.clone();
        let mut usage = state.usage;
        usage.latency_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let text = state.finish()?;
        Ok((Completion { value: text, usage }, model))
    }
}

/// Accumulates one streamed response.
#[derive(Default)]
struct StreamState {
    text: String,
    /// `text` content blocks by index, so thinking/fallback/tool deltas are ignored.
    text_blocks: Vec<u64>,
    usage: Usage,
    model: Option<String>,
    stop_reason: Option<String>,
    category: Option<String>,
    explanation: Option<String>,
    done: bool,
}

fn as_u32(v: Option<&Value>) -> Option<u32> {
    v.and_then(Value::as_u64)
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
}

impl StreamState {
    fn take_usage(&mut self, usage: &Value) {
        if let Some(n) = as_u32(usage.get("input_tokens")) {
            self.usage.input_tokens = n;
        }
        if let Some(n) = as_u32(usage.get("output_tokens")) {
            self.usage.output_tokens = n;
        }
        if let Some(n) = as_u32(usage.get("cache_read_input_tokens")) {
            self.usage.cache_read_tokens = n;
        }
        if let Some(n) = as_u32(usage.get("cache_creation_input_tokens")) {
            self.usage.cache_creation_tokens = n;
        }
    }

    fn handle(&mut self, data: &str, on_delta: OnDelta<'_>) -> Result<(), AiError> {
        if data.trim().is_empty() {
            return Ok(());
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(());
        };
        match v.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                if let Some(m) = v.get("message") {
                    self.model = m.get("model").and_then(Value::as_str).map(str::to_string);
                    if let Some(u) = m.get("usage") {
                        self.take_usage(u);
                    }
                }
            }
            Some("content_block_start") => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                let kind = v.pointer("/content_block/type").and_then(Value::as_str);
                if kind == Some("text") {
                    self.text_blocks.push(index);
                    // Some servers include initial text in the start event.
                    if let Some(t) = v
                        .pointer("/content_block/text")
                        .and_then(Value::as_str)
                        .filter(|t| !t.is_empty())
                    {
                        self.text.push_str(t);
                        on_delta(t);
                    }
                }
            }
            Some("content_block_delta") => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if v.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta")
                    && self.text_blocks.contains(&index)
                {
                    if let Some(t) = v.pointer("/delta/text").and_then(Value::as_str) {
                        self.text.push_str(t);
                        on_delta(t);
                    }
                }
            }
            Some("message_delta") => {
                if let Some(r) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop_reason = Some(r.to_string());
                }
                if let Some(d) = v.pointer("/delta/stop_details") {
                    self.category = d
                        .get("category")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    self.explanation = d
                        .get("explanation")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                if let Some(u) = v.get("usage") {
                    self.take_usage(u);
                }
            }
            Some("message_stop") => self.done = true,
            Some("error") => {
                let kind = v
                    .pointer("/error/type")
                    .and_then(Value::as_str)
                    .unwrap_or("error");
                let message = v
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("the provider reported an error");
                let status = match kind {
                    "overloaded_error" => 529,
                    "rate_limit_error" => {
                        return Err(AiError::RateLimited { retry_after: None });
                    }
                    "authentication_error" | "permission_error" => {
                        return Err(AiError::Auth(message.to_string()));
                    }
                    "invalid_request_error" => 400,
                    _ => 500,
                };
                return Err(AiError::Http {
                    status,
                    message: message.to_string(),
                });
            }
            _ => {}
        }
        Ok(())
    }

    /// Checks the stop reason before the content is used.
    fn finish(self) -> Result<String, AiError> {
        match self.stop_reason.as_deref() {
            Some("refusal") => Err(AiError::Refused {
                category: self.category,
                explanation: self.explanation,
            }),
            Some("max_tokens") => Err(AiError::Truncated),
            Some("model_context_window_exceeded") => Err(AiError::Http {
                status: 400,
                message: "The request does not fit the model's context window".to_string(),
            }),
            Some(_) => Ok(self.text),
            None => Err(AiError::Network(
                "the response ended before it was complete".to_string(),
            )),
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Effort, Task};

    fn provider(model: &str, effort: Effort) -> AnthropicProvider {
        AnthropicProvider::new(ProviderConfig {
            kind: ProviderKind::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            model: model.into(),
            effort,
            api_key: Some(Secret::new("sk-test-0000")),
        })
    }

    fn prompt() -> Prompt {
        Prompt {
            system: "SYS".into(),
            file_section: "FILE".into(),
            chunk_section: "CHUNK".into(),
            task: Task::Suggest,
            max_tokens: 9000,
        }
    }

    #[test]
    fn the_request_body_for_a_current_model() {
        let p = provider("claude-opus-5", Effort::High);
        let schema = crate::schema::suggestion_schema();
        let b = p.request_body(
            &prompt(),
            Output::Json {
                name: "suggestion",
                schema: &schema,
            },
            false,
        );
        assert_eq!(b["model"], "claude-opus-5");
        assert_eq!(b["stream"], true);
        assert_eq!(b["max_tokens"], 9000);
        assert_eq!(b["thinking"], json!({"type": "adaptive"}));
        assert_eq!(b["output_config"]["effort"], "high");
        assert_eq!(b["output_config"]["format"]["type"], "json_schema");
        assert_eq!(b["output_config"]["format"]["schema"], schema);
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["system"][0]["text"], "SYS");
        assert!(b["system"][0].get("cache_control").is_none());
        // The breakpoint is after the file context and before the per-chunk content.
        assert_eq!(b["messages"][0]["content"][0]["text"], "FILE");
        assert_eq!(
            b["messages"][0]["content"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
        assert_eq!(b["messages"][0]["content"][1]["text"], "CHUNK");
        assert!(b["messages"][0]["content"][1]
            .get("cache_control")
            .is_none());
        for banned in ["temperature", "top_p", "top_k", "budget_tokens"] {
            assert!(!b.to_string().contains(banned), "{banned}");
        }
        assert_eq!(p.beta_header(false), Some(FALLBACK_BETA));
    }

    #[test]
    fn explain_has_no_output_format() {
        let p = provider("claude-opus-5-5", Effort::Xhigh);
        let b = p.request_body(&prompt(), Output::Text, false);
        assert_eq!(b["output_config"]["effort"], "xhigh");
        assert!(b["output_config"].get("format").is_none());
    }

    #[test]
    fn fallbacks_and_beta_header_only_where_accepted() {
        let p = provider("claude-sonnet-5", Effort::High);
        let b = p.request_body(&prompt(), Output::Text, false);
        assert!(b.get("fallbacks").is_none());
        assert_eq!(p.beta_header(false), None);
    }

    #[test]
    fn older_models_get_neither_thinking_nor_effort() {
        let p = provider("claude-haiku-4-5", Effort::High);
        let b = p.request_body(&prompt(), Output::Text, false);
        assert!(b.get("thinking").is_none());
        assert!(b.get("output_config").is_none());
    }

    #[test]
    fn the_light_body_is_minimal() {
        let p = provider("claude-opus-5", Effort::High);
        let b = p.request_body(&prompt(), Output::Text, true);
        assert_eq!(b["max_tokens"], 16);
        for k in ["thinking", "output_config", "fallbacks"] {
            assert!(b.get(k).is_none(), "{k}");
        }
        assert_eq!(p.beta_header(true), None);
    }

    fn feed(events: &[&str]) -> (Result<String, AiError>, String, Usage) {
        let mut st = StreamState::default();
        let mut seen = String::new();
        let mut cb = |t: &str| seen.push_str(t);
        for e in events {
            if let Err(err) = st.handle(e, &mut cb) {
                return (Err(err), seen, st.usage);
            }
        }
        let usage = st.usage;
        (st.finish(), seen, usage)
    }

    #[test]
    fn only_text_blocks_contribute_and_usage_is_collected() {
        let (r, seen, usage) = feed(&[
            r#"{"type":"message_start","message":{"model":"claude-opus-5","usage":{"input_tokens":12,"cache_creation_input_tokens":300,"cache_read_input_tokens":0,"output_tokens":1}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hel"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"lo"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":40}}"#,
            r#"{"type":"message_stop"}"#,
        ]);
        assert_eq!(r.unwrap(), "Hello");
        assert_eq!(seen, "Hello");
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.cache_creation_tokens, 300);
        assert_eq!(usage.output_tokens, 40);
    }

    #[test]
    fn refusal_reports_the_category_and_discards_content() {
        let (r, _, _) = feed(&[
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"partial"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"refusal","stop_details":{"type":"refusal","category":"cyber","explanation":"nope"}},"usage":{"output_tokens":3}}"#,
            r#"{"type":"message_stop"}"#,
        ]);
        assert_eq!(
            r.unwrap_err(),
            AiError::Refused {
                category: Some("cyber".into()),
                explanation: Some("nope".into())
            }
        );
    }

    #[test]
    fn max_tokens_and_incomplete_streams_are_errors() {
        let (r, _, _) = feed(&[
            r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":9}}"#,
        ]);
        assert_eq!(r.unwrap_err(), AiError::Truncated);
        let (r, _, _) = feed(&[
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":"x"}}"#,
        ]);
        assert!(matches!(r.unwrap_err(), AiError::Network(_)));
    }

    #[test]
    fn mid_stream_errors_are_mapped() {
        let (r, _, _) = feed(&[
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        ]);
        assert_eq!(
            r.unwrap_err(),
            AiError::Http {
                status: 529,
                message: "Overloaded".into()
            }
        );
        let (r, _, _) = feed(&[
            r#"{"type":"error","error":{"type":"authentication_error","message":"bad key"}}"#,
        ]);
        assert_eq!(r.unwrap_err(), AiError::Auth("bad key".into()));
    }

    #[test]
    fn unknown_events_and_garbage_are_ignored() {
        let (r, _, _) = feed(&[
            "",
            "not json",
            r#"{"type":"ping"}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"fallback"}}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"ok"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#,
        ]);
        assert_eq!(r.unwrap(), "ok");
    }
}
