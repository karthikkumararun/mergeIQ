## Context

`mergeiq-core` is the heart of MergeIQ. It must be pure (no IO), deterministic, fast, and testable with fixtures. The interactive session (applying chunks, user edits, undo) lives in the UI because it runs per keystroke; the engine provides analysis and pure helper operations. See `merge-editor-ui` for the session model.

## Goals / Non-Goals

**Goals:**
- Chunk model matching IntelliJ semantics (non-conflicting left/right, identical, conflict).
- Byte-exact round-trip for untouched regions (encoding, BOM, line endings).
- Token-level highlighting and simple-conflict auto-resolution.

**Non-Goals:**
- Syntax-aware/structural merge (`structural-merge`).
- Git interaction (`git-adapter`).
- Non-text files (`special-conflicts`).

## Decisions

**Module layout (`crates/mergeiq-core/src/`)**
```
lib.rs          public API re-exports
text.rs         Decoded { text: String, encoding, bom, lines: Vec<Line> }; Line { start, end, term: Terminator }
encoding.rs     detect + decode + encode
tokenize.rs     word/whitespace/punct tokenizer, UTF-16 offset mapping
diff.rs         imara-diff wrapper -> Vec<Hunk { before: Range, after: Range }> with policy-normalized interning
diff3.rs        merges two hunk lists into Vec<Chunk>
fine.rs         token-level diff per chunk
simple.rs       token-level 3-way for magic wand
markers.rs      conflict marker parser
serialize.rs    result writer
error.rs        MergeError
```

**Public API (sketch)**
```rust
pub struct MergeInput<'a> { pub base: &'a [u8], pub ours: &'a [u8], pub theirs: &'a [u8] }
pub struct Options { pub whitespace: WhitespacePolicy, pub fine_diff_max_lines: usize }
pub fn analyze(input: MergeInput, opts: &Options) -> Result<Analysis, MergeError>;

pub struct Analysis {
    pub base: SideText, pub ours: SideText, pub theirs: SideText, // text + line table
    pub chunks: Vec<Chunk>,
    pub encoding: EncodingInfo, pub dominant_eol: Terminator,
}
pub struct Chunk {
    pub id: u32, pub kind: ChunkKind,
    pub base: LineRange, pub ours: LineRange, pub theirs: LineRange,
    pub fine: Option<FineDiff>,          // None if skipped
    pub simple: Option<SimpleResolution> // only for Conflict, precomputed
}
pub fn resolve_simple(base: &str, ours: &str, theirs: &str) -> SimpleResolution;
pub fn parse_markers(bytes: &[u8]) -> Result<MarkerParse, MergeError>;
pub fn serialize(lines: &[ResultLine], enc: &EncodingInfo, opts: &SerializeOptions) -> Result<Vec<u8>, MergeError>;
```
`LineRange` is half-open `[start, end)` in line indices; insertion = empty range.

**Diff algorithm: imara-diff histogram.** Same algorithm family as `git diff --histogram`; produces readable hunks for code, fast. Lines interned by normalized key (per whitespace policy) so policy is a pure interning concern.

**diff3 merge.** Classic sweep over two sorted hunk lists by base position. Hunks overlap or touch (`a.end >= b.start` with equal-point insertions counting as touching) → grow a combined region until stable → if only one side present: `OursOnly`/`TheirsOnly`; both present: compare ours slice vs theirs slice by normalized key → `BothSame` or `Conflict`. Matches git's `xdl_merge` default level conservatively (git merges adjacent non-overlapping hunks; we keep them as conflict only when they touch at the same point — validated against `git merge-file` in property tests, with documented deviations if any).

**Fine diff and simple resolve share the tokenizer.** Simple resolve = diff3 algorithm reused over token sequences instead of lines; "Resolved" only if zero conflict regions result. Touching edits → unresolvable (conservative, matches IntelliJ behavior).

**Offsets: UTF-16 for UI.** CodeMirror positions are JS string indices. Engine computes in bytes internally and maps to UTF-16 at the boundary via a per-chunk prefix table.

**Serialization types:** `ResultLine { text: String, term: Terminator }` — UI sends the final document back split into lines; user-typed lines use `dominant_eol`.

**Specta feature flag** keeps the core free of Tauri-specific deps while allowing TS type export.

## Risks / Trade-offs

- [Divergence from git's chunking on edge cases] → property tests vs `git merge-file` for conflict-free cases; golden fixtures for conflicts; document deviations.
- [Histogram diff produces surprising alignment for repetitive code (e.g. `}` lines)] → allow future `DiffAlgorithm::Myers` option; keep algorithm behind enum.
- [UTF-16 offset mapping bugs] → dedicated tests with emoji, combining marks, CJK.
- [Huge files] → fine-diff cap; analysis is O(N) memory in lines.

## Open Questions

- Should `IgnoreAll` policy also ignore blank-line insertions? Default: no (matches IntelliJ "Ignore whitespaces").
