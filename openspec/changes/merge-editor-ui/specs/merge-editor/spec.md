## ADDED Requirements

### Requirement: Three-pane layout
The merge editor SHALL show three panes left to right: **Left** (ours, read-only), **Result** (editable), **Right** (theirs, read-only), separated by connector gutters. Pane headers SHALL show the contextual side label from `git-adapter` (role, ref, short SHA, subject); the Result header SHALL show the file path. Panes SHALL be resizable by dragging and default to equal widths.

#### Scenario: Open a conflicted file
- **WHEN** the editor opens a `MergeDocument` for a merge of `feature` into `main`
- **THEN** left header shows `main`, right header shows `feature`, result header shows the file path

### Requirement: Initial result content
The Result pane SHALL initially contain the base text. If setting `autoApplyNonConflicting` is true, all `OursOnly`, `TheirsOnly` and `BothSame` chunks SHALL be applied on open as a single undoable step.

#### Scenario: Default open
- **WHEN** a document with 2 non-conflicting chunks and 1 conflict opens with default settings
- **THEN** the result equals base and all 3 chunks are unresolved

#### Scenario: Auto-apply enabled
- **WHEN** `autoApplyNonConflicting` is true
- **THEN** non-conflicting chunks are applied, only the conflict remains, and one undo reverts to base

### Requirement: Chunk highlighting
Each chunk SHALL be highlighted in all panes where it has lines, with colors by kind and change type: inserted (green), modified (blue), deleted (gray, shown as a thin marker line where there are no lines), conflict (red). Token-level fine diff ranges SHALL be emphasized within highlighted lines. Resolved chunks SHALL be rendered muted with a check icon. Every state SHALL also have a non-color indicator (gutter icon or pattern).

#### Scenario: Conflict colors
- **WHEN** a `Conflict` chunk is unresolved
- **THEN** its lines in left, result and right panes carry the conflict style and a conflict gutter icon

#### Scenario: Word highlight
- **WHEN** a chunk has fine diff data
- **THEN** the changed tokens carry an inline emphasis decoration in the side pane and base-equivalent tokens in result

### Requirement: Connector gutters
Between Left↔Result and Result↔Right the editor SHALL draw bands connecting each chunk's line range in the side pane to its current range in the Result pane, updating on scroll, resize and edits. Bands SHALL be colored like their chunk and drawn only for visible chunks.

#### Scenario: Band follows edits
- **WHEN** the user inserts 3 lines above a chunk in the Result pane
- **THEN** that chunk's bands end 3 lines lower in the Result pane

### Requirement: Per-chunk actions
For each unresolved side of a chunk the side pane gutter SHALL show **Apply** (`>>` on left, `<<` on right) and **Ignore** (`×`). Semantics:
- Apply on a non-conflicting chunk replaces the chunk's result range with that side's text and marks it resolved.
- Ignore marks that side ignored without changing text; the chunk is resolved when every side that has changes is applied or ignored.
- For a `Conflict` chunk, after one side is applied the other side's Apply SHALL become **Append**, inserting that side's text after the already-applied text.
- For `BothSame`, applying either side resolves the chunk.
Every action SHALL be a single undoable step that also restores chunk status on undo.

#### Scenario: Apply then append
- **WHEN** the user applies left then appends right on a conflict
- **THEN** the result range contains left text followed by right text and the chunk is resolved

#### Scenario: Ignore both sides
- **WHEN** the user ignores both sides of a conflict
- **THEN** the result range keeps base text and the chunk is resolved

#### Scenario: Undo restores status
- **WHEN** the user applies left on a chunk and then presses undo
- **THEN** the result text and the chunk's unresolved status are both restored

### Requirement: Manual edits
Typing inside an unresolved chunk's result range SHALL mark the chunk `Edited` (resolved). Each resolved or edited chunk SHALL offer **Revert** (in the result gutter) restoring base text and unresolved status. Edits outside chunks SHALL NOT change chunk states.

#### Scenario: Edit resolves chunk
- **WHEN** the user types inside a conflict's result range
- **THEN** the chunk becomes `Edited` and the remaining-conflicts counter decrements

### Requirement: Bulk actions
The toolbar SHALL provide:
- **Apply non-conflicting changes**: All / Left only / Right only — applies every unresolved `OursOnly`/`TheirsOnly`/`BothSame` chunk on the chosen side(s).
- **Resolve simple conflicts** (magic wand): for each unresolved `Conflict` whose engine `simple` result is `Resolved`, replace its result range with that text and mark it `AutoResolved`. Disabled when none qualify.
- **Accept Left** / **Accept Right** (whole file): replace the whole result with that side and mark all chunks resolved, after confirmation if any manual edits exist.
Each bulk action SHALL be one undoable step.

#### Scenario: Magic wand
- **WHEN** a file has 3 conflicts of which 2 are simple-resolvable and the user clicks Resolve simple conflicts
- **THEN** those 2 are resolved with merged text, 1 remains, and one undo reverts both

