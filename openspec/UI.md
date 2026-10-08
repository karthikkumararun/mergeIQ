# MergeIQ UI designs

Approved screen designs live in each change under `openspec/changes/<change>/ui/`. The live canvas they were exported from (2026-10-07, after peer review) is https://claude.ai/artifact/Xrog6wb93Cmt8uN6KQNWb3. The repo copies are the versioned source for implementation; if the canvas changes later, re-export before building.

## Precedence

1. `specs/**/spec.md` — behaviour. Always wins.
2. `design.md` — architecture and mechanics (e.g. editor connector bands, CodeMirror decorations). Wins over the mock-ups where they differ; each `design.md` "UI reference" section lists known differences.
3. `ui/*.dc.html` — layout, hierarchy, copy, states, spacing and colour. Match these unless 1 or 2 say otherwise.

## Reading a `.dc.html` file

Each file is one screen as plain HTML + inline CSS, written for a design canvas runtime (`support.js`, not in this repo), so it will not render by opening it in a browser. Read the source:

- `<x-dc>…</x-dc>` holds the markup. Inline `style="…"` carries the real layout values (flex/grid, gaps, padding, font sizes, radii).
- `<helmet><style>` holds shared classes (`.btn`, `.btn.pri`, `.ib` icon button, `.kbd`, …) — treat them as component specs.
- `{{name}}` is a value from `renderVals()` in the `<script type="text/x-dc">` block at the bottom; `<sc-for list=… as=…>` repeats, `<sc-if value=…>` is conditional. The script contains sample data (file lists, code lines) — it is fixture data, not app logic.
- `data-props` on that script lists the screen's variants (e.g. `theme`, `showBase`, `allResolved`, `outcome`). Implement every listed variant as a real state.
- `<a href="X.dc.html">` links show navigation between screens. `<dc-import name="X">` embeds another screen with different props.
- Hex colours in most boards are the dark-theme values. Map each to the variable with the same value in `changes/bootstrap-app/ui/tokens.css` — never hard-code them. `MergeEditor.dc.html` already uses the variables.
- Sample names (repos, branches, SHAs, authors, sizes, `[COST]`) are placeholders.

## Shared visual rules

- Fonts: IBM Plex Sans (UI), JetBrains Mono (code, paths, SHAs, commands). Bundle both; no network font loading.
- Code lines 13px / 22px. UI body 13–15px. Radii 4 / 6 / 10px.
- One accent (`--accent`, amber) for primary actions and focus rings. At most one primary button per view.
- Chunk states always pair colour with a non-colour mark: `+` inserted, `~` modified, `!` conflict, `✓` resolved, hatched background for missing lines.
- Every control is a real `<button>` / `<a>` / `<input>` with an accessible name; icon-only buttons have `aria-label`. Focus ring: 2px `--accent`.
- Paths, branches and SHAs are monospace; branch names are always the contextual labels from `git-adapter`, never "ours/theirs".

## Screen index

| Change | Screen | Covers |
|---|---|---|
| bootstrap-app | `Main.dc.html` | Home window, theme switch (also repo-browser open/recents) |
| bootstrap-app | `Tokens.dc.html`, `tokens.css` | Theme tokens, dark + light |
| merge-editor-ui | `MergeEditor.dc.html` | Three-pane editor; variants `theme`, `showBase`, `showPopover` |
| merge-editor-ui | `EditorBase.dc.html` | Show base pane |
| merge-editor-ui | `EditorLight.dc.html` | Light theme + commit popover |
| merge-editor-ui | `SaveDialog.dc.html` | Apply with unresolved changes |
| mergetool-cli | `CliSetup.dc.html` | Settings › Command line |
| repo-browser | `RepoWindow.dc.html` | Banner, conflicts list, resolved section, tabs; variant `allResolved` |
| structural-merge | `Structural.dc.html` | Proposal indicator, preview, bulk action |
| special-conflicts | `ImageConflict`, `ModifyDelete`, `Lockfile`, `Submodule`, `Rename`, `GoSum`, `PickSide` | One panel per conflict class |
| ai-assist | `AiAssist.dc.html` | Suggestion panel; variant `syntaxWarning` |
| ai-assist | `AiSettings.dc.html` | Settings › AI |
