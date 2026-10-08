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
 → always preview with diff, confidence, risks; syntax check; never bulk-apply.
- [Cost surprises] → pre-flight token estimate for bulk, session totals, per-request max_tokens 16000.
- [Provider API drift] → adapters isolated, contract tests against recorded fixtures; live tests behind env var `MERGEIQ_LIVE_AI=1` (manual only).
- [Sensitive code sent externally] → opt-in per repo, exclusion globs, payload preview, Ollama option.

## Open Questions

- Should we offer GitHub Copilot integration? There is no public third-party Copilot chat API for this use; GitHub Models covers GitHub-account-based access for now.
- Offer `claude-opus-5-5` in suggestions once generally available.
