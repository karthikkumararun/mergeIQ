# merge-engine Specification

## Purpose
Pure 3-way merge analysis for MergeIQ: text decoding, line-ending-preserving splitting, histogram-diff chunk classification, word-level fine diffing, simple-conflict auto-resolution, conflict-marker parsing, and result serialization. No IO, no git, no UI.

## Requirements

### Requirement: Text decoding and binary detection
The engine SHALL decode input bytes as UTF-8 (optional BOM) or UTF-16 LE/BE (BOM required). Input containing a NUL byte in the first 8000 bytes (after BOM handling for UTF-16) or failing to decode SHALL be classified as binary and rejected with `MergeError::Binary`. The detected encoding and BOM presence SHALL be reported and reused when serializing.

#### Scenario: UTF-8 with BOM round-trips
- **WHEN** base, ours, theirs are UTF-8 with BOM and no changes are made
- **THEN** serialized result bytes equal base bytes exactly

#### Scenario: Binary input rejected
- **WHEN** any input contains a NUL byte in its first 8000 bytes and is not UTF-16
- **THEN** analysis returns `MergeError::Binary { side }`

### Requirement: Line terminators preserved
The engine SHALL split text into lines keeping each line's original terminator (LF, CRLF, CR, or none for a final unterminated line). Unchanged and applied lines SHALL be emitted with their original terminators. The dominant terminator of the result SHALL be reported for use on user-typed lines.

#### Scenario: Mixed line endings preserved
- **WHEN** base uses CRLF and ours adds a line with CRLF and theirs adds a non-conflicting line with LF
- **THEN** after applying both changes the result contains each line with its original terminator

#### Scenario: Missing final newline
- **WHEN** base lacks a final newline and only theirs adds one
- **THEN** the change is classified as a theirs-only change on the last line

### Requirement: Chunk classification
The engine SHALL compute line-level diffs base→ours and base→theirs (histogram algorithm) and merge them into an ordered list of chunks covering every changed region. Each chunk SHALL have a stable `id` (index), ranges in base, ours and theirs (half-open line ranges), and a `kind`:
- `OursOnly` — only ours changed the region
- `TheirsOnly` — only theirs changed the region
- `BothSame` — both changed it identically (under the active whitespace policy)
- `Conflict` — both changed overlapping or adjacent regions differently
Changes from the two sides whose base ranges overlap, or which touch at the same insertion point, SHALL be combined into one `Conflict` chunk.

#### Scenario: Non-overlapping edits
- **WHEN** ours edits line 2 and theirs edits line 10 of a 20-line base
- **THEN** two chunks are produced: `OursOnly` at base 1..2 and `TheirsOnly` at base 9..10

#### Scenario: Identical edits
- **WHEN** ours and theirs both change line 5 from `a` to `b`
- **THEN** one chunk of kind `BothSame` is produced

#### Scenario: Overlapping edits
- **WHEN** ours changes lines 3–5 and theirs changes lines 5–7 differently
- **THEN** one `Conflict` chunk spanning base lines 3–7 is produced

#### Scenario: Insertions at the same point
- **WHEN** ours and theirs both insert different lines after base line 4
- **THEN** one `Conflict` chunk with an empty base range at line 4 is produced

#### Scenario: No changes
- **WHEN** ours and theirs equal base
- **THEN** chunk list is empty

### Requirement: Fine-grained diff within chunks
For every chunk the engine SHALL compute token-level differences of ours vs base and theirs vs base, where tokens are runs of word characters (Unicode alphanumeric or `_`), runs of whitespace, or single other characters. Fine ranges SHALL be reported as UTF-16 code unit offsets relative to the chunk's text on each side (matching JavaScript string indexing).

#### Scenario: Word change highlighted
- **WHEN** base line is `let total = sum(a, b);` and ours is `let total = sum(a, c);`
- **THEN** the ours fine diff marks only the token `c` as modified, and base marks only `b`

#### Scenario: Non-ASCII offsets
- **WHEN** a changed line contains `naïve 😀 x` and only `x` changes
- **THEN** reported offsets index `x` correctly in UTF-16 units

### Requirement: Whitespace policy
The engine SHALL accept a whitespace policy: `Exact`, `TrimTrailing`, `IgnoreAmount` (collapse runs), `IgnoreAll`. Line equality during diff SHALL use the policy. A side whose change is whitespace-only under the policy SHALL NOT produce a chunk for that side; if both sides only differ by whitespace the region SHALL be treated as unchanged and the base text kept.

