//! Live smoke tests against real providers. Never run by default or in CI.
//!
//! ```text
//! MERGEIQ_LIVE_AI=1 ANTHROPIC_API_KEY=sk-ant-... \
//!   cargo test -p mergeiq-ai --test live -- --ignored --nocapture
//! ```
//!
//! - Anthropic needs `ANTHROPIC_API_KEY` (model: `MERGEIQ_LIVE_ANTHROPIC_MODEL`, default
//!   `claude-opus-5`). It asks for suggestions for two conflicts in the same file and checks that
//!   the second request reads the first one's prompt cache.
//! - Ollama needs a running server (`MERGEIQ_LIVE_OLLAMA_URL`, default `http://localhost:11434`)
//!   and a pulled model (`MERGEIQ_LIVE_OLLAMA_MODEL`, default `qwen2.5-coder`).
//!
//! Each test prints the suggestion and token usage so a person can judge the result.

use mergeiq_ai::anthropic::AnthropicProvider;
use mergeiq_ai::context::{build, ChunkSpans, ContextInput, ContextSettings, SideInput, Span};
use mergeiq_ai::ollama::OllamaProvider;
use mergeiq_ai::{
    AiProvider, CancellationToken, Effort, ProviderConfig, ProviderKind, Secret, Suggestion, Task,
};

fn enabled() -> bool {
    if std::env::var("MERGEIQ_LIVE_AI").as_deref() != Ok("1") {
        eprintln!("skipped: set MERGEIQ_LIVE_AI=1 to run live provider tests");
        return false;
    }
    true
}

/// A file long enough (about 8k tokens over its three versions) for prompt caching to apply,
/// with two independent conflicts.
fn input(chunk_line: u32) -> ContextInput {
    let mut base = String::new();
    for i in 0..250 {
        base.push_str(&format!(
            "/// Returns the price for tier {i}.\nfn price_{i}(units: u32) -> u32 {{\n    units * {i}\n}}\n\n"
        ));
    }
    let lines: Vec<&str> = base.split_inclusive('\n').collect();
    let edit = |text: &mut String, line: u32, new: &str| {
        let mut parts: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
        parts[line as usize] = format!("    {new}\n");
        *text = parts.concat();
    };
    let mut left = base.clone();
    let mut right = base.clone();
    for line in [102u32, 202] {
        edit(&mut left, line, "units * 3 + 1 // left: add a flat fee");
        edit(
            &mut right,
            line,
            "units.saturating_mul(3) // right: avoid overflow",
        );
    }
    let span = Span {
        start: chunk_line,
        end: chunk_line + 1,
    };
    assert!(lines.len() > 1000);
    ContextInput {
        path: "src/pricing.rs".into(),
        base: base.clone(),
        left: SideInput {
            label: "main".into(),
            text: left,
            commits: vec![],
        },
        right: SideInput {
            label: "feature/overflow".into(),
            text: right,
            commits: vec![],
        },
        result: base,
        chunk: ChunkSpans {
            base: span,
            left: span,
            right: span,
            result: span,
        },
    }
}

fn show(label: &str, s: &Suggestion) {
    eprintln!(
        "{label}: {:?} confidence, strategy {:?}\n--- resolution ---\n{}--- explanation ---\n{}\n--- risks ---\n{:?}",
        s.confidence, s.strategy, s.resolution, s.explanation, s.risks
    );
}

#[tokio::test]
#[ignore = "live provider test"]
async fn anthropic_suggests_and_reuses_the_cache_on_the_second_chunk() {
    if !enabled() {
        return;
    }
    let Ok(key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipped: ANTHROPIC_API_KEY is not set");
        return;
    };
    let provider = AnthropicProvider::new(ProviderConfig {
        kind: ProviderKind::Anthropic,
        base_url: "https://api.anthropic.com".into(),
        model: std::env::var("MERGEIQ_LIVE_ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-opus-5".into()),
        effort: Effort::Medium,
        api_key: Some(Secret::new(key)),
    });
    provider.test().await.expect("test connection");

    let settings = ContextSettings::default();
    let cancel = CancellationToken::new();
    let first = build(&input(102), Task::Suggest, &settings);
    let second = build(&input(202), Task::Suggest, &settings);
    assert!(
        first.notes.is_empty(),
        "the file fits the budget: {:?}",
        first.notes
    );
    assert_eq!(first.prompt.file_section, second.prompt.file_section);

    let a = provider
        .suggest(&first.prompt, &cancel)
        .await
        .expect("first suggestion");
    show("first", &a.value);
    eprintln!("first usage: {:?}", a.usage);
    let b = provider
        .suggest(&second.prompt, &cancel)
        .await
        .expect("second suggestion");
    show("second", &b.value);
    eprintln!("second usage: {:?}", b.usage);

    assert!(!a.value.resolution.contains("<<<<<<<"));
    assert!(
        b.usage.cache_read_tokens > 0,
        "the second request should read the first one's cache: {:?}",
        b.usage
    );
}

#[tokio::test]
#[ignore = "live provider test"]
async fn ollama_explains_and_suggests() {
    if !enabled() {
        return;
    }
    let url = std::env::var("MERGEIQ_LIVE_OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434".into());
    let provider = OllamaProvider::new(ProviderConfig {
        kind: ProviderKind::Ollama,
        base_url: url,
        model: std::env::var("MERGEIQ_LIVE_OLLAMA_MODEL")
            .unwrap_or_else(|_| "qwen2.5-coder".into()),
        effort: Effort::High,
        api_key: None,
    });
    if provider.test().await.is_err() {
        eprintln!("skipped: no Ollama server with that model is reachable");
        return;
    }
    let cancel = CancellationToken::new();
    let built = build(&input(102), Task::Suggest, &ContextSettings::default());
    let s = provider
        .suggest(&built.prompt, &cancel)
        .await
        .expect("suggestion");
    show("ollama", &s.value);
    eprintln!("usage: {:?}", s.usage);

    let explain = build(&input(102), Task::Explain, &ContextSettings::default());
    let mut streamed = String::new();
    let mut on = |t: &str| streamed.push_str(t);
    let e = provider
        .explain(&explain.prompt, &mut on, &cancel)
        .await
        .expect("explanation");
    assert_eq!(e.value, streamed);
    eprintln!("explanation: {}", e.value);
}

/// Offline guard for the inputs above: they fit the budget and share one cacheable prefix.
#[test]
fn the_live_fixture_fits_the_budget_and_shares_its_prefix() {
    let settings = ContextSettings::default();
    let a = build(&input(102), Task::Suggest, &settings);
    let b = build(&input(202), Task::Suggest, &settings);
    assert!(a.notes.is_empty(), "{:?}", a.notes);
    assert!(!a.over_budget);
    assert_eq!(a.prompt.file_section, b.prompt.file_section);
    assert_ne!(a.prompt.chunk_section, b.prompt.chunk_section);
    // Enough for prompt caching to apply (at least 4096 tokens), well under the budget.
    assert!(
        a.estimated_tokens > 6_000 && a.estimated_tokens < 40_000,
        "{}",
        a.estimated_tokens
    );
    assert!(a.prompt.chunk_section.contains("units * 20"));
    assert!(b.prompt.chunk_section.contains("units * 40"));
}
