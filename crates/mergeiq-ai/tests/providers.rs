//! OpenAI, GitHub Models, Ollama and the mock provider against local fake servers.

mod support;

use std::time::Duration;

use mergeiq_ai::http::Retry;
use mergeiq_ai::mock::{MockProvider, Step};
use mergeiq_ai::ollama::OllamaProvider;
use mergeiq_ai::openai::{ChatProvider, Flavor};
use mergeiq_ai::{
    AiError, AiProvider, Confidence, Effort, Prompt, ProviderConfig, ProviderKind, Secret,
    Strategy, Suggestion, Task,
};
use serde_json::json;
use support::{FakeServer, Reply};
use tokio_util::sync::CancellationToken;

const TOKEN: &str = "ghp_exampletoken1234567890";

fn config(kind: ProviderKind, url: &str, model: &str, key: bool) -> ProviderConfig {
    ProviderConfig {
        kind,
        base_url: url.to_string(),
        model: model.to_string(),
        effort: Effort::High,
        api_key: key.then(|| Secret::new(TOKEN)),
    }
}

fn prompt(task: Task) -> Prompt {
    Prompt {
        system: "SYSTEM".into(),
        file_section: "FILE".into(),
        chunk_section: "<left label=\"main\">\nL\n</left>\n<right label=\"x\">\nR\n</right>".into(),
        task,
        max_tokens: 2048,
    }
}

fn chat_sse(deltas: &[&str], finish: &str, usage: Option<(u32, u32, u32)>) -> String {
    let mut out = String::new();
    for d in deltas {
        out.push_str(&format!(
            "data: {}\n\n",
            json!({"id":"c1","model":"gpt-4.1-2026","choices":[{"index":0,"delta":{"content":d},"finish_reason":null}]})
        ));
    }
    out.push_str(&format!(
        "data: {}\n\n",
        json!({"id":"c1","choices":[{"index":0,"delta":{},"finish_reason":finish}]})
    ));
    if let Some((prompt, cached, completion)) = usage {
        out.push_str(&format!(
            "data: {}\n\n",
            json!({"id":"c1","choices":[],"usage":{"prompt_tokens":prompt,"completion_tokens":completion,"prompt_tokens_details":{"cached_tokens":cached}}})
        ));
    }
    out.push_str("data: [DONE]\n\n");
    out
}

fn suggestion_json() -> String {
    json!({
        "resolution": "L\nR\n", "explanation": "Both.", "confidence": "medium",
        "strategy": "both", "risks": []
    })
    .to_string()
}

#[tokio::test]
async fn openai_explain_streams_and_splits_cached_tokens() {
    let body = chat_sse(&["Hello", " there"], "stop", Some((1000, 800, 20)));
    let server = FakeServer::start(vec![Reply::sse(&body)]).await;
    let p = ChatProvider::new(
        config(ProviderKind::Openai, &server.url, "gpt-4.1", true),
        Flavor::Openai,
    )
    .with_retry(Retry::instant());
    let mut seen = Vec::new();
    let mut cb = |t: &str| seen.push(t.to_string());
    let done = p
        .explain(&prompt(Task::Explain), &mut cb, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value, "Hello there");
    assert_eq!(seen, vec!["Hello", " there"]);
    assert_eq!(done.usage.input_tokens, 200);
    assert_eq!(done.usage.cache_read_tokens, 800);
    assert_eq!(done.usage.output_tokens, 20);

    let req = &server.requests()[0];
    assert_eq!(req.path, "/v1/chat/completions");
    assert_eq!(
        req.header("authorization"),
        Some(&format!("Bearer {TOKEN}")[..])
    );
    let b = req.json();
    assert_eq!(b["stream"], true);
    assert_eq!(b["stream_options"]["include_usage"], true);
    assert_eq!(b["max_completion_tokens"], 2048);
    assert!(b.get("response_format").is_none());
    assert!(
        b.get("reasoning_effort").is_none(),
        "gpt-4.1 is not a reasoning model"
    );
    assert_eq!(b["messages"][0]["role"], "system");
}

#[tokio::test]
async fn openai_suggest_uses_strict_json_schema() {
    let json = suggestion_json();
    let server = FakeServer::start(vec![Reply::sse(&chat_sse(
        &[&json[..20], &json[20..]],
        "stop",
        None,
    ))])
    .await;
    let p = ChatProvider::new(
        config(ProviderKind::Openai, &server.url, "gpt-5", true),
        Flavor::Openai,
    )
    .with_retry(Retry::instant());
    let done = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value.strategy, Strategy::Both);
    let b = server.requests()[0].json();
    assert_eq!(b["response_format"]["type"], "json_schema");
    assert_eq!(b["response_format"]["json_schema"]["strict"], true);
    assert_eq!(
        b["response_format"]["json_schema"]["schema"]["additionalProperties"],
        false
    );
    assert_eq!(b["reasoning_effort"], "high");
}

