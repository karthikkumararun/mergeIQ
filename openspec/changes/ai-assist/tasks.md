## 1. Core abstractions

- [ ] 1.1 `provider.rs` trait + error enum; `mock.rs` fixture provider; unit tests
- [ ] 1.2 `schema.rs` Suggestion schema + validation; tests for valid/invalid payloads
- [ ] 1.3 `context.rs` builder with sections, token estimate, budget trimming order; tests per trimming stage and exclusion globs
- [ ] 1.4 `prompts.rs` static system prompts for explain/suggest with data-not-instructions framing; snapshot tests ensuring prefix stability (no volatile content)

## 2. Secrets, privacy, usage

- [ ] 2.1 `secrets.rs` keyring wrapper (service `dev.mergeiq.app`); log redaction layer for auth headers; tests
- [ ] 2.2 Privacy settings: per-repo opt-in store, exclusion globs (defaults), payload preview IPC
- [ ] 2.3 `usage.rs` accounting + editable price table; tests

## 3. Providers

- [ ] 3.1 `anthropic.rs`: request builder (system, cache_control breakpoint, adaptive thinking, effort, output_config.format json_schema, fallbacks for claude-opus-5), SSE parser, stop_reason handling (refusal/max_tokens), retries with retry-after; tests against recorded SSE fixtures
- [ ] 3.2 `ollama.rs` with JSON schema format; tests with fixtures
- [ ] 3.3 `openai.rs` and `github.rs` adapters with structured output or JSON mode + validation; fixture tests
- [ ] 3.4 Model capability table (effort, fallbacks, structured output support)

## 4. IPC and UI

- [ ] 4.1 IPC `ai_explain` (event-streamed), `ai_suggest`, `ai_cancel`, `ai_test_connection`, settings commands; TS bindings
- [ ] 4.2 Settings UI: provider form with test connection, key entry (masked), privacy form, usage table; first-use data-sharing notice
- [ ] 4.3 Editor extension: chunk actions, AiPanel streaming explanation, SuggestionPreview with diff/confidence/risks/syntax warning, Apply/Dismiss/Regenerate (undoable via `applyReplacement` kind `ai`)
- [ ] 4.4 "Suggest remaining" queue with concurrency 3, pre-flight token estimate, review stepper
- [ ] 4.5 Playwright tests with mock provider: explain stream + cancel, suggest preview + apply + undo, review queue, excluded file refusal, repo opt-out

## 5. Manual verification

- [ ] 5.1 Live smoke test script (`MERGEIQ_LIVE_AI=1`) for Anthropic and Ollama verifying a real suggestion and cache read on second chunk

## 6. UI fidelity

- [ ] 6.1 Match `ui/AiAssist.dc.html` (panel tabs, suggestion layout, confidence/strategy chips, syntax warning, usage footer) and `ui/AiSettings.dc.html` (provider form, masked key, privacy chips, per-repo list) using `--ai-*` tokens; build the undrawn states listed in design.md › UI reference in the same style; Playwright screenshots with the mock provider
