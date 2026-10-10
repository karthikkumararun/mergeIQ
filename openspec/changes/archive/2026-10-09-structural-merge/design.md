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
propose.rs     merge the root containers of the 3 versions, diff against base, grow hunks to chunk boundaries, validate (see "Merging from the roots")
```

**Query captures convention.** `@container` (node whose named children are entries; its interior is the text between its brackets, or its whole lines when undelimited), `@entry` (a direct child of a container), `@entry.key` (nodes whose text is joined, in source order, into the key; several captures and several matches of one entry node accumulate), `@import` (on the entry node: set-union semantics), `@sep` (separator token that is a direct child of a container) and `(#set! entry.kind "label")` (replaces the node kind as key prefix so `export function f` and `function f` share an identity). Keys may be composite: for Java methods, key = name + the parameter types, whitespace-normalised. Named children no pattern claims become *opaque* entries keyed by kind and first line, so nothing in a container is silently dropped.

**Merging from the roots (as built).** Instead of locating a container per chunk, `propose.rs` merges the *root* container of each file (a module, a program, a top-level JSON object, each YAML document) and recurses into entries both sides changed (`render.rs`); the nested container of an entry is the single container found inside it, so `@entry.container` is not needed. Entries that cannot be combined (changed by both and not recursable, deleted vs changed, opaque entries deleted by both) become *protected* base ranges and keep their base text in the merged output, so one real conflict never costs the imports or other members around it. A container is only abandoned wholesale for structural reasons (duplicate keys, reordered entries, mixed separators, comments or tokens in unexpected places, YAML anchors); a container nested in an abandoned one is never merged on its own. The merged file is line-diffed against the base and each hunk becomes a candidate proposal.

**Position of additions.** Entries' order derived from base order; additions anchored after their preceding base entry (by key) in their side. When both sides anchor additions after the same entry: ours' then theirs'. Sorted mode detection: keys sorted lexicographically (case-sensitive) in all three sides.

**Covered ranges.** A hunk is grown to whole-chunk boundaries (chunks that overlap it, insertion points included; chunks that merely touch it are left alone) and hunks that end up overlapping are joined. The candidate is dropped unless it covers at least one `Conflict` chunk, touches no protected range, and the base with the proposal applied parses with no more ERROR/MISSING nodes than the worst of the three inputs. The editor maps the base line range to Result coordinates from the covered chunks' positions; unchanged base lines around them are identical in the Result, and chunks that are only half applied are simply overwritten, which the preview shows.

**IPC.** `structural_resolve(path, analysis)` takes the `Analysis` the editor already holds (so chunk ids always match, also after a whitespace re-analysis) and returns `{ outcome: Unsupported | TimedOut | Proposals(Vec<Proposal>), elapsed_ms }`. It runs on the blocking pool with a 2 s deadline that `propose_until` checks between phases; a timeout is logged and shows nothing. A `Proposal` carries `chunk_ids`, base/ours/theirs line ranges, `text`, `explanation` and `container`. The explanation says "Left"/"Right" (never ours/theirs) and is generated in the engine.

**Editor extension.** Built into `MergeEditor` (so the repository window, the CLI request window and the dev host all get it) on top of `MergeEditorExtension`: `toolbarItems` (the bulk action and status text, which also starts the background computation), `resultExtensions` (a CodeMirror gutter with the "S" markers; two small additions to the hook) and `resultOverlay` (the preview popover; also new). Applying goes through `applyReplacements(chunkIds, baseRange, text, kind)` in the session model, which resolves the chunks with kind `structural` as one undoable transaction. CodeMirror hides its gutter container from assistive technology, so the extension exposes it and hides only the line numbers.

**Grammar versions.** Pin grammar crates; verify they build on Windows MSVC in CI. The Kotlin grammar fails on some valid constructs (e.g. single-line `object` bodies): if any of the three Kotlin versions has parse errors the file is `Unsupported`.

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