### Requirement: Status counter
The toolbar SHALL show the number of unresolved changes and unresolved conflicts, e.g. "3 changes · 1 conflict left", and "All changes processed" when zero.

#### Scenario: Counter updates
- **WHEN** the last conflict is resolved
- **THEN** counter shows "All changes processed"

### Requirement: Navigation
The editor SHALL support next/previous change and next/previous unresolved conflict, scrolling all panes to the chunk and placing the Result cursor at its start. Navigation SHALL wrap with a visual hint.

#### Scenario: Next conflict
- **WHEN** cursor is above the first unresolved conflict and user presses next-conflict
- **THEN** all panes scroll to that conflict and it is outlined as current

### Requirement: Synchronized scrolling
Scrolling any pane SHALL scroll the others so corresponding lines stay aligned: within unchanged regions by line offset; within a chunk by proportional interpolation across the chunk's ranges. Sync SHALL be toggleable.

#### Scenario: Scroll alignment
- **WHEN** the user scrolls the left pane to an unchanged line N
- **THEN** result and right panes show their corresponding unchanged line at the same vertical position (±1 line)

### Requirement: Base pane and unchanged folding
A **Show base** toggle SHALL add a read-only base pane between Left and Result. A **Collapse unchanged** toggle SHALL fold unchanged regions longer than 8 lines in all panes, keeping 3 context lines, with folds aligned across panes.

#### Scenario: Toggle base
- **WHEN** user enables Show base
- **THEN** a fourth pane with base text appears with chunk highlights and the setting persists

### Requirement: Whitespace policy switch
The toolbar SHALL let the user choose the whitespace policy. Changing it SHALL re-run analysis only if no chunk has been resolved yet; otherwise it SHALL ask to confirm resetting progress.

#### Scenario: Change policy before progress
- **WHEN** no chunk is resolved and the user selects Ignore trailing whitespace
- **THEN** chunks are recomputed without a prompt

### Requirement: Syntax highlighting
The panes SHALL apply syntax highlighting by file extension for Java (`.java`), Python (`.py`, `.pyi`), Kotlin (`.kt`, `.kts`), YAML (`.yml`, `.yaml`), JSON (`.json`, `.jsonc`), JavaScript (`.js`, `.mjs`, `.cjs`, `.jsx`), TypeScript (`.ts`, `.mts`, `.cts`, `.tsx`), Go (`.go`), falling back to plain text.

#### Scenario: Kotlin file
- **WHEN** editing `Main.kt`
- **THEN** Kotlin keywords are highlighted in all panes

### Requirement: Commit context
Clicking a side header SHALL open a popover listing that side's commits touching the file (from `git-adapter` context) with sha, subject, author and relative date, and a copy-SHA action.

#### Scenario: Popover
- **WHEN** user clicks the right header
- **THEN** popover lists theirs-side commits newest first

### Requirement: Save and cancel
**Apply** SHALL save. If unresolved chunks remain, a dialog SHALL offer: *Continue resolving* (default), *Save with conflict markers* (save unstaged), *Mark as resolved anyway* (save current result and stage). With none remaining it SHALL save and stage directly. **Cancel** SHALL close without saving, confirming first if the result differs from its initial content. Cmd/Ctrl+S SHALL trigger Apply.

#### Scenario: Save with unresolved
- **WHEN** 1 conflict remains and user clicks Apply then Save with conflict markers
- **THEN** file is saved with markers around that region and not staged

#### Scenario: Clean save
- **WHEN** all chunks are resolved and user clicks Apply
- **THEN** result is saved and staged and the editor reports success to its host

### Requirement: Keyboard shortcuts
The editor SHALL provide default shortcuts (Mod = Cmd on macOS, Ctrl elsewhere): next/prev change `Alt+↓`/`Alt+↑`; next/prev conflict `F7`/`Shift+F7`; apply left to current chunk `Mod+Alt+←`; apply right `Mod+Alt+→`; ignore current chunk sides `Mod+Alt+Backspace`; resolve simple conflicts `Mod+Alt+M`; apply all non-conflicting `Mod+Alt+A`; save `Mod+S`; undo/redo standard; find `Mod+F`. All toolbar and gutter actions SHALL be reachable by keyboard and have accessible names.

#### Scenario: Keyboard-only resolution
- **WHEN** a user presses F7 then Mod+Alt+← then Mod+S
- **THEN** the first conflict is resolved with left text and the save flow starts

### Requirement: Large file responsiveness
Opening a 20,000-line file with 200 chunks SHALL render the first screen within 1 s after analysis returns, and typing latency in the Result pane SHALL stay under 50 ms.

#### Scenario: Large file smoke test
- **WHEN** the Playwright perf test opens the 20k-line fixture
- **THEN** first paint and typing latency are within budget
