## Why

After line, token and structural merging, the conflicts left are ones where both sides changed the same logic. Resolving them needs understanding of intent: what each side was trying to do, according to its code and commit messages. LLMs (Claude, OpenAI Codex models, GitHub Models, local Ollama) are good at this. MergeIQ should offer AI explanations and suggested resolutions that are always previewed and never auto-applied, with the user's own API keys and clear control over what leaves the machine.

Depends on: `merge-engine`, `git-adapter`, `merge-editor-ui` (extension hook), `structural-merge` (syntax validation reuse).

## What Changes

- Implement `mergeiq-ai` crate:
  - `AiProvider` trait with adapters: Anthropic (Messages API over HTTPS), OpenAI, GitHub Models, Ollama (local).
  - Context builder: chunk texts, surrounding code, whole-file sides (budgeted), commit messages per side, language, path.
  - Two tasks: **Explain** (streamed prose) and **Suggest** (structured JSON resolution).
  - Output validation (schema + syntax check via tree-sitter when language is supported).
  - Usage/cost accounting per request and per session.
- Secrets in OS keychain; privacy controls (per-repo opt-in, path exclusion globs, preview of payload).
- Editor integration: per-chunk Explain / Suggest, suggestion preview diff with Apply / Dismiss / Regenerate, "Suggest all remaining" queue reviewed one by one; resolution kind `ai`.
- Settings UI: provider, model, effort/temperature where applicable, keys, privacy.

## Capabilities

### New Capabilities
- `ai-assist`: AI-powered conflict explanation and resolution suggestions with provider abstraction, privacy controls and validation.

### Modified Capabilities
<!-- none -->

## Impact

- New crate `crates/mergeiq-ai`; IPC commands `ai_explain`, `ai_suggest`, `ai_cancel`, `ai_settings_*`; UI under `apps/desktop/src/merge-editor/extensions/ai/` and `src/settings/ai/`.
- Dependencies: `reqwest` (rustls, stream), `eventsource-stream`, `keyring`, `jsonschema`, `globset`.
- Network egress to user-selected providers only; no telemetry.
