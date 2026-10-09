## Context

The Rust engine returns a static `Analysis`. The interactive session (apply/ignore/append, manual edits, undo) must run at keystroke speed in the webview, so it lives in TypeScript on top of CodeMirror 6 state. The component must be host-agnostic: mergetool mode and repo browser both embed it.

## Goals / Non-Goals

**Goals:**
- IntelliJ-parity interactions with stronger labeling and keyboard flow.
- Single source of truth for chunk status inside CodeMirror state, so undo/redo is always consistent.

**Non-Goals:**
- Structural merge suggestions (`structural-merge` adds a provider hook).
- AI actions (`ai-assist` adds toolbar/gutter items via the same hook).
- Non-text conflicts (`special-conflicts`).

## Decisions

**Module layout (`apps/desktop/src/merge-editor/`)**
```
MergeEditor.tsx          host component: props { doc: MergeDocument, onSave, onCancel, extensions? }
model/types.ts           ChunkState, SideStatus, ResolutionKind
model/session.ts         StateField<ChunkSet> + StateEffects (apply, append, ignore, revert, bulk) + invertedEffects for undo
model/actions.ts         pure functions computing transactions for each action (unit-tested without DOM)
model/mapping.ts         result-range tracking via ChangeSet.mapPos; edit-inside-chunk detection
panes/SidePane.tsx       read-only CM view with chunk decorations + gutter buttons
panes/ResultPane.tsx     editable CM view, owns history()
gutters/Connector.tsx    SVG overlay; computes band polygons from lineBlockAt() of both views
sync/scrollSync.ts       chunk-aligned scroll mapping
toolbar/Toolbar.tsx      bulk actions, nav, counter, toggles
header/SideHeader.tsx    labels + commit popover
lang/index.ts            extension → LanguageSupport loader (lazy import)
keymap.ts                shortcuts
dialogs/SaveDialog.tsx
```

**Chunk state lives in the Result view's EditorState.** `ChunkState { id, kind, leftStatus, rightStatus: 'pending'|'applied'|'ignored'|'na', resolution: 'none'|'applied'|'edited'|'auto'|'whole-file', resultFrom, resultTo }`. Positions mapped through every transaction's changes. Actions dispatch text changes + a `StateEffect` in one transaction; `invertedEffects` records previous ChunkState so `undo` restores both. Side panes are derived views that subscribe to Result state via a small Zustand store updated in an `updateListener`. Alternative (separate store + manual undo stack) rejected: two undo systems drift.

**Edited detection.** In a transaction filter: if a user change (`userEvent` input/delete/paste) intersects an unresolved chunk's result range (inclusive of empty insertion point), add an `markEdited` effect.

**Empty ranges.** A chunk whose result range is empty (pure insertion side or deleted in result) is rendered as a 2px marker line between lines; bands converge to that line.

**Connector rendering.** One absolutely-positioned SVG per gutter (width 48px). For each visible chunk: polygon from side `(top,bottom)` to result `(top,bottom)` using cubic Bézier edges (IntelliJ-style curves). Recomputed on `requestAnimationFrame` after scroll/geometry change. Buttons are HTML elements positioned over the side-pane gutter, not in SVG, for accessibility.

**Scroll sync.** Build a monotonic piecewise-linear map between line numbers of each pair (side↔result) from chunk ranges (equal segments 1:1, chunk segments proportional). On scroll of pane A, compute top visible line → map → `scrollTop` of B via `lineBlockAt`. Guard against feedback loops with an `isSyncing` flag per frame.

**Folding unchanged.** Use computed equal segments; apply CM `foldEffect` in each pane on the corresponding ranges so folds align.

**Whitespace re-analysis.** Calls IPC `conflict_analyze(pathToken, policy)` (or for mergetool, analysis on in-memory bytes) and rebuilds editor state.

**Host contract.**
```ts
type MergeDocument = { pathToken; displayPath; analysis: Analysis; labels: { left: SideLabel; right: SideLabel; base?: SideLabel }; context?: CommitContext; languageHint?: string }
onSave(result: { lines: ResultLine[]; unresolvedIds: number[]; mode: 'resolved'|'markers'|'force' }): Promise<void>
```
`extensions` prop: array of `MergeEditorExtension { toolbarItems?, chunkActions? }` — hook for structural/AI later.

**Testing.** Vitest for `model/*` (pure state transitions, undo). Playwright against Vite dev build with an IPC mock layer (`src/ipc/mock.ts` using fixture JSON produced by the Rust engine) for interaction scenarios and perf smoke. Visual baseline screenshots for light/dark.

## UI reference

Approved screens are in `ui/`; how to read them and precedence rules: `openspec/UI.md`.

