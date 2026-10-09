## Why

Mergetool mode handles one file at a time and has no overview. Users mid-rebase need a home base that shows every conflicted file, what operation is running, lets them resolve files in any order (including batch "take theirs" for generated files), and continue or abort, like IntelliJ's Conflicts dialog plus a status banner.

Depends on: `bootstrap-app`, `git-adapter`, `merge-editor-ui`, `mergetool-cli` (for `mergeiq open`).

## What Changes

- Home view: open repository (folder picker, drag-and-drop, `mergeiq open <dir>`), recent repositories.
- Repository window: operation banner (merge/rebase/cherry-pick/revert with progress) and Continue / Abort / Skip actions.
- Conflicts panel: list of conflicted files with conflict type and per-side change description, filter, group-by-folder, multi-select; Accept Left / Accept Right / Merge… actions.
- Merge editor tabs inside the repo window; auto-advance to the next conflicted file after save.
- Resolved-in-session list with "Reopen conflict".
- Live refresh from `git-adapter` change events; unsaved-work guards.

## Capabilities

### New Capabilities
- `repo-browser`: Repository-level conflict overview, operation control UI, multi-file resolution workflow.

### Modified Capabilities
<!-- none -->

## Impact

- New UI under `apps/desktop/src/repo/` and routes `/` (home), `/repo/:id`.
- Settings additions: `recentRepos` (max 15), `autoAdvanceAfterSave` (default true).
- One window per repository; reuses `<MergeEditor>`.