#[tokio::test]
async fn github_models_uses_json_mode_and_validates_client_side() {
    let good = suggestion_json();
    let server = FakeServer::start(vec![
        Reply::sse(&chat_sse(&["{\"resolution\":\"x\"}"], "stop", None)),
        Reply::sse(&chat_sse(&[&good], "stop", None)),
    ])
    .await;
    let p = mergeiq_ai::github::provider(config(
        ProviderKind::GithubModels,
        &server.url,
        "openai/gpt-4.1",
        true,
    ))
    .with_retry(Retry::instant());
    let done = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value.confidence, Confidence::Medium);
    let reqs = server.requests();
    assert_eq!(reqs.len(), 2, "invalid output retried once");
    assert_eq!(reqs[0].path, "/chat/completions");
    assert_eq!(reqs[0].header("x-github-api-version"), Some("2022-11-28"));
    let b = reqs[0].json();
    assert_eq!(b["response_format"], json!({"type": "json_object"}));
    assert_eq!(b["max_tokens"], 2048);
    let user = b["messages"][1]["content"].as_str().unwrap();
    assert!(
        user.contains("JSON Schema"),
        "json mode needs the schema in the prompt"
    );
}

#[tokio::test]
async fn chat_finish_reasons_map_to_errors() {
    let run = |finish: &'static str| async move {
        let server = FakeServer::start(vec![Reply::sse(&chat_sse(&["x"], finish, None))]).await;
        let p = ChatProvider::new(
            config(ProviderKind::Openai, &server.url, "gpt-4.1", true),
            Flavor::Openai,
        );
        let mut ignore = |_: &str| {};
        p.explain(
            &prompt(Task::Explain),
            &mut ignore,
            &CancellationToken::new(),
        )
        .await
        .unwrap_err()
    };
    assert_eq!(run("length").await, AiError::Truncated);
    assert!(matches!(
        run("content_filter").await,
        AiError::Refused { .. }
    ));

    let refusal = format!(
        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"delta":{"refusal":"I can't help with that."},"finish_reason":null}]}),
        json!({"choices":[{"delta":{},"finish_reason":"stop"}]})
    );
    let server = FakeServer::start(vec![Reply::sse(&refusal)]).await;
    let p = ChatProvider::new(
        config(ProviderKind::Openai, &server.url, "gpt-4.1", true),
        Flavor::Openai,
    );
    let err = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        err,
        AiError::Refused {
            category: Some("refusal".into()),
            explanation: Some("I can't help with that.".into())
        }
    );
}

