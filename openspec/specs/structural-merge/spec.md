# structural-merge Specification

## Purpose
TBD - created by archiving change structural-merge. Update Purpose after archive.

## Requirements

### Requirement: Language detection
The structural merger SHALL select a language by file extension: Java `.java`; Python `.py`, `.pyi`; Kotlin `.kt`, `.kts`; YAML `.yml`, `.yaml`; JSON `.json` (and `.jsonc` with comments allowed); JavaScript `.js`, `.mjs`, `.cjs`, `.jsx`; TypeScript `.ts`, `.mts`, `.cts`, `.tsx`; Go `.go`. Other files SHALL return `Unsupported` without error.

#### Scenario: Unsupported extension
- **WHEN** structural resolve is requested for `README.md`
- **THEN** result is `Unsupported` and no suggestions are shown

### Requirement: Keyed entry merge
For each `Conflict` chunk, the merger SHALL find the innermost container (per language entry definitions) enclosing the chunk in all three versions, split its children into entries with keys, and 3-way merge entries by key:
- entry only added by one side → include it
- entries added by both sides with different keys → include both: ours' additions first, then theirs', at the position where they were added
- entry added by both with the same key and identical text → include once
- entry changed by one side only → take the changed version
- entry deleted by one side and unchanged by the other → delete it
- entry changed by both differently → recurse if the entry is itself a container, otherwise the chunk is unresolvable
- entry deleted by one side and changed by the other → unresolvable
Entry text SHALL be copied from source spans (formatting and comments preserved), never re-printed from the AST.

#### Scenario: Both add different methods at end of a Java class
- **WHEN** ours adds method `a()` and theirs adds method `b()` after the last method of class `Foo`
- **THEN** proposal contains both methods, `a()` then `b()`, with base indentation

#### Scenario: Both add the same Python function differently
- **WHEN** ours and theirs both add top-level `def helper()` with different bodies
- **THEN** the chunk is unresolvable

#### Scenario: JSON different keys
- **WHEN** ours adds `"lint": "eslint ."` and theirs adds `"test": "vitest"` inside the same `"scripts"` object at the same position
- **THEN** proposal contains both keys with valid comma placement

#### Scenario: YAML nested keys
- **WHEN** ours changes `spec.replicas` and theirs changes `spec.image` on adjacent lines
- **THEN** proposal contains both changes

#### Scenario: Go functions
- **WHEN** ours adds `func A()` and theirs adds `func B()` at the end of the file
- **THEN** proposal contains both functions separated by a blank line as in base style

### Requirement: Entry definitions per language
Entry and key definitions SHALL be data-driven tree-sitter queries per language stored in `crates/mergeiq-struct/queries/<lang>/entries.scm` with at least:
- Java: imports (key: qualified name), class/interface/enum/record bodies (members keyed by kind + name, methods by name + parameter types), top-level types
- Kotlin: imports, class/object bodies (functions by name + params, properties by name), top-level declarations
- Python: import statements, module and class bodies (def/class by name, assignments by target)
- Go: import specs (path), top-level func (receiver + name), type, const and var specs (name)
- JavaScript/TypeScript: import declarations (source), top-level declarations (name), class bodies (members by name), TS interface/type literal members (name), object literal properties (key)
- JSON: object members (key), recursive
- YAML: block mapping pairs (key), recursive

#### Scenario: Query-driven key
- **WHEN** a new key rule is added to a language's `entries.scm`
- **THEN** it takes effect without changes to merge code

### Requirement: Import merging
Import entries SHALL be merged as a set union with deduplication. If the import block is sorted in base, ours and theirs, additions SHALL be inserted in sorted order; otherwise ours' additions then theirs'. Python `from m import a, b` and JS/TS `import { a, b } from "m"` statements for the same module SHALL merge their name lists when both sides changed the same statement. Removal by one side SHALL be honored unless the other side also edited that same statement.

#### Scenario: Sorted Java imports
- **WHEN** base imports are sorted, ours adds `java.util.List` and theirs adds `java.util.Map`
- **THEN** proposal inserts both in sorted order

#### Scenario: Python from-import names
- **WHEN** base `from os import path`, ours `from os import path, sep`, theirs `from os import getcwd, path`
- **THEN** proposal is `from os import getcwd, path, sep` (sorted because all inputs sorted)

### Requirement: Separators and formatting
When entries require separators (commas in JSON objects, JS/TS object literals, Go grouped imports none, etc.), the merger SHALL produce a syntactically valid sequence, preserving trailing-comma style from base. Indentation of inserted entries SHALL match their source side relative to container indentation.

#### Scenario: JSON trailing entry
- **WHEN** both sides append a different key after the last key of an object without trailing comma
- **THEN** proposal adds a comma after the previous last entry and none after the new last entry

### Requirement: Proposal scope across chunks
A proposal SHALL report the base/ours/theirs line ranges it covers, which MAY span several adjacent chunks and the unchanged lines between them. It SHALL only be offered if every chunk it covers is unresolved; applying it SHALL mark all covered chunks resolved with kind `structural` as one undoable step.

#### Scenario: Proposal spans two chunks
- **WHEN** an entry merge covers a conflict chunk and an adjacent ours-only chunk
- **THEN** the proposal lists both chunk ids and applying resolves both

### Requirement: Validation
A proposal SHALL be discarded if parsing the full resulting file (base with the proposal applied in place of covered ranges, other chunks taken from base) yields more tree-sitter ERROR/MISSING nodes than the maximum among base, ours and theirs.

#### Scenario: Invalid output discarded
- **WHEN** a merged entry sequence would produce a missing brace
- **THEN** no proposal is returned for that chunk

### Requirement: Editor integration
The merge editor SHALL show a structural indicator on chunks with a proposal; activating it SHALL show a preview (diff vs current result range) with Apply / Dismiss. The toolbar SHALL show "Resolve structurally (N)" applying all proposals as one undoable step. Proposals SHALL be computed in the background after the editor opens without blocking interaction, with a 2 s timeout per file.

#### Scenario: Toolbar bulk structural
- **WHEN** a TS file has 4 conflicts of which 3 have proposals and the user clicks Resolve structurally (3)
- **THEN** 3 chunks become resolved with kind `structural`, 1 remains, and one undo reverts all 3

#### Scenario: Timeout
- **WHEN** structural analysis exceeds 2 s
- **THEN** no proposals are shown and a log entry records the timeout
