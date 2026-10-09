## 1. Test harness and fixtures

- [x] 1.1 Add Rust test/bin that exports engine Analysis JSON for fixtures into `apps/desktop/src/merge-editor/__fixtures__/`
- [x] 1.2 IPC mock layer `src/ipc/mock.ts` switchable via `VITE_IPC_MOCK=1`; dev route `/dev/merge?fixture=<name>`
- [x] 1.3 Playwright setup (chromium + webkit) against Vite dev server

## 2. Session model (no UI)

- [x] 2.1 `model/types.ts` and `model/session.ts`: ChunkSet StateField, effects, position mapping
- [x] 2.2 `model/actions.ts`: apply, append, ignore, revert, apply-non-conflicting (all/left/right), resolve-simple, accept-whole-side
- [x] 2.3 invertedEffects for undo/redo of chunk state; edited-detection transaction filter
- [x] 2.4 Vitest suite covering every scenario in "Per-chunk actions", "Manual edits", "Bulk actions", "Initial result content"

## 3. Panes and highlighting

- [x] 3.1 ResultPane + SidePane CodeMirror setups; read-only sides; resizable split layout
- [x] 3.2 Chunk line decorations by kind/state, empty-range marker lines, fine-diff mark decorations, gutter icons
- [x] 3.3 Merge colours from the chunk/syntax tokens in `src/theme/tokens.css` (`--ins-*`, `--mod-*`, `--con-*`, `--res-*`, `--hatch`, `--kw` …); contrast check ≥ 3:1 for highlights vs background
- [x] 3.4 `lang/index.ts` lazy language loading for the 8 languages; test mapping by extension

## 4. Gutters and actions

- [x] 4.1 Connector SVG overlay with Bézier bands for visible chunks; updates on scroll/resize/edit
- [x] 4.2 Gutter action buttons (apply/append/ignore/revert) with aria-labels; wired to actions
- [x] 4.3 Playwright tests: apply-then-append, ignore both, undo restores, band follows edits

## 5. Toolbar, navigation, sync

- [x] 5.1 Toolbar: apply non-conflicting menu, magic wand, accept left/right (with confirm), counter
- [x] 5.2 Navigation prev/next change and conflict with wrap hint and current-chunk outline
- [x] 5.3 Scroll sync piecewise map + toggle; Playwright alignment test
- [x] 5.4 Show base pane toggle and collapse-unchanged aligned folding; persist settings
- [x] 5.5 Whitespace policy selector with re-analysis and reset confirmation

## 6. Headers, save flow, keyboard

- [x] 6.1 SideHeader with labels and commit context popover (copy SHA)
- [x] 6.2 Save dialog (continue / markers / force), cancel confirmation; onSave contract with ResultLine serialization
- [x] 6.3 Keymap for all shortcuts; Playwright keyboard-only resolution test
- [x] 6.4 Extension hook (`extensions` prop) with no-op default

## 7. Quality

- [x] 7.1 Perf smoke test with 20k-line fixture (first paint < 1 s, typing < 50 ms)
- [x] 7.2 Visual baseline screenshots light/dark
- [x] 7.3 a11y check with axe-core in Playwright (no serious violations)
- [x] 7.4 UI fidelity pass against `ui/MergeEditor.dc.html`, `ui/EditorBase.dc.html`, `ui/EditorLight.dc.html`, `ui/SaveDialog.dc.html` (see design.md › UI reference; design.md decisions win on bands/padding): toolbar order and labels, header chips, chunk marks, counter pill, save dialog default; Playwright screenshots of default, show-base, light + popover and save dialog added to visual baselines