#### Scenario: Trailing whitespace ignored
- **WHEN** policy is `TrimTrailing` and ours only adds trailing spaces to line 3 while theirs edits line 3's content
- **THEN** the region is a `TheirsOnly` chunk

#### Scenario: Exact policy
- **WHEN** policy is `Exact` and ours only adds trailing spaces to line 3 while theirs edits line 3's content
- **THEN** the region is a `Conflict` chunk

### Requirement: Resolve simple conflicts
For a `Conflict` chunk, the engine SHALL attempt a token-level 3-way merge of the chunk's base, ours and theirs text. If no token-level edits from the two sides overlap or touch, it SHALL return the merged text (`SimpleResolution::Resolved(text)`); otherwise `SimpleResolution::Unresolvable`. The engine SHALL also expose a whole-file operation returning resolutions for all conflict chunks.

#### Scenario: Different words on the same line
- **WHEN** base `foo(a, b)`, ours `foo(x, b)`, theirs `foo(a, y)`
- **THEN** result is `Resolved("foo(x, y)")`

#### Scenario: Same word changed differently
- **WHEN** base `foo(a)`, ours `foo(x)`, theirs `foo(y)`
- **THEN** result is `Unresolvable`

#### Scenario: Adjacent token edits are not merged
- **WHEN** base `ab cd`, ours changes `ab`→`AB`, theirs inserts text immediately after `ab` (touching)
- **THEN** result is `Unresolvable`

### Requirement: Conflict marker parsing
The engine SHALL parse a file containing git conflict markers (`<<<<<<<`, optional `|||||||` base section, `=======`, `>>>>>>>`) into base/ours/theirs texts plus labels from the marker lines. Marker lines SHALL be recognized only when the marker is at line start followed by space or end of line. For two-way markers (no base section) the base text for that region SHALL be empty and the parse result flagged `has_base = false`.

#### Scenario: diff3-style markers
- **WHEN** a file contains one region with ours, base and theirs sections
- **THEN** parsing yields three texts whose 3-way analysis produces one `Conflict` chunk at that region

#### Scenario: Malformed markers
- **WHEN** a `<<<<<<<` has no matching `>>>>>>>`
- **THEN** parsing returns `MergeError::MalformedMarkers { line }`

#### Scenario: Nested-looking content
- **WHEN** a line contains `x <<<<<<< y` not at line start
- **THEN** it is treated as content

### Requirement: Result serialization
The engine SHALL serialize a result from a list of lines with terminators, re-encoding with the original encoding and BOM. For unresolved chunks the caller SHALL choose to emit conflict markers (labels configurable, `diff3` style optional) or to receive `MergeError::Unresolved { chunk_ids }`.

#### Scenario: Unresolved chunks with markers
- **WHEN** serializing with one unresolved conflict and `emit_markers = true`
- **THEN** output contains `<<<<<<< <ours label>`, ours text, `=======`, theirs text, `>>>>>>> <theirs label>` at that location

#### Scenario: Unresolved chunks without markers
- **WHEN** serializing with unresolved conflicts and `emit_markers = false`
- **THEN** `MergeError::Unresolved` lists their ids

### Requirement: Performance
Analysis (decode, chunk, fine diff) SHALL complete within 100 ms for 10,000-line inputs with 50 chunks and within 1 s for 100,000-line inputs on a 2020-era laptop (release build). Fine diff SHALL be skipped (flagged `fine_diff_skipped`) for chunks larger than 2,000 lines per side.

#### Scenario: Large file benchmark
- **WHEN** the criterion benchmark runs a 100,000-line fixture
- **THEN** median time is under 1 s

### Requirement: Merge correctness invariants
For any inputs, the following SHALL hold: applying ours to every chunk (and theirs-only chunks as theirs) yields ours when theirs == base; accepting every non-conflicting chunk on a conflict-free merge equals `git merge-file` output; chunk ranges are ordered, non-overlapping and within bounds.

#### Scenario: Property test against git merge-file
- **WHEN** proptest generates random line edits for ours and theirs from a random base with no overlapping edits
- **THEN** result of applying all chunks equals `git merge-file -p` output
