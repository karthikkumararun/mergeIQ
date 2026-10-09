## Why

The merge engine works on three texts; something must find conflicted files in a real repository, load the base/ours/theirs versions from the index, explain *who* each side is (the "ours/theirs" swap during rebase confuses everyone), and write resolutions back safely. This adapter is shared by the mergetool CLI and the repo browser.

Depends on: `bootstrap-app`, `merge-engine`.

## What Changes

- Implement `mergeiq-git` crate:
  - Repository discovery from any path inside a worktree (incl. linked worktrees, submodules).
  - Detection of in-progress operation: merge, rebase (merge/apply backends), cherry-pick, revert, am, stash-apply (best effort), none.
  - Contextual side labels (branch/ref, short SHA, subject, author) per operation, correctly handling rebase's swapped ours/theirs.
  - Listing conflicted paths with conflict type derived from index stages.
  - Reading stage 1/2/3 blobs for a path.
  - File-level commit context: commits on each side touching the path since merge base.
  - Writing resolutions: save working-tree file and stage it; accept whole side; delete; restore conflicted state.
  - Operation control: continue, abort, skip (rebase).
  - Change notifications when index or working tree conflict state changes.
- Tauri IPC commands wrapping the adapter.

## Capabilities

### New Capabilities
- `git-adapter`: Repository/conflict discovery, stage blob reading, operation context and labels, resolution writing, operation control, change watching.

### Modified Capabilities
<!-- none -->

## Impact

- New code in `crates/mergeiq-git` and IPC commands in `apps/desktop/src-tauri/src/commands/git.rs`.
- Dependencies: `gix` (reads), `notify` (watching), `which`; runtime requirement: `git` ≥ 2.30 on PATH (or configured path).