#[tokio::test]
async fn chat_http_errors_map_and_never_leak_the_token() {
    let body = format!(r#"{{"error":{{"message":"Incorrect API key provided: {TOKEN}"}}}}"#);
    let server = FakeServer::start(vec![Reply::json(401, &body)]).await;
    let p = ChatProvider::new(
        config(ProviderKind::Openai, &server.url, "gpt-4.1", true),
        Flavor::Openai,
    );
    let err = p.test().await.unwrap_err();
    assert!(matches!(err, AiError::Auth(_)));
    assert!(!format!("{err:?}{err}").contains(TOKEN));

    let none = ChatProvider::new(
        config(ProviderKind::Openai, &server.url, "gpt-4.1", false),
        Flavor::Openai,
    );
    assert!(matches!(
        none.test().await.unwrap_err(),
        AiError::NotConfigured(_)
    ));
}

#[tokio::test]
async fn chat_test_connection_reports_the_model() {
    let server = FakeServer::start(vec![Reply::sse(&chat_sse(
        &["OK"],
        "stop",
        Some((9, 0, 1)),
    ))])
    .await;
    let p = ChatProvider::new(
        config(ProviderKind::Openai, &server.url, "gpt-4.1", true),
        Flavor::Openai,
    );
    let report = p.test().await.unwrap();
    assert_eq!(report.model, "gpt-4.1-2026");
    let b = server.requests()[0].json();
    assert_eq!(b["max_completion_tokens"], 16);
}

fn ndjson(deltas: &[&str], done_reason: &str) -> String {
    let mut out = String::new();
    for d in deltas {
        out.push_str(&format!(
            "{}\n",
            json!({"model":"qwen2.5-coder","message":{"role":"assistant","content":d},"done":false})
        ));
    }
    out.push_str(&format!(
        "{}\n",
        json!({"model":"qwen2.5-coder","message":{"role":"assistant","content":""},"done":true,"done_reason":done_reason,"prompt_eval_count":321,"eval_count":45})
    ));
    out
}

#[tokio::test]
async fn ollama_explain_streams_ndjson_without_a_key() {
    let server = FakeServer::start(vec![Reply::ndjson(&ndjson(
        &["Both ", "sides ", "changed it."],
        "stop",
    ))])
    .await;
    let p = OllamaProvider::new(config(
        ProviderKind::Ollama,
        &server.url,
        "qwen2.5-coder",
        false,
    ))
    .with_retry(Retry::instant());
    let mut seen = Vec::new();
    let mut cb = |t: &str| seen.push(t.to_string());
    let done = p
        .explain(&prompt(Task::Explain), &mut cb, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value, "Both sides changed it.");
    assert_eq!(seen.len(), 3);
    assert_eq!(done.usage.input_tokens, 321);
    assert_eq!(done.usage.output_tokens, 45);
    let req = &server.requests()[0];
    assert_eq!(req.path, "/api/chat");
    assert_eq!(req.header("authorization"), None);
    let b = req.json();
    assert_eq!(b["stream"], true);
    assert!(
        b["options"]["num_ctx"].as_u64().unwrap() >= 8192,
        "default 4096 would truncate"
    );
    assert_eq!(b["options"]["num_predict"], 2048);
}

#[tokio::test]
async fn ollama_suggest_passes_the_schema_as_format() {
    let json = suggestion_json();
    let server = FakeServer::start(vec![Reply::ndjson(&ndjson(&[&json], "stop"))]).await;
    let p = OllamaProvider::new(config(
        ProviderKind::Ollama,
        &server.url,
        "qwen2.5-coder",
        false,
    ));
    let done = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value.resolution, "L\nR\n");
    let b = server.requests()[0].json();
    assert_eq!(b["format"]["type"], "object");
    assert_eq!(b["format"]["required"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn ollama_errors_and_truncation() {
    let server = FakeServer::start(vec![Reply::json(
        404,
        r#"{"error":"model 'nope' not found"}"#,
    )])
    .await;
    let p = OllamaProvider::new(config(ProviderKind::Ollama, &server.url, "nope", false));
    let err = p.test().await.unwrap_err();
    assert_eq!(
        err,
        AiError::Http {
            status: 404,
            message: "model 'nope' not found".into()
        }
    );

    let server = FakeServer::start(vec![Reply::ndjson(&ndjson(&["cut"], "length"))]).await;
    let p = OllamaProvider::new(config(ProviderKind::Ollama, &server.url, "m", false));
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
async fn ollama_test_connection_tolerates_the_tiny_prediction_limit() {
    let server = FakeServer::start(vec![Reply::ndjson(&ndjson(&["OK"], "length"))]).await;
    let p = OllamaProvider::new(config(
        ProviderKind::Ollama,
        &server.url,
        "qwen2.5-coder",
        false,
    ));
    assert_eq!(p.test().await.unwrap().model, "qwen2.5-coder");
}

#[tokio::test]
async fn ollama_connection_refused_is_a_network_error() {
    // Nothing listens on this port.
    let p = OllamaProvider::new(config(
        ProviderKind::Ollama,
        "http://127.0.0.1:9",
        "m",
        false,
    ))
    .with_retry(Retry {
        max_retries: 1,
        base_delay: Duration::ZERO,
        max_delay: Duration::ZERO,
    });
    assert!(matches!(p.test().await.unwrap_err(), AiError::Network(_)));
}

#[tokio::test]
async fn the_mock_provider_runs_the_full_suggest_flow_offline() {
    let p = MockProvider::new(vec![]);
    let done = p
        .suggest(&prompt(Task::Suggest), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(done.value.resolution, "L\nR\n");
    assert_eq!(done.value.strategy, Strategy::Both);
    assert_eq!(p.prompts().len(), 1);

    let scripted = Suggestion {
        resolution: "X\n".into(),
        explanation: "Because.".into(),
        confidence: Confidence::Low,
        strategy: Strategy::New,
        risks: vec!["r".into()],
    };
    let p = MockProvider::new(vec![
        Step::Raw("{oops".into()),
        Step::Suggestion(scripted.clone()),
    ]);
    assert_eq!(
        p.suggest(&prompt(Task::Suggest), &CancellationToken::new())
            .await
            .unwrap()
            .value,
        scripted
    );

    let p = MockProvider::new(vec![Step::Fail(AiError::Truncated)]);
    assert_eq!(
        p.suggest(&prompt(Task::Suggest), &CancellationToken::new())
            .await
            .unwrap_err(),
        AiError::Truncated
    );
}

#[tokio::test]
async fn the_mock_provider_streams_and_can_be_cancelled() {
    let p = MockProvider::new(vec![Step::Text("one two three four".into())]);
    let mut words = Vec::new();
    let mut cb = |t: &str| words.push(t.to_string());
    let done = p
        .explain(&prompt(Task::Explain), &mut cb, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(words.concat(), "one two three four");
    assert_eq!(words.len(), 4);
    assert!(done.usage.output_tokens > 0);

    let p = MockProvider::new(vec![Step::Text("a b c d e f g h".into())])
        .with_delta_delay(Duration::from_millis(20));
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let mut n = 0;
    let mut cb = |_: &str| {
        n += 1;
        if n == 2 {
            trigger.cancel();
        }
    };
    let err = p
        .explain(&prompt(Task::Explain), &mut cb, &cancel)
        .await
        .unwrap_err();
    assert_eq!(err, AiError::Cancelled);
    assert_eq!(n, 2);
}
