//! Ollama's native chat API (`/api/chat`, newline-delimited JSON).

use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::context::estimate_tokens;
use crate::http::{self, Retry};
use crate::provider::{
    json_instruction, AiError, AiProvider, Completion, OnDelta, Output, Prompt, ProviderConfig,
    ProviderKind, TestReport,
};
use crate::sse::LineParser;
use crate::usage::Usage;

/// An Ollama provider.
pub struct OllamaProvider {
    config: ProviderConfig,
    client: reqwest::Client,
    retry: Retry,
}

impl OllamaProvider {
    /// A provider for `config`. No key is needed.
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

    fn url(&self) -> String {
        http::join_url(&self.config.base_url, "", "/api/chat")
    }

    /// Ollama defaults to a 4096-token context, which silently truncates our prompts, so the
    /// window is sized to the request (rounded up to 4096, at most 131072).
    fn context_window(prompt: &Prompt) -> u32 {
        let needed = estimate_tokens(&prompt.preview())
            .saturating_add(prompt.max_tokens)
            .saturating_add(256);
        (needed.div_ceil(4096) * 4096).clamp(8192, 131_072)
    }

    /// The request body. `light` builds the minimal "Test connection" body.
    pub fn request_body(&self, prompt: &Prompt, output: Output<'_>, light: bool) -> Value {
        let mut user = format!("{}\n\n{}", prompt.file_section, prompt.chunk_section);
        let mut body = json!({
            "model": self.config.model,
            "stream": true,
        });
        if light {
            body["options"] = json!({ "num_predict": 8 });
        } else {
            body["options"] = json!({
                "num_predict": prompt.max_tokens,
                "num_ctx": Self::context_window(prompt),
                "temperature": 0.2,
            });
            if let Output::Json { schema, .. } = output {
                body["format"] = schema.clone();
                user.push_str("\n\n");
                user.push_str(&json_instruction(schema));
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
    ) -> Result<Completion<String>, AiError> {
        let body = self.request_body(prompt, output, light);
        let url = self.url();
        let started = Instant::now();
        let response = http::send(&self.retry, cancel, &[], || {
            self.client.post(&url).json(&body)
        })
        .await?;

        let mut text = String::new();
        let mut usage = Usage::default();
        let mut done_reason: Option<String> = None;
        let mut parser = LineParser::new();
        let mut stream = response.bytes_stream();
        let mut handle = |line: &str, on_delta: OnDelta<'_>| -> Result<(), AiError> {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                return Ok(());
            };
            if let Some(e) = v.get("error").and_then(Value::as_str) {
                return Err(AiError::Http {
                    status: 500,
                    message: e.to_string(),
                });
            }
            if let Some(t) = v.pointer("/message/content").and_then(Value::as_str) {
                if !t.is_empty() {
                    text.push_str(t);
                    on_delta(t);
                }
            }
            if v.get("done").and_then(Value::as_bool) == Some(true) {
                done_reason = Some(
                    v.get("done_reason")
                        .and_then(Value::as_str)
                        .unwrap_or("stop")
                        .to_string(),
                );
                let n = |k: &str| {
                    v.get(k)
                        .and_then(Value::as_u64)
                        .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
                };
                usage.input_tokens = n("prompt_eval_count");
                usage.output_tokens = n("eval_count");
            }
            Ok(())
        };
        loop {
            let next = tokio::select! {
                () = cancel.cancelled() => return Err(AiError::Cancelled),
                n = stream.next() => n,
            };
            match next {
                Some(Ok(bytes)) => {
                    for line in parser.feed(&bytes) {
                        handle(&line, on_delta)?;
                    }
                }
                Some(Err(e)) => return Err(AiError::Network(e.without_url().to_string())),
                None => break,
            }
        }
        if let Some(line) = parser.finish() {
            handle(&line, on_delta)?;
        }
        match done_reason.as_deref() {
            Some("length") => return Err(AiError::Truncated),
            Some(_) => {}
            None => {
                return Err(AiError::Network(
                    "the response ended before it was complete".to_string(),
                ))
            }
        }
        usage.latency_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        Ok(Completion { value: text, usage })
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    async fn stream_text(
        &self,
        prompt: &Prompt,
        output: Output<'_>,
        on_delta: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<Completion<String>, AiError> {
        self.run(prompt, output, false, on_delta, cancel).await
    }

    async fn test(&self) -> Result<TestReport, AiError> {
        let prompt = Prompt {
            system: "Reply with the single word OK.".to_string(),
            file_section: "Connection test.".to_string(),
            chunk_section: "Say OK.".to_string(),
            task: crate::provider::Task::Explain,
            max_tokens: 8,
        };
        let started = Instant::now();
        let mut ignore = |_: &str| {};
        // `num_predict` ends the generation with done_reason "length"; that is still a working
        // connection, so only other failures count.
        match self
            .run(
                &prompt,
                Output::Text,
                true,
                &mut ignore,
                &CancellationToken::new(),
            )
            .await
        {
            Ok(_) | Err(AiError::Truncated) => Ok(TestReport {
                model: self.config.model.clone(),
                latency_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
            }),
            Err(e) => Err(e),
        }
    }
}
