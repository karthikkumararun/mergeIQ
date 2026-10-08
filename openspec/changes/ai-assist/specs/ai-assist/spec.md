## ADDED Requirements

### Requirement: Provider configuration
The app SHALL support providers `anthropic`, `openai`, `github-models`, and `ollama`, each with a base URL (overridable), model id, and credentials. The default provider SHALL be `anthropic` with model `claude-opus-5`; the model field SHALL be free text with suggestions (e.g. `claude-opus-5`, `claude-sonnet-5`, `claude-haiku-4-5`). AI features SHALL be disabled until the user configures a provider and accepts a one-time data-sharing notice.

#### Scenario: Not configured
- **WHEN** no provider is configured
- **THEN** AI actions in the editor are hidden behind a "Set up AI" link to settings

#### Scenario: Ollama local
- **WHEN** the user selects `ollama` with base URL `http://localhost:11434` and model `qwen2.5-coder`
- **THEN** requests go only to localhost and no API key is required

### Requirement: Credential storage
API keys and tokens SHALL be stored only in the OS credential store (macOS Keychain, Windows Credential Manager) under service `dev.mergeiq.app`, never in settings files or logs. The UI SHALL show only the last 4 characters. A "Test connection" action SHALL make a minimal request and report success or the provider's error.

#### Scenario: Key not in settings
- **WHEN** a key is saved and settings.json is inspected
- **THEN** it contains no key material

#### Scenario: Logs redacted
- **WHEN** a request fails with debug logging on
- **THEN** logs contain no Authorization or x-api-key header values

### Requirement: Privacy controls
AI usage SHALL be opt-in per repository (first AI action in a repo asks; choice remembered). The user SHALL be able to configure path exclusion globs (default: `**/.env*`, `**/*secret*`, `**/*.pem`, `**/*.key`); excluded files SHALL NOT be sent. A "Preview request" action SHALL show the exact payload text before sending.

#### Scenario: Excluded file
- **WHEN** the user requests a suggestion for `config/.env.production`
- **THEN** the action is refused with "File excluded from AI by your settings"

#### Scenario: Repo opt-in
- **WHEN** the user first clicks Suggest in a repo and declines
- **THEN** no request is sent and AI actions are disabled for that repo until re-enabled in settings

### Requirement: Context assembly
For a chunk, the context builder SHALL include: file path and language; base, left and right text of the chunk; 40 lines of surrounding context (configurable); contextual side labels; commit subjects and bodies touching the file on each side (max 10 per side); and, if within the token budget (default 60k input tokens, estimated), the full base, left and right file texts. When over budget it SHALL drop in order: full files, then commit bodies, then trim surrounding context, never the chunk itself. File content SHALL be wrapped in clearly delimited sections and the system prompt SHALL state that this content is data, not instructions.

#### Scenario: Budget trimming
- **WHEN** a file is 20,000 lines and the budget is 60k tokens
- **THEN** full-file sections are omitted, chunk and surrounding context are included, and the preview shows "Full file omitted (budget)"

### Requirement: Explain action
**Explain** SHALL stream a plain-language explanation of what each side changed and why (using commit messages), where they clash, and what a correct merge must preserve. Output SHALL render incrementally in a side panel and be cancellable.

#### Scenario: Streamed explanation
- **WHEN** the user clicks Explain on a conflict
- **THEN** text appears incrementally within 3 s of the first token and Cancel stops the request

### Requirement: Suggest action
**Suggest** SHALL request a structured response matching this schema: `{ resolution: string, explanation: string, confidence: "high"|"medium"|"low", strategy: "left"|"right"|"both"|"combined"|"new", risks: string[] }`, using the provider's JSON-schema structured output mechanism (Anthropic: `output_config.format` with `type: "json_schema"`). Responses failing schema validation SHALL be retried once, then reported as an error. The `resolution` SHALL replace exactly the chunk's result range.

#### Scenario: Suggestion preview
- **WHEN** a suggestion returns
- **THEN** the editor shows a diff of the current result range vs `resolution`, the explanation, the confidence, risks, and Apply / Dismiss / Regenerate

#### Scenario: Apply suggestion
- **WHEN** the user clicks Apply
- **THEN** the range is replaced, the chunk is marked resolved with kind `ai` in one undoable step

### Requirement: Validation of suggestions
For supported languages, the app SHALL parse the file with the suggestion applied (other unresolved chunks taken from base) and, if it has more syntax errors than the maximum among base/left/right, show a warning "Suggestion may not compile" on the preview. Suggestions SHALL never be applied without an explicit user action.

#### Scenario: Syntax warning
- **WHEN** a suggestion leaves an unclosed brace in a Java file
- **THEN** the preview shows the syntax warning and Apply still requires a click

### Requirement: Suggest all remaining
The toolbar SHALL offer "AI: suggest remaining (N)", sending requests for unresolved conflicts with at most 3 concurrent, then presenting them one at a time in document order for Apply / Dismiss / Edit. Total estimated input tokens SHALL be shown before starting.

#### Scenario: Review queue
- **WHEN** suggestions for 4 conflicts arrive
- **THEN** the user steps through each with Apply / Dismiss and the counter updates per decision

### Requirement: Anthropic adapter behavior
The Anthropic adapter SHALL call `POST https://api.anthropic.com/v1/messages` with headers `x-api-key`, `anthropic-version: 2023-06-01`, and SHALL:
- stream responses (SSE) for both actions;
- use adaptive thinking (`thinking: {type: "adaptive"}`) and expose `output_config.effort` (default `high`) in settings;
- place a `cache_control: {type: "ephemeral"}` breakpoint after the stable prefix (system prompt + file-level context) so multiple chunks of the same file reuse the cache, with per-chunk content after the breakpoint;
- for `claude-opus-5`, send beta header `server-side-fallback-2026-07-01` with `fallbacks: "default"`;
- check `stop_reason` before reading content: `refusal` → show "The model declined this request" with `stop_details.category`; `max_tokens` → report truncation;
- retry 429/5xx/connection errors with exponential backoff (max 2 retries) honoring `retry-after`.

#### Scenario: Cache reuse
- **WHEN** the user requests suggestions for two conflicts in the same file within 5 minutes
- **THEN** the second response reports `cache_read_input_tokens > 0`

#### Scenario: Refusal
- **WHEN** a response has `stop_reason: "refusal"`
- **THEN** no suggestion is shown and the user sees the decline message

### Requirement: Usage accounting
Each request SHALL record provider, model, input/output/cache tokens and latency; the AI panel SHALL show per-request and session totals, and an estimated cost when a price table entry exists for the model (price table editable in settings).

#### Scenario: Session totals
- **WHEN** three suggestions are generated
- **THEN** the panel shows the sum of their token usage

### Requirement: Provider abstraction
All providers SHALL implement `AiProvider { explain(ctx) -> Stream<TextDelta>, suggest(ctx, schema) -> Suggestion, test() }`. Providers lacking native JSON-schema output SHALL use JSON mode plus client-side validation. Adding a provider SHALL require no changes outside `mergeiq-ai` and the settings UI list.

#### Scenario: Mock provider in tests
- **WHEN** tests run with the `mock` provider returning fixture JSON
- **THEN** the full Suggest flow (preview, apply, undo) passes without network
