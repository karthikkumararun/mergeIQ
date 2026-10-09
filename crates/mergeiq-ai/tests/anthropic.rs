//! The Anthropic adapter against recorded-format SSE streams served by a local server.

mod support;

use std::time::Duration;

use mergeiq_ai::anthropic::{AnthropicProvider, API_VERSION, FALLBACK_BETA};
use mergeiq_ai::http::Retry;
use mergeiq_ai::{
    AiError, AiProvider, Confidence, Effort, Prompt, ProviderConfig, ProviderKind, Secret,
    Strategy, Task,
};
use support::{fixture, FakeServer, Reply};
use tokio_util::sync::CancellationToken;

const KEY: &str = "sk-ant-api03-test-key-ZZ99";

fn provider(url: &str, model: &str) -> AnthropicProvider {
    AnthropicProvider::new(ProviderConfig {
        kind: ProviderKind::Anthropic,
        base_url: url.to_string(),
        model: model.to_string(),
        effort: Effort::High,
        api_key: Some(Secret::new(KEY)),
    })
    .with_retry(Retry::instant())
}

fn prompt(task: Task) -> Prompt {
    Prompt {
        system: "SYSTEM".into(),
        file_section: "FILE SECTION".into(),
        chunk_section: "CHUNK SECTION".into(),
        task,
        max_tokens: 4096,
    }
}

#[tokio::test]
async fn explain_streams_text_and_reports_usage() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/explain.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let mut seen: Vec<String> = Vec::new();
    let mut cb = |t: &str| seen.push(t.to_string());
    let done = p
        .explain(&prompt(Task::Explain), &mut cb, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        done.value,
        "The left side renamed `total` to `sum`, while the right side added a tax parameter."
    );
    assert_eq!(seen.len(), 3, "deltas arrive one by one");
    assert_eq!(done.usage.input_tokens, 1850);
    assert_eq!(done.usage.output_tokens, 212);
    assert_eq!(done.usage.cache_read_tokens, 0);

    let req = &server.requests()[0];
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/messages");
    assert_eq!(req.header("x-api-key"), Some(KEY));
    assert_eq!(req.header("anthropic-version"), Some(API_VERSION));
    assert_eq!(req.header("anthropic-beta"), Some(FALLBACK_BETA));
    let body = req.json();
    assert_eq!(body["stream"], true);
    assert_eq!(body["fallbacks"], "default");
    assert_eq!(body["thinking"]["type"], "adaptive");
}

#[tokio::test]
async fn suggest_returns_a_validated_suggestion() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/suggest.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let done = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value.confidence, Confidence::High);
    assert_eq!(done.value.strategy, Strategy::Combined);
    assert!(done.value.resolution.starts_with("fn total(items"));
    assert_eq!(done.value.risks.len(), 1);
    assert_eq!(done.usage.cache_creation_tokens, 1808);

    let body = server.requests()[0].json();
    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    assert_eq!(body["output_config"]["effort"], "high");
    assert_eq!(
        body["output_config"]["format"]["schema"]["additionalProperties"],
        false
    );
}

