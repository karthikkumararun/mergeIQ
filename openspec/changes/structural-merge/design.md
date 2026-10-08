## Context

Full AST merging (GumTree matching, as mergiraf does) is powerful but complex. Most real false conflicts are in keyed, mostly order-insensitive containers: import blocks, class bodies, top-level declarations, config maps. A keyed-entry merge targeted at line-level conflict chunks gives most of the value at a fraction of the complexity, and it can only make things better because proposals are optional and validated.

## Goals / Non-Goals

**Goals:**
- High-precision proposals (never produce wrong merges silently; user always previews or explicitly bulk-applies).
- Data-driven language support via queries.

**Non-Goals:**
- Statement-level merging inside function bodies (left to line/token merge and AI).
- Move/rename detection of members.
- Auto-applying without user action.

## Decisions

**Module layout (`crates/mergeiq-struct/src/`)**
```
lang.rs        Language enum, extension mapping, grammar + query loading (queries embedded with include_str!)
parse.rs       parse three versions; error-node counting
entries.rs     run entries.scm → Container { span, children: Vec<Entry { key, span, is_container }>, separator: Option<Sep> }
keyed3.rs      3-way merge of keyed entry lists (pure, generic over key type), recursion
imports.rs     import-specific union/sort/name-list merge
render.rs      assemble text from spans, separators, indentation
propose.rs     per-chunk: locate container in all 3 versions (via line→byte mapping from mergeiq-core Analysis), merge, compute covered ranges, validate
```

**Query captures convention.** `@container` (node whose named children are entries), `@entry`, `@entry.key` (node text or composed key), `@entry.container` (entry is itself a container to recurse into), `@sep` (separator token kind). Keys may be composite: for Java methods, key = name + `(` + parameter type list text normalized for whitespace.

**Locating containers for a chunk.** Map chunk base line range to byte range; find innermost `@container` whose span strictly contains it in base; find corresponding container in ours/theirs by matching container key path (chain of entry keys from root). If not found in any version → unresolvable.

**Position of additions.** Entries' order derived from base order; additions anchored after their preceding base entry (by key) in their side. When both sides anchor additions after the same entry: ours' then theirs'. Sorted mode detection: keys sorted lexicographically (case-sensitive) in all three sides.

**Covered ranges.** Proposal replaces the full container-children span from the first changed entry to the last changed entry (in base coordinates), expanded to whole lines. The editor maps that to result coordinates using chunk mappings: allowed only if all intersected chunks are unresolved, so result text between them still equals base.

**IPC.** `structural_resolve(analysisId | {base, ours, theirs, path})` → `Vec<StructuralProposal { chunk_ids, base_range, text, explanation }>`; run on a blocking thread pool with timeout.

**Editor extension.** Uses `MergeEditorExtension` from `merge-editor-ui`: `chunkActions` adds the indicator; `toolbarItems` adds bulk action; applies via a new `applyReplacement(chunkIds, text, kind='structural')` session action (add to session model if missing).

**Grammar versions.** Pin grammar crates; verify they build on Windows MSVC in CI.

## UI reference

Approved screen: `ui/Structural.dc.html`; how to read it and precedence rules: `openspec/UI.md`.

- Shown with only the Result pane for focus; in the app it is the normal three-pane editor.
- Toolbar item "Resolve structurally (N)" uses the info style (`--mod-bg` fill, `--mod-fg`-tinted border) with a small status text after it ("Proposals ready · computed in N ms"; while running: "Finding structural merges…"; on timeout: nothing shown).
- Chunk indicator: a 20×20 "S" button in the Result gutter on the first line of each chunk with a proposal, `aria-label="Show structural proposal"`. The chunk outline uses `--mod-fg` when its proposal is open.
- Preview popover (anchored to the chunk; drawn docked to the right in the mock): title "Structural proposal · `<container key>`", one-paragraph explanation from `StructuralProposal.explanation`, a −/+ diff of current result range vs proposal with token emphasis, a footer line ("Covers N conflicts", "Validated: parses without errors"), then Dismiss and Apply proposal (primary).
- Chunks without a proposal get no indicator; the footer legend in the mock is optional.

## Risks / Trade-offs

- [Wrong key identity (overloads, decorators)] → composite keys; unresolvable when ambiguous (duplicate keys in a container → skip container).
- [Comments between entries attached to wrong entry] → leading comments/blank lines belong to the following entry (span extended upward to previous entry end).
- [Kotlin grammar maturity] → treat Kotlin parse errors as unsupported for that file.
- [YAML anchors/aliases, multi-docs] → skip containers containing anchors; handle each document separately.

## Open Questions

- Should magic wand include structural proposals? Default no; add a setting later based on feedback.
