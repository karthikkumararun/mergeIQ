## 1. Core abstractions

- [x] 1.1 `provider.rs` trait + error enum; `mock.rs` fixture provider; unit tests
- [x] 1.2 `schema.rs` Suggestion schema + validation; tests for valid/invalid payloads
- [x] 1.3 `context.rs` builder with sections, token estimate, budget trimming order; tests per trimming stage and exclusion globs
- [x] 1.4 `prompts.rs` static system prompts for explain/suggest with data-not-instructions framing; snapshot tests ensuring prefix stability (no volatile content)

## 2. Secrets, privacy, usage

- [x] 2.1 `secrets.rs` keyring wrapper (service `dev.mergeiq.app`); log redaction layer for auth headers; tests
- [x] 2.2 Privacy settings: per-repo opt-in store, exclusion globs (defaults), payload preview IPC
- [x] 2.3 `usage.rs` accounting + editable price table; tests

## 3. Providers

- [x] 3.1 `anthropic.rs`: request builder (system, cache_control breakpoint, adaptive thinking, effort, output_config.format json_schema, fallbacks for claude-opus-5), SSE parser, stop_reason handling (refusal/max_tokens), retries with retry-after; tests against recorded SSE fixtures
- [x] 3.2 `ollama.rs` with JSON schema format; tests with fixtures
- [x] 3.3 `openai.rs` and `github.rs` adapters with structured output or JSON mode + validation; fixture tests
- [x] 3.4 Model capability table (effort, fallbacks, structured output support)

## 4. IPC and UI

- [x] 4.1 IPC `ai_explain` (event-streamed), `ai_suggest`, `ai_cancel`, `ai_test_connection`, settings commands; TS bindings
- [x] 4.2 Settings UI: provider form with test connection, key entry (masked), privacy form, usage table; first-use data-sharing notice
- [x] 4.3 Editor extension: chunk actions, AiPanel streaming explanation, SuggestionPreview with diff/confidence/risks/syntax warning, Apply/Dismiss/Regenerate (undoable via `applyReplacement` kind `ai`)
- [x] 4.4 "Suggest remaining" queue with concurrency 3, pre-flight token estimate, review stepper
- [x] 4.5 Playwright tests with mock provider: explain stream + cancel, suggest preview + apply + undo, review queue, excluded file refusal, repo opt-out

## 5. Manual verification

- [ ] 5.1 Live smoke test script (`MERGEIQ_LIVE_AI=1`) for Anthropic and Ollama verifying a real suggestion and cache read on second chunk — written in `crates/mergeiq-ai/tests/live.rs` and compiled, but not yet run against a real provider

## 6. UI fidelity

- [x] 6.1 Match `ui/AiAssist.dc.html` (panel tabs, suggestion layout, confidence/strategy chips, syntax warning, usage footer) and `ui/AiSettings.dc.html` (provider form, masked key, privacy chips, per-repo list) using `--ai-*` tokens; build the undrawn states listed in design.md › UI reference in the same style; Playwright screenshots with the mock provider