#[tokio::test]
async fn the_second_request_reports_cache_reads_and_shares_the_prefix() {
    let server = FakeServer::start(vec![
        Reply::sse(&fixture("anthropic/suggest.sse")),
        Reply::sse(&fixture("anthropic/suggest_cached.sse")),
    ])
    .await;
    let p = provider(&server.url, "claude-opus-5");
    let first = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    let mut second_prompt = prompt(Task::Suggest);
    second_prompt.chunk_section = "A DIFFERENT CHUNK".into();
    let second = p
        .suggest(&second_prompt, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(first.usage.cache_read_tokens, 0);
    assert!(second.usage.cache_read_tokens > 0);
    assert_eq!(second.usage.total_input(), 51 + 1808);

    let reqs = server.requests();
    let (a, b) = (reqs[0].json(), reqs[1].json());
    assert_eq!(a["system"], b["system"]);
    assert_eq!(
        a["messages"][0]["content"][0],
        b["messages"][0]["content"][0]
    );
    assert_ne!(
        a["messages"][0]["content"][1],
        b["messages"][0]["content"][1]
    );
}

#[tokio::test]
async fn an_invalid_response_is_retried_once_then_reported() {
    let server = FakeServer::start(vec![
        Reply::sse(&fixture("anthropic/suggest_invalid.sse")),
        Reply::sse(&fixture("anthropic/suggest.sse")),
    ])
    .await;
    let p = provider(&server.url, "claude-opus-5");
    let done = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(server.requests().len(), 2);
    assert_eq!(done.value.strategy, Strategy::Combined);
    // Both attempts' tokens are counted.
    assert_eq!(done.usage.output_tokens, 9 + 305);

    let server = FakeServer::start(vec![
        Reply::sse(&fixture("anthropic/suggest_invalid.sse")),
        Reply::sse(&fixture("anthropic/suggest_invalid.sse")),
        Reply::sse(&fixture("anthropic/suggest.sse")),
    ])
    .await;
    let p = provider(&server.url, "claude-opus-5");
    let err = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, AiError::Schema(_)), "{err:?}");
    assert_eq!(server.requests().len(), 2, "exactly one retry");
}

#[tokio::test]
async fn a_refusal_shows_no_content_and_carries_the_category() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/refusal.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let err = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        err,
        AiError::Refused {
            category: Some("cyber".into()),
            explanation: None
        }
    );
    assert_eq!(err.to_string(), "The model declined this request");
    assert_eq!(server.requests().len(), 1, "a refusal is not retried");
}

#[tokio::test]
async fn max_tokens_is_reported_as_truncation() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/max_tokens.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let mut ignore = |_: &str| {};
    let err = p
        .explain(
            &prompt(Task::Explain),
            &mut ignore,
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(err, AiError::Truncated);
}

#[tokio::test]
async fn fallback_blocks_are_ignored() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/fallback.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let mut ignore = |_: &str| {};
    let done = p
        .explain(
            &prompt(Task::Explain),
            &mut ignore,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(done.value, "Recovered after a fallback.");
}

#[tokio::test]
async fn an_error_event_mid_stream_fails_the_request() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/mid_error.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let mut seen = String::new();
    let mut cb = |t: &str| seen.push_str(t);
    let err = p
        .explain(&prompt(Task::Explain), &mut cb, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        err,
        AiError::Http {
            status: 529,
            message: "Overloaded".into()
        }
    );
    assert_eq!(seen, "Partial");
}

#[tokio::test]
async fn transient_failures_are_retried_at_most_twice() {
    let overloaded = || {
        Reply::json(
            529,
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        )
    };
    let server = FakeServer::start(vec![
        overloaded(),
        Reply::json(500, "{}").header("retry-after", "0"),
        Reply::sse(&fixture("anthropic/test_ok.sse")),
    ])
    .await;
    let p = provider(&server.url, "claude-opus-5");
    let report = p.test().await.unwrap();
    assert_eq!(report.model, "claude-opus-5");
    assert_eq!(server.requests().len(), 3);

    let server = FakeServer::start(vec![
        overloaded(),
        overloaded(),
        overloaded(),
        Reply::sse(&fixture("anthropic/test_ok.sse")),
    ])
    .await;
    let p = provider(&server.url, "claude-opus-5");
    let err = p.test().await.unwrap_err();
    assert!(matches!(err, AiError::Http { status: 529, .. }), "{err:?}");
    assert_eq!(server.requests().len(), 3, "first try plus two retries");
}

