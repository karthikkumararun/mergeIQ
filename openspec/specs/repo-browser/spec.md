# repo-browser Specification

## Purpose
TBD - created by archiving change repo-browser. Update Purpose after archive.

## Requirements

### Requirement: Open repository
The home view SHALL let the user open a repository via folder picker, drag-and-drop of a folder onto the window, a recent-repositories list, or `mergeiq open <dir>`. Opening a path that is not inside a git working tree SHALL show an error and not add it to recents. Opening a repository already open SHALL focus its window.

#### Scenario: Open via CLI
- **WHEN** user runs `mergeiq open ~/code/app` while the app is running
- **THEN** the app focuses or opens a window for that repository

#### Scenario: Not a repo
- **WHEN** user picks a folder that isn't in a git repo
- **THEN** an error "Not a git repository" is shown and recents are unchanged

### Requirement: Recent repositories
The app SHALL keep up to 15 recent repositories (most recent first) in settings, show their name, path and last-opened time, and let the user remove entries. Missing paths SHALL be shown disabled with a remove action.

#### Scenario: Recents ordering
- **WHEN** user opens repo A then repo B
- **THEN** recents list B first then A

### Requirement: Operation banner
The repository window SHALL show a banner describing the in-progress operation with contextual labels, e.g. "Rebasing `feature` onto `main` — commit 3 of 7: <subject>", "Merging `feature` into `main`", "Cherry-picking <sha> <subject>". It SHALL offer Continue (enabled only when no conflicts remain), Abort (with confirmation), and Skip (rebase only, with confirmation). Git errors from these actions SHALL be shown verbatim in an expandable panel.

#### Scenario: Continue enabled after last resolution
- **WHEN** the last conflicted file is saved as resolved during a merge
- **THEN** Continue becomes enabled and the banner shows "All conflicts resolved"

#### Scenario: Rebase continues to next stop
- **WHEN** user clicks Continue during a rebase and the next commit also conflicts
- **THEN** the banner updates to the next step and the conflict list repopulates

#### Scenario: No operation
- **WHEN** the repository has no operation in progress and no conflicts
- **THEN** the window shows the current branch and an empty state "No conflicts to resolve"

### Requirement: Conflicts list
The window SHALL list all conflicted files with path, conflict type, and per-side description (e.g. Left: "Modified", Right: "Deleted") using contextual side names. It SHALL support text filter, flat vs group-by-folder view, sorting by path, and multi-select. Row actions: **Merge…** (open editor; default on double-click/Enter), **Accept Left**, **Accept Right**. Accept actions on multiple selected rows SHALL ask for confirmation listing the files.

#### Scenario: Batch accept theirs
- **WHEN** user selects 5 lockfiles and chooses Accept Right and confirms
- **THEN** all 5 are written from the right side, staged, and removed from the list

#### Scenario: Modify/delete row
- **WHEN** a file is `DeletedByUs`
- **THEN** its row shows Left: "Deleted", Right: "Modified", and Merge… opens the non-text conflict panel

### Requirement: Merge editor tabs
Merge… SHALL open the file's merge editor as a tab in the repository window; re-opening an already-open file SHALL focus its tab. Tabs with unsaved changes SHALL show a dot. After a resolved save, if `autoAdvanceAfterSave` is true, the next unresolved file in list order SHALL open in the same tab.

#### Scenario: Auto-advance
- **WHEN** user saves file 1 of 3 as resolved
- **THEN** the tab now shows file 2's merge editor

### Requirement: Resolved in this session
Files resolved through MergeIQ during the window's lifetime SHALL appear in a "Resolved" section with the resolution method (Merged, Accepted Left, Accepted Right) and a **Reopen conflict** action that restores the conflicted state via `git-adapter` (after confirmation).

#### Scenario: Reopen
- **WHEN** user clicks Reopen conflict on a resolved file and confirms
- **THEN** the file returns to the conflicts list with its original stages

### Requirement: Live refresh
The window SHALL refresh operation state and conflicts on `repo-changed` events. If a file open in a tab is resolved or changed externally, the tab SHALL show a non-blocking notice offering to close or reload.

#### Scenario: External resolution
- **WHEN** the user runs `git checkout --theirs a.txt && git add a.txt` in a terminal
- **THEN** a.txt leaves the conflicts list within 1 s and its open tab shows "Resolved outside MergeIQ"

### Requirement: Unsaved-work guard
Closing a tab or window, aborting the operation, or continuing with unsaved editor changes SHALL prompt to save, discard or cancel.

#### Scenario: Abort with unsaved tab
- **WHEN** a tab has unsaved changes and the user clicks Abort
- **THEN** a prompt appears before the abort confirmation
