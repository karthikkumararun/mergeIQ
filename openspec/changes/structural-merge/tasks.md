## 1. Parsing foundation

- [ ] 1.1 Add tree-sitter + 8 grammar crates; `lang.rs` extension mapping and grammar loading; CI build check on Windows/macOS/Linux
- [ ] 1.2 `parse.rs`: parse three versions, ERROR/MISSING node counting; tests
- [ ] 1.3 `entries.rs`: query capture convention and Container/Entry extraction; comment attachment rule; tests

## 2. Generic keyed merge

- [ ] 2.1 `keyed3.rs`: keyed 3-way merge with all rules from spec, recursion hook, duplicate-key bailout; exhaustive unit tests (table-driven)
- [ ] 2.2 `render.rs`: span assembly, separators with trailing-comma style, indentation adjustment; tests
- [ ] 2.3 `imports.rs`: union/dedupe, sorted detection and insertion, Python from-import and JS/TS named specifier merging; tests

## 3. Language queries (one task per language, each with ≥5 fixture cases incl. a negative case)

- [ ] 3.1 JSON (+ jsonc)
- [ ] 3.2 YAML (multi-doc, skip anchors)
- [ ] 3.3 Java
- [ ] 3.4 Kotlin
- [ ] 3.5 Python
- [ ] 3.6 Go
- [ ] 3.7 JavaScript
- [ ] 3.8 TypeScript (incl. TSX, interfaces, type literals)

## 4. Proposals

- [ ] 4.1 `propose.rs`: chunk → container lookup across versions by key path, merge, covered ranges, multi-chunk coverage
- [ ] 4.2 Validation by reparse and error-count comparison; tests for discarded proposals
- [ ] 4.3 Fixture runner: `tests/fixtures/<lang>/<case>/{base,ours,theirs,expected|unresolvable}` covering every spec scenario

## 5. Integration

- [ ] 5.1 IPC `structural_resolve` on blocking pool with 2 s timeout; TS bindings
- [ ] 5.2 Session action `applyReplacement(chunkIds, text, kind)` (undoable) in merge-editor model with Vitest
- [ ] 5.3 Editor extension: chunk indicator, preview popover (Apply/Dismiss), toolbar "Resolve structurally (N)", background computation
- [ ] 5.4 Playwright tests for preview/apply and bulk structural with undo
- [ ] 5.5 UI fidelity per `ui/Structural.dc.html` (toolbar item + status text, "S" gutter indicator, preview popover layout and copy); Playwright screenshot baseline of the open preview
