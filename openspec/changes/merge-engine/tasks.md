## 1. Text foundation

- [x] 1.1 `error.rs`: `MergeError` (Binary{side}, Decode{side}, MalformedMarkers{line}, Unresolved{chunk_ids}) with thiserror
- [x] 1.2 `encoding.rs`: detect UTF-8/UTF-8 BOM/UTF-16 LE/BE BOM, NUL-based binary detection, encode back; tests for each encoding + binary
- [x] 1.3 `text.rs`: line splitting with terminators (LF/CRLF/CR/None), dominant EOL; tests incl. lone CR and missing final newline
- [x] 1.4 `tokenize.rs`: tokenizer + UTF-16 offset mapping; tests with emoji, combining marks, CJK

## 2. Line diff and diff3

- [x] 2.1 `diff.rs`: imara-diff histogram wrapper with policy-normalized interning (Exact, TrimTrailing, IgnoreAmount, IgnoreAll); unit tests per policy
- [x] 2.2 `diff3.rs`: sweep-merge of two hunk lists into chunks (OursOnly, TheirsOnly, BothSame, Conflict) incl. same-point insertion handling
- [x] 2.3 `analyze()` public entry assembling Analysis; unit tests for every scenario in spec "Chunk classification" and "Whitespace policy"
- [x] 2.4 Golden fixtures `tests/fixtures/<case>/{base,ours,theirs}` (22 cases incl. Java, Python, YAML, JSON, Kotlin, JS, TS, Go samples) with `insta` snapshot test runner (snapshots in `tests/snapshots/*.snap`, insta's standard mechanism, instead of a hand-rolled `expected.json` per case)

## 3. Fine diff and simple resolution

- [x] 3.1 `fine.rs`: token diff per chunk side vs base; skip above `fine_diff_max_lines`; tests from spec scenarios
- [x] 3.2 `simple.rs`: token-level diff3; Resolved/Unresolvable; precompute for Conflict chunks in `analyze`; tests from spec scenarios plus 10 realistic cases

## 4. Markers and serialization

- [x] 4.1 `markers.rs`: parse merge and diff3/zdiff3 markers, labels, `has_base`; malformed detection; tests
- [x] 4.2 `serialize.rs`: write ResultLines with encoding/BOM, optional markers (merge or diff3 style, configurable labels); tests for round-trip byte equality

## 5. Quality gates

- [x] 5.1 proptest: random base + non-overlapping edits; compare with `git merge-file -p` (test skipped if git missing); invariants on chunk ordering/bounds
- [x] 5.2 criterion benchmarks for 10k and 100k line fixtures; assert budgets in a CI-friendly smoke bench
- [x] 5.3 `specta` feature: derive `Type` on all public types; doc comments on every public item; `cargo doc` without warnings
