## Why

Many conflicts are false: both sides added different imports, added different methods at the end of a class, or edited different keys in a JSON/YAML object. Line-based diff3 (and IntelliJ's magic wand, except for Java imports) reports these as conflicts. A syntax-aware merge for the user's main languages (Java, Python, Kotlin, YAML, JSON, JavaScript, TypeScript, Go) removes most of this busywork and is a clear advantage over IntelliJ and VS Code.

Depends on: `merge-engine`, `merge-editor-ui` (extension hook).

## What Changes

- Implement `mergeiq-struct` crate using tree-sitter:
  - Parse base/ours/theirs per language.
  - Data-driven "entry" definitions per language (tree-sitter queries) describing keyed, order-insensitive-ish containers (imports, class members, top-level declarations, JSON/YAML mapping keys).
  - Keyed 3-way merge of entries for each line-level `Conflict` chunk, recursing into nested containers, producing a proposed resolution that may span adjacent chunks.
  - Import merging (union with dedupe, sorted insertion when the block is sorted, merging Python `from x import a, b` and JS/TS named specifiers).
  - Output validation: proposed result must parse with no more syntax errors than the inputs.
- Editor integration via `MergeEditorExtension`: per-chunk suggestion with preview, toolbar "Resolve structurally (N)", resolution kind `structural`.

## Capabilities

### New Capabilities
- `structural-merge`: Syntax-aware resolution proposals for conflict chunks in the eight supported languages.

### Modified Capabilities
<!-- none -->

## Impact

- New code in `crates/mergeiq-struct`, IPC command `structural_resolve`, UI extension in `apps/desktop/src/merge-editor/extensions/structural/`.
- Dependencies: `tree-sitter`, `tree-sitter-java`, `tree-sitter-python`, `tree-sitter-kotlin-ng`, `tree-sitter-yaml`, `tree-sitter-json`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-go` (all MIT-compatible). Binary size grows by a few MB.
- Note: mergiraf (GPL-3.0) may be studied for ideas but no code copied, to stay Apache-2.0.
