## Why

Not every conflict is two text versions of one file. Modify/delete, renames, binary assets, symlinks, submodule pointers, LFS pointers, huge files and lockfiles all break text-merge UIs, and these are the cases where users usually fall back to the terminal. Handling them properly makes MergeIQ a complete conflict tool.

Depends on: `git-adapter`, `merge-editor-ui`, `repo-browser`.

## What Changes

- Conflict classification beyond index stage types: binary, image, symlink, submodule, LFS pointer, oversized text, lockfile, rename-involved.
- Dedicated resolution panels:
  - Modify/delete: show the surviving side's diff vs base; Keep modified / Delete / Keep and edit.
  - Binary and image: metadata per side; side-by-side image preview (png, jpg, gif, webp, svg as image); pick side.
  - Symlink: show targets; pick side.
  - Submodule: commit per side with subject (if submodule present) and ancestry relation; pick side.
  - LFS pointer: show oid/size per side; pick side.
  - Oversized text (> 20 MB per side): pick side or open externally.
- Rename awareness: show rename info (e.g. "Left renamed to `src/new.ts`") and allow choosing the final path for rename/rename conflicts.
- Lockfile helpers: `go.sum` line-union auto-merge; "Take one side + regenerate" for package-lock.json, pnpm-lock.yaml, yarn.lock, poetry.lock, Cargo.lock, gradle.lockfile with explicit confirmation of the command.

## Capabilities

### New Capabilities
- `special-conflicts`: Classification and resolution UIs for non-standard conflicts.

### Modified Capabilities
<!-- none (repo-browser's minimal non-text panel is replaced by the panels here; behavior defined here) -->

## Impact

- `crates/mergeiq-git`: `classify.rs`, `renames.rs`, `lockfiles.rs`; IPC additions.
- UI: `apps/desktop/src/special/`.
- Running regeneration commands executes project tooling; always user-confirmed, never automatic.
