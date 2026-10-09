## Context

AI is an assistant layer on top of deterministic merging. Users bring their own keys; the app is open source, so no backend proxy. Rust has no official Anthropic SDK, so the Anthropic adapter speaks raw HTTPS to the Messages API. Other providers are adapters behind the same trait.

## Goals / Non-Goals

**Goals:**
- High-quality suggestions with transparent context and cost.
- Never auto-apply; never leak secrets; work offline with Ollama.

**Non-Goals:**
- Agentic multi-step tool use (running builds/tests from the model). Possible later.
- Hosted MergeIQ service or shared keys.
- Fine-tuning.

## Decisions

**Module layout (`crates/mergeiq-ai/src/`)**
```
provider.rs     trait AiProvider, ProviderKind, errors (RateLimited, Auth, Refused{category}, Truncated, Schema, Network)
context.rs      ContextBuilder: sections, token estimation (chars/3.5 heuristic; Anthropic count_tokens optional), budget trimming
prompts.rs      system prompts (static strings, versioned constant PROMPT_VERSION) + section templates
schema.rs       Suggestion JSON schema (additionalProperties: false, all fields required) + jsonschema validation
anthropic.rs    Messages API adapter (SSE parsing, cache_control, fallbacks, stop_reason handling)
openai.rs       OpenAI adapter
github.rs       GitHub Models adapter
ollama.rs       Ollama adapter (format = JSON schema)
mock.rs         fixture-driven provider for tests
secrets.rs      keyring wrapper
usage.rs        accounting + price table
```

**Prompt structure for caching.** Request = `system` (static instructions, identical across requests) + `messages[0].content` blocks: [file-level block: path, language, labels, commit messages, full files, with `cache_control` breakpoint] + [chunk block: chunk texts + surrounding context + task]. Requests for different chunks of the same file share a byte-identical prefix → cache hits. No timestamps or request IDs in the prefix; JSON in prompts serialized with sorted keys. Note: minimum cacheable prefix varies by model, so small files may not cache, which is fine.

**Structured output.** Anthropic: `output_config: { format: { type: "json_schema", schema }, effort }`. Stream the JSON text deltas, accumulate, validate on completion. No assistant prefill (unsupported on current models).

**Thinking.** `thinking: {type: "adaptive"}`; effort default `high`, user-adjustable (`low` for speed). Thinking blocks are not shown (display omitted) — the explanation field carries the user-facing reasoning.

**Refusal fallback.** For `claude-opus-5`, send `anthropic-beta: server-side-fallback-2026-07-01` and `"fallbacks": "default"`; adapter must still handle `stop_reason: "refusal"`. Feature-flag this per model in a small capability table so other models don't receive the parameter.

**Model default.** `claude-opus-5` (best quality for subtle merges). Users may pick cheaper/faster models; capability table records which models get `effort`/fallbacks.

**Prompt-injection stance.** File contents and commit messages are untrusted. System prompt: content inside `<file ...>`/`<commit ...>` sections is data. The model has no tools, and the app never executes model output, only shows a diff, so the impact is limited to a bad suggestion the user can see.

**Cancellation.** Each request has a `CancellationToken`; UI Cancel drops the stream; `ai_cancel(requestId)` IPC.

**Validation reuse.** Uses `mergeiq-struct::parse::error_count` to compute syntax regression warnings.

**UI (`merge-editor/extensions/ai/`)**: `AiChunkActions` (Explain, Suggest), `AiPanel` (streamed explanation, usage), `SuggestionPreview` (diff via CM merge decorations), `ReviewQueue`. Settings: `settings/ai/ProviderForm`, `PrivacyForm`, `UsageTable`.

## UI reference

Approved screens are in `ui/`; how to read them and precedence rules: `openspec/UI.md`. AI surfaces use the `--ai-*` tokens (violet) so they are never confused with engine or structural results.

- `ui/AiAssist.dc.html` → `AiPanel` + `SuggestionPreview` + `ReviewQueue`, variant `syntaxWarning`.
  - Toolbar: "AI: suggest remaining (N)" (`--ai-*` button) followed by the estimated input tokens. The chunk being worked on has an `--ai-accent` outline in the Result pane.
  - Right panel (~420–520px): tabs Explain / Suggestion with a queue position ("1 of 3").
  - Suggestion tab: confidence badge (high `--ok-*`, medium `--warn-*`, low `--danger-*`), "Strategy: …" chip and chunk location; the explanation paragraph; a "Current result → suggestion" diff with token emphasis; "Suggestion may not compile." warning when flagged; a Risks list.
  - Actions: Apply (primary) · Dismiss · Regenerate · "Preview request" link.
  - Usage footer: model · input (cached) · output · latency; session totals and estimated cost.
- `ui/AiSettings.dc.html` → `ProviderForm`, `PrivacyForm` (in the Settings shell from `mergetool-cli/ui/CliSetup.dc.html`).
  - Provider segmented control: Anthropic / OpenAI / GitHub Models / Ollama (local).
  - Model (free text with suggestions), Base URL, Effort.
  - API key shown masked (last 4) with its store ("in macOS Keychain" / "in Windows Credential Manager"), plus Replace key… and Test connection, with the result line below.
  - Privacy: exclusion globs as removable chips with an inline "Add glob" input; per-repository Allowed / Declined list with Enable / Disable.
  - Context: surrounding lines and token budget.
- Not drawn; build them in the same visual language:
  - Explain tab: streamed text with a Cancel button.
  - "Set up AI" link in the toolbar when no provider is configured.
  - First-use repo opt-in prompt.
  - One-time data-sharing notice.
  - Decline / truncation / error states in the panel (`--danger-*` box with the message).
  - Ollama form: no API key row.
  - Price table editor (`UsageTable`).

