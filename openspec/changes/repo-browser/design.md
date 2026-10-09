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

Approved screens are in `ui/` and `../archive/2026-10-08-bootstrap-app/ui/Main.dc.html`; how to read them and precedence rules: `openspec/UI.md`.

- `../archive/2026-10-08-bootstrap-app/ui/Main.dc.html` (home body): "Open repository…" primary button (⌘O / Ctrl+O), drop zone with `mergeiq open` hint, inline "Not a git repository." error (`--danger-*`, dismissible), Recent repositories list (name, mono path, relative time, remove ×; missing paths dimmed with "folder not found" and a Remove button).
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
- `mergeiq open` no longer blocks (see below). If a "wait until the window closes" mode is wanted (e.g. for scripts), add `--wait` later.

## Implementation notes (resolved during apply)

- **`mergeiq open` returns immediately.** `mergetool-cli` made every request block until its window closed. For `open` that is unhelpful (`code .` semantics: focus or open the window, then return), and this spec only requires "focuses or opens a window". The socket handler answers `open` with exit 0 right after `open_repo_window` (exit 2 + message for a non-repo); the invoking process is the app itself only when it had to start one (then it lives until its last window closes, like any `launchedForCli` instance). Merge/resolve requests are unchanged. The request registry no longer has a repository window kind; `/repo/<id>` is now the **repository** id, `/merge/<id>` stays the request id. The `mergetool-cli` spec text is left as archived (it describes the general request flow).
- **One registry, ids = windows.** `RepoRegistry` (canonical worktree root → id, one `Repo` + one watcher per entry; the watcher emits `repo-changed` to that window only). Every git command now takes the repository id as its first argument (`repo_status(repo)`, `conflict_load(repo, path)`, …); the old single-repository `GitState` is gone. Closing a window drops its entry (and watcher); reopening creates a fresh one.
- **Recents** live in `settings.json` (`recentRepos`: path + Unix seconds, max 15, newest first, de-duplicated by root path). `autoAdvanceAfterSave` is part of `MergeEditorSettings` (default true) with a toggle in Settings › General. Recents are recorded on every successful open (picker, drop, recent entry or `mergeiq open`).
- **Dialog plugin.** `tauri-plugin-dialog` provides the folder picker (capability `dialog:default`); drag-and-drop uses the webview's drag-drop event (`onDragDropEvent`).
- **Virtualisation without a dependency.** The conflicts list is a small fixed-row-height window (`VirtualList`, 56px rows, folder headers use the same height) instead of `react-window`: no new dependency, and the list needs nothing else. A 1,200-file scenario renders under 60 rows (Playwright).
- **Row text.** The mock's `Modified / Deleted` is kept visually; the exact "Left: Deleted, Right: Modified" wording from the spec is the span's accessible name and tooltip. Type labels say "Deleted on left/right" (never "by us/them").
- **Save…** in the unsaved-work guard focuses the first unsaved tab and opens that editor's own Apply flow (it can have unresolved-conflict choices), instead of saving silently; the original action (abort, continue, close) is cancelled and can be retried afterwards. **Discard changes** proceeds with the action.
- **Tabs.** Hidden tabs stay mounted (`visibility: hidden`) so editor state survives switching. A new tab at the cap of 10 drops the least recently used *clean* tab; if every tab has unsaved changes it refuses with a message. The editor document is memoised per loaded file because a new document object remounts the editor (and discards its edits).
- **External changes** are detected by diffing each `repo_status` against the open tabs: a tab whose file left the conflict list (and was not resolved by this window) shows "was resolved outside MergeIQ"; a file whose stage object ids changed shows "was changed outside MergeIQ". Both offer Close tab and Reload.
- **Non-text panel.** Anything that is not a plain `BothModified` text file (deleted on one side, added on one/both, binary, symlink, submodule) opens a minimal panel with Accept Left / Accept Right / Delete file (`conflict_delete` → `git rm`). `special-conflicts` replaces it.
- **Banner without an operation.** With no operation and no conflicts the banner shows "On branch `x`" and the panel/editor area say "No conflicts to resolve". `Unknown` (e.g. stash pop) and `am` show no Continue/Abort/Skip for `Unknown`; `am` offers Continue/Abort.
- **E2E (tasks 5.1/5.2).** `tauri-driver` was **not** set up: it needs a Linux display server and WebKitWebDriver in CI that cannot be verified from here, and the Playwright suite (mock backend) already drives every UI flow. The scripted-repository flows run as Rust integration tests (`src-tauri/tests/repo_flows.rs`): the app's own `RepoRegistry`, `repo_service::accept_many` and the git-adapter calls the IPC commands make, against a real merge with 3 conflicts (resolve + batch accept → Continue → merge commit with two parents) and a rebase with 2 conflicting stops (banner state progresses, rebase completes), plus reopen and external-resolution/watcher checks. Unix-only (`#![cfg(unix)]`).
- **Visual baselines** (`e2e/repo-visual.e2e.ts`, Chromium, per platform like the editor's) for conflicts-remaining and all-resolved in dark and light, plus the home view.
