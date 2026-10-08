## Why

The merge editor is only as good as the 3-way analysis behind it. IntelliJ's editor feels intuitive because it classifies every change (non-conflicting left, non-conflicting right, identical on both sides, true conflict), highlights at word level, and can auto-resolve "simple" conflicts whose edits don't overlap at word granularity. MergeIQ needs a pure, well-tested Rust engine that does this, independent of UI and git, so it can be reused by the editor, the CLI, structural merge and AI features.

Depends on: `bootstrap-app`.

## What Changes

- Implement `mergeiq-core` crate:
  - Text decoding: UTF-8 (with/without BOM), UTF-16 LE/BE (BOM); binary detection.
  - Line splitting preserving original line terminators (LF, CRLF, CR) and missing final newline.
  - Line-level 3-way merge (diff3) using histogram diff producing typed chunks.
  - Word-level (token) fine diff inside each changed chunk for highlighting.
  - "Resolve simple conflicts" (magic wand): token-level 3-way merge of a conflict chunk; succeeds only when token edits don't overlap.
  - Whitespace comparison policies.
  - Conflict marker parser (merge and diff3/zdiff3 style) to rebuild base/ours/theirs from a marked file.
  - Result serializer with configurable line ending and optional conflict markers for unresolved chunks.
- All public types serializable (serde) and exportable to TS (specta, behind feature `specta`).
- Golden fixture test suite plus property tests.

## Capabilities

### New Capabilities
- `merge-engine`: Pure 3-way merge analysis — decoding, chunk classification, fine-grained diff, simple-conflict auto-resolution, marker parsing, result serialization.

### Modified Capabilities
<!-- none -->

## Impact

- New code in `crates/mergeiq-core` only. No IO, no async, no Tauri dependency.
- Dependencies: `imara-diff`, `serde`, `thiserror`, `encoding_rs`; optional `specta`; dev: `proptest`, `insta`.
- Consumed by `git-adapter`, `merge-editor-ui` (via IPC), `structural-merge`, `ai-assist`.
