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
RepoWindow.tsx         banner + split: left ConflictsPanel (resizable), right Tabs (see ui/RepoSplit.dc.html)
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
- `ui/RepoSplit.dc.html` — **the repository window layout (chosen 2026-10-07)**: matches the split decision above. It embeds `merge-editor-ui/ui/MergeEditor.dc.html` in the tab area.
  - Header (repo name, mono path), then the operation banner as one slim strip: contextual sentence (bold mono branch names) + "· N conflicted files", "Git output" disclosure, buttons Skip commit… (rebase only) · Abort… (danger outline) · Continue (primary, disabled until resolved).
  - Left `ConflictsPanel` (default ~340px, resizable 280–520px, width persisted): filter box + Flat / Folders control; selection row ("N selected", Accept Left / Accept Right, with the contextual label in the tooltip and in the confirmation); a two-line row per file — line 1 file name (bold mono) + muted directory (ellipsis), line 2 conflict type · Left / Right status ("Deleted" in `--danger-fg`) · kind badge. Checkbox per row for multi-select; clicking the row opens it (= Merge…). The file open in the active tab has a `--surface` row background.
  - "Resolved in this session · N" is a collapsible section at the bottom of the panel: path, method (Merged / Accepted Left / Accepted Right), "Reopen conflict…" on hover/focus.
  - Right: editor tabs (mono name, `--accent` dot when unsaved) above the merge editor or special-conflict panel. No separate "Conflicts" tab.
  - With no tab open, the right side shows "Select a file to resolve" with the first unresolved file's name as a link.
- `ui/RepoWindow.dc.html` — rejected Option A (full-width table as the first tab). Keep it only as a reference for:
  - The banner sub-line states: "N conflicted files · Continue unlocks when none remain" / "All conflicts resolved" in `--ok-fg`. Its `allResolved` variant shows the all-resolved state.
  - The external-change toast: "`<file>` was resolved outside MergeIQ", with Close tab and Reload (Reload is required by the spec, though the mock shows only Close tab).
- Kind badges show `special-conflicts` classes. Until that change lands, omit the badge.

## Risks / Trade-offs

- [Many open tabs memory] → cap 10 tabs, LRU close of clean tabs.
- [Rebase continue triggers hooks that take long] → run async with spinner and cancel-not-possible note; stream stderr.

## Open Questions

- Should Continue offer editing the commit message? v1: default message; later add message editor.
