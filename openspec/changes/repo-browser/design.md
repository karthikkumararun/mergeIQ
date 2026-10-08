## Context

`git-adapter` provides all state and mutations; `merge-editor-ui` provides the editor. This change is mostly UI composition and window management.

## Goals / Non-Goals

**Goals:**
- Fast multi-file conflict workflow with clear operation context.
- One window per repository; robust to external git activity.

**Non-Goals:**
- General git client features (commit graph, staging hunks, branches). Future changes.
- Starting merges/rebases.

## Decisions

**Window model.** Backend `RepoRegistry` maps canonical worktree root → window label; `repo_open(path)` focuses existing or creates `WebviewWindow` with route `/repo/<repoId>`. Each repo has one watcher (from `git-adapter`) emitting `repo-changed` to its window only.

**UI layout (`apps/desktop/src/repo/`)**
```
HomeView.tsx           open/drag-drop/recents
RepoWindow.tsx         banner + split: left ConflictsPanel (resizable), right Tabs
OperationBanner.tsx
ConflictsPanel.tsx     virtualized list (react-window) — repos can have 1000s of conflicts
ResolvedSection.tsx
EditorTabs.tsx         hosts <MergeEditor> per tab; keeps editor state alive while hidden
store/repoStore.ts     Zustand: status, conflicts, tabs, resolved-session log
```

**Non-text conflicts.** Until `special-conflicts` lands, Merge… on non-`BothModified` or binary files opens a minimal panel with Accept Left / Accept Right / Delete, built so `special-conflicts` can replace it.

**Side description text.** Derived from ConflictType with contextual labels: `DeletedByUs` → Left "Deleted", Right "Modified".

**Refresh strategy.** On `repo-changed`, refetch `repo_status`; diff conflict list by path token to preserve selection and scroll.

**Testing.** Vitest for store reducers; Playwright with IPC mock for UI flows; one end-to-end test via `tauri-driver` on Linux CI against a scripted repo (merge with 3 conflicts → resolve → continue).

## UI reference

Approved screens are in `ui/` and `bootstrap-app/ui/Main.dc.html`; how to read them and precedence rules: `openspec/UI.md`.

- `bootstrap-app/ui/Main.dc.html` (home body): "Open repository…" primary button (⌘O / Ctrl+O), drop zone with `mergeiq open` hint, inline "Not a git repository." error (`--danger-*`, dismissible), Recent repositories list (name, mono path, relative time, remove ×; missing paths dimmed with "folder not found" and a Remove button).
  - The mock also shows "Resolve a single file…"; no spec covers it. Leave it out.
- `ui/RepoWindow.dc.html` — repository window. Variant `allResolved` in `data-props`.
  - Tab bar: "Conflicts (N)" tab first, then one tab per open file (mono name, `--accent` dot when unsaved).
  - Operation banner card: the contextual sentence (bold mono branch names), a sub-line ("N conflicted files · Continue unlocks when none remain" or "All conflicts resolved" in `--ok-fg`), buttons Skip commit… (rebase only) · Abort… (danger outline) · Continue (primary, disabled until resolved). A "Git output" disclosure holds verbatim git messages.
  - Conflicts toolbar: filter box, Flat / By folder segmented control, selection count, "Accept Left `<label>`" / "Accept Right `<label>`".
  - Table columns: checkbox · Path (dir muted, file bold, mono) · Conflict (type) · Left · `<label>` · Right · `<label>` · Kind badge · Merge… button. "Deleted" side text is `--danger-fg`. Wide tables scroll horizontally.
  - "Resolved in this session · N" list: check icon, path, method (Merged / Accepted Left / Accepted Right), "Reopen conflict…".
  - External-change notice: a non-blocking toast "`<file>` was resolved outside MergeIQ" with Close tab and Reload (the mock shows only Close tab; Reload is required by the spec).
- The Kind column shows `special-conflicts` classes. Until that change lands, show the conflict type only and hide the column.
- **Layout:** the mock puts the conflicts list in the first tab, full width, while the layout decision above describes a split view (list left, tabs right). See Open Questions.
 → cap 10 tabs, LRU close of clean tabs.
- [Rebase continue triggers hooks that take long] → run async with spinner and cancel-not-possible note; stream stderr.

## Open Questions

- **Decide before implementing:** conflicts list as the first tab (approved mock, full-width table) or as a resizable left panel beside the editor tabs (Decisions › UI layout)? Default if undecided: follow the mock, because the table's columns need the width.
- Should Continue offer editing the commit message? v1: default message; later add message editor.