#[tokio::test]
async fn rate_limits_honor_retry_after_and_then_surface() {
    let limited =
        || Reply::json(429, r#"{"error":{"message":"slow down"}}"#).header("retry-after", "0");
    let server = FakeServer::start(vec![limited(), limited(), limited()]).await;
    let p = provider(&server.url, "claude-opus-5");
    let err = p.test().await.unwrap_err();
    assert!(
        matches!(err, AiError::RateLimited { retry_after: Some(d) } if d == Duration::ZERO),
        "{err:?}"
    );
    assert_eq!(server.requests().len(), 3);
}

#[tokio::test]
async fn client_errors_are_not_retried_and_keys_never_leak() {
    let body = format!(
        r#"{{"type":"error","error":{{"type":"authentication_error","message":"invalid x-api-key {KEY}"}}}}"#
    );
    let server = FakeServer::start(vec![Reply::json(401, &body)]).await;
    let p = provider(&server.url, "claude-opus-5");
    let err = p.test().await.unwrap_err();
    assert!(matches!(err, AiError::Auth(_)), "{err:?}");
    let shown = format!("{err} {err:?}");
    assert!(!shown.contains(KEY), "{shown}");
    assert!(shown.contains("[redacted]"));
    assert_eq!(server.requests().len(), 1);

    let server = FakeServer::start(vec![Reply::json(
        400,
        r#"{"error":{"message":"model: not found"}}"#,
    )])
    .await;
    let err = provider(&server.url, "nope").test().await.unwrap_err();
    assert_eq!(
        err,
        AiError::Http {
            status: 400,
            message: "model: not found".into()
        }
    );
}

#[tokio::test]
async fn test_connection_sends_a_minimal_request() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/test_ok.sse"))]).await;
    let p = provider(&server.url, "claude-opus-5");
    let report = p.test().await.unwrap();
    assert_eq!(report.model, "claude-opus-5");
    let req = &server.requests()[0];
    assert_eq!(req.header("anthropic-beta"), None);
    let body = req.json();
    assert_eq!(body["max_tokens"], 16);
    assert!(body.get("thinking").is_none() && body.get("fallbacks").is_none());
}

#[tokio::test]
async fn a_missing_key_is_a_configuration_error_and_sends_nothing() {
    let server = FakeServer::start(vec![]).await;
    let p = AnthropicProvider::new(ProviderConfig {
        kind: ProviderKind::Anthropic,
        base_url: server.url.clone(),
        model: "claude-opus-5".into(),
        effort: Effort::High,
        api_key: None,
    });
    assert!(matches!(
        p.test().await.unwrap_err(),
        AiError::NotConfigured(_)
    ));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn cancelling_stops_the_stream() {
    let first = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5\",\"usage\":{\"input_tokens\":1}}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"first\"}}\n\n";
    let server =
        FakeServer::start(vec![Reply::slow(200, first, Duration::from_secs(30), "")]).await;
    let p = provider(&server.url, "claude-opus-5");
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let mut got = String::new();
    let started = std::time::Instant::now();
    let mut cb = |t: &str| {
        got.push_str(t);
        trigger.cancel();
    };
    let err = p
        .explain(&prompt(Task::Explain), &mut cb, &cancel)
        .await
        .unwrap_err();
    assert_eq!(err, AiError::Cancelled);
    assert_eq!(got, "first");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn a_base_url_that_already_ends_in_v1_works() {
    let server = FakeServer::start(vec![Reply::sse(&fixture("anthropic/test_ok.sse"))]).await;
    let p = provider(&format!("{}/v1/", server.url), "claude-opus-5");
    p.test().await.unwrap();
    assert_eq!(server.requests()[0].path, "/v1/messages");
}

#[derive(Clone, Default)]
struct LogSink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogSink {
    type Writer = LogSink;
    fn make_writer(&'a self) -> LogSink {
        self.clone()
    }
}

#[tokio::test]
async fn debug_logging_of_a_failed_request_never_contains_the_key() {
    let sink = LogSink::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(mergeiq_ai::logging::Redacting(sink.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let body = format!(
        r#"{{"type":"error","error":{{"type":"authentication_error","message":"invalid x-api-key {KEY}"}}}}"#
    );
    let server = FakeServer::start(vec![Reply::json(401, &body)]).await;
    let p = provider(&server.url, "claude-opus-5");
    let err = p.test().await.unwrap_err();
    assert!(matches!(err, AiError::Auth(_)));
    // Something was logged (the stack is chatty at trace level), and none of it holds the key.
    tracing::debug!("anthropic request failed with x-api-key: {KEY} authorization: Bearer {KEY}");
    let logs = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
    assert!(!logs.is_empty());
    assert!(!logs.contains(KEY), "the key leaked into the logs:\n{logs}");
    assert!(!logs.contains("ZZ99"), "part of the key leaked:\n{logs}");
}