- `ui/MergeEditor.dc.html` — the editor. Variants in its `data-props`: `theme` (dark/light), `showBase`, `showPopover`. It already uses the token variables from `apps/desktop/src/theme/tokens.css`.
  - Title bar: file name, "Merging `<right label>` into `<left label>`", status counter pill (`--pill-*`), Cancel, Apply (primary, shows ⌘S / Ctrl+S).
  - Toolbar order: Apply non-conflicting (menu: All / Left only / Right only) · Resolve simple (disabled when none qualify) | Accept Left · Accept Right | prev/next change · next conflict (F7) | Show base · Collapse unchanged · Sync scroll (pressed toggles) | Whitespace select. Prev conflict (Shift+F7) is keyboard-only and in the overflow menu.
  - Pane headers: contextual label in mono + role chip ("Left · ours · read-only", "Result · editable", "Right · theirs · read-only") + short SHA and subject. Clicking a side header opens the commit popover (`EditorLight.dc.html` shows it).
  - Chunks: line background `--{ins,mod,con,res}-bg`, word emphasis `--*-em`, first-line gutter mark `+ ~ ! ✓` in `--*-fg`. Current conflict gets a 1px `--con-fg` outline in the Result pane. Resolved chunks are muted with a check on the side gutter and a ↺ Revert button in the Result gutter.
  - Collapsed unchanged regions show an aligned "⋯ N unchanged lines" row in every pane.
  - Footer: language · encoding · line ending · cursor, plus shortcut hints.
- `ui/EditorBase.dc.html` — Show base: a read-only Base pane between Left and Result with chunk highlights (no gutter marks).
- `ui/EditorLight.dc.html` — light theme with the commit popover open (SHA, subject, author, relative time, copy-SHA button per row, newest first).
- `ui/SaveDialog.dc.html` — Apply with unresolved chunks: title counts conflicts and changes, three full-width options, *Continue resolving* is default (focused, accent-tinted, Enter).

**Where this design.md wins over the mock-up:**
- The mock aligns panes with hatched padding rows and draws flat rectangular bands. Implement the decisions above: no padding rows, empty ranges as 2px marker lines, Bézier connector bands in a 48px SVG gutter.
- The mock places `>>` / `<<` / `×` buttons in the connector column. Per the decision above, they are HTML buttons in the side panes' gutters next to the connector; keep their look (24×20, mono glyphs, `--border-3` outline) and accessible names.

## Risks / Trade-offs

- [Gutter band rendering perf with many chunks] → only visible chunks; rAF throttle.
- [Scroll sync jitter in WKWebView] → sync on `scroll` event with rAF, not smooth-scroll.
- [Kotlin/Go highlighting quality via legacy modes] → acceptable for v1; Lezer grammars later.
- [Edits straddling chunk boundaries] → mark every intersected chunk edited; revert restores each independently.

## Open Questions

- Default for `autoApplyNonConflicting`: false (IntelliJ parity). Revisit after user feedback.
- **Line endings are normalized on save.** The Result document holds `\n`-normalized text (CodeMirror cannot keep mixed terminators), so a file with mixed endings is saved with the dominant ending (`Analysis.dominant_eol`) on every line, and the last line has none only when no side had a trailing newline. Files with uniform endings round-trip byte-for-byte (tested for CRLF). Preserving per-line terminators of untouched lines would need a terminator side-table mapped through edits; deferred until someone hits a mixed-ending file.
- **Implementation notes / deviations found while building** (resolved here rather than silently):
  - `conflict_analyze(path, whitespace)` did not exist in `git-adapter`; this change adds it (`apps/desktop/src-tauri/src/commands/git.rs`, wrapping `Repo::load_conflict` with `Options`). `MergeEditor` stays host-agnostic: the whitespace selector is enabled only when the host passes `reanalyze(policy)`; `reanalyzeViaIpc(pathToken)` in `merge-editor/hosts.ts` is the IPC implementation.
  - Settings persistence uses two new commands, `get_merge_editor_settings` / `update_merge_editor_settings`, with the preferences nested under `mergeEditor` in `settings.json` (unknown keys are still preserved). `SettingsDto` is unchanged so the home/settings views are unaffected.
  - `onSave` receives `{ lines, unresolvedIds, unresolved, mode, encoding }`. `unresolved` carries the conflicts to splice in as markers for `mode: "markers"` and `renderSaveText()` (mirrors `mergeiq_core::serialize`) turns the payload into the text `conflict_save` takes; hosts decide staging from `mode` (`resolved` and `force` stage, `markers` does not).
  - The counter counts every unresolved chunk as a change and the conflicts among them: "3 changes · 1 conflict left" means three chunks remain, one of which is a conflict.
  - With Show base on, the Base pane sits between Left and the left gutter, so the left bands visually start at Base's edge; they are still computed from the Left pane's lines.
  - Revert lives in the gutter on the side that changed the chunk (left gutter, right gutter for theirs-only chunks) next to the Result pane edge.
  - The save dialog says "still has a conflict at line N" instead of naming the enclosing function (no symbol lookup in v0.1).
  - Contrast check (task 3.3): highlight backgrounds are deliberately subtle, so "≥ 3:1" is applied to what must stay legible: gutter marks against the pane and chunk backgrounds (≥ 3:1) and code text on every highlight (≥ 4.5:1). Run for both themes in `panes/contrast.test.ts`.
  - Visual baselines are generated per platform (darwin/chromium committed). Playwright runs are not part of the CI gate (`pnpm test` is Vitest); run `pnpm --filter desktop exec playwright test` locally.