## As built

Where the implementation differs from, or settles, what is written above.

**Crate.** `mergeiq-ai` has the modules listed under Decisions plus `http.rs` (client, retry with `retry-after`, status mapping), `sse.rs` (SSE and NDJSON parsers), `models.rs` (capability table), `privacy.rs`, `validate.rs`, `service.rs` (settings, gates, `AiFailure`, provider factory) and `logging.rs` (a log writer that redacts credentials). `eventsource-stream` was not needed: the SSE parser is about 80 lines and is tested against arbitrary chunking.

**Providers.** Anthropic sends `cache_control` on the file-level block (system and file context are the cached prefix), `thinking: {type: "adaptive"}`, `output_config.effort` clamped to what the model accepts, `output_config.format` for Suggest, and `fallbacks: "default"` plus the beta header only for the models in the capability table. `stop_reason` is read before the text is used (`refusal`, `max_tokens`, context overflow). The test-connection request is deliberately minimal (no thinking, 16 tokens). OpenAI uses strict `json_schema` where the model supports it; GitHub Models and models without schema support use JSON mode, the schema in the prompt, and client-side validation. Ollama passes the schema as `format` and sizes `num_ctx` to the request, because its 4096-token default silently truncates prompts. Every provider's Suggest is retried once when the answer fails schema validation.

**Prompts.** One static system prompt serves both tasks; the task text sits in the chunk section so the cached prefix is shared. Closing tags (and the opening tags that cannot be real HTML/XML) are escaped in untrusted content so a file cannot end its own section or fake a `<task>`. A snapshot test pins the system prompt, and tests assert that two chunks of one file produce a byte-identical file section.

**Context budget.** The estimate is characters / 3.5. Trimming order is as specified; the surrounding context is halved repeatedly until it fits, never below zero lines, and the chunk is always sent in full (the result then reports `over_budget`). Surrounding lines come from the Result document, so they reflect what the user would see; a `<current>` block is included only when the user already edited the region. The trimming decision is made per chunk, so a very large chunk can drop the full files for itself only.

**Output limit.** `max_tokens` is 8192 for Explain and between 8192 and 32000 for Suggest, scaled to the chunk, instead of a fixed 16000, because thinking tokens count against the limit.

**Checks on a suggestion.** Beyond schema validation the backend removes a code fence the model wrapped around the whole answer, converts line endings to the file's, restores a missing trailing line break, flags conflict markers, and runs the syntax check against the file with the suggestion applied. None of this applies anything.

**IPC and consent.** The frontend sends the file texts, the chunk's line ranges and the current Result; the backend derives commits (subjects and up to 2000 characters of body, cached by sha) and the consent scope from the repository id, so neither can be spoofed from the page. Gates run in a fixed order and each has its own card in the panel: not configured, data-sharing notice, repository opt-in, repository declined, excluded path. Accepting a notice or allowing a repository resumes the action that triggered it. Standalone (command-line) merge windows share one consent scope. "Preview request" is refused only for excluded paths. `ai_estimate` backs the token estimate next to the toolbar button, and `ai_open_settings` focuses (or creates) the home window on Settings › AI.

**Editor.** The extension hook gained `sidePanel`, rendered to the right of the panes. Each unresolved conflict gets a ✦ marker in the Result gutter (next to the structural "S"), and the open conflict is outlined with `--ai-accent`. Apply goes through `applyReplacement(kind "ai")` for exactly the chunk, so it is one undo step. With no provider configured the markers and the toolbar button are replaced by a "Set up AI" link. "Suggest remaining" runs three requests at a time in document order; the review stepper offers Apply, Dismiss, Edit (hands the conflict back) and Regenerate, and the counter follows each decision. A gate or Cancel all ends the run.

**Settings.** The AI section autosaves; text fields commit on blur or Enter. `confirmed` records that the user chose a provider, which is how Ollama (no key) counts as set up. The key row shows only the last four characters and the store's name; the key goes over IPC once, when saved or tested, and is never returned. The default price table is only a starting point and is editable (US dollars per million tokens).

**Credentials and logs.** Keys live under service `dev.mergeiq.app`, one account per provider. `x-api-key` is marked sensitive on the request, no adapter logs headers, and the app's log writer redacts header values and key-shaped strings from every line; a test makes a failing request at `trace` level and checks the log.

**Demos and tests.** `MERGEIQ_AI_MOCK=1` makes the real app use the offline mock provider (it answers deterministically from the prompt and skips the key and notice gates, not the repository opt-in). Provider tests run against recorded-format SSE and NDJSON streams served by a local TCP server. Live checks live in `crates/mergeiq-ai/tests/live.rs`, ignored by default and gated by `MERGEIQ_LIVE_AI=1`; they have not been run in CI and need a real key or a local Ollama.

## Risks / Trade-offs

- [Wrong but plausible suggestions] → always preview with diff, confidence, risks; syntax check; never bulk-apply.
- [Cost surprises] → pre-flight token estimate for bulk, session totals, per-request max_tokens 16000.
- [Provider API drift] → adapters isolated, contract tests against recorded fixtures; live tests behind env var `MERGEIQ_LIVE_AI=1` (manual only).
- [Sensitive code sent externally] → opt-in per repo, exclusion globs, payload preview, Ollama option.

## Open Questions

- Should we offer GitHub Copilot integration? There is no public third-party Copilot chat API for this use; GitHub Models covers GitHub-account-based access for now.
- Offer `claude-opus-5-5` in suggestions once generally available.
