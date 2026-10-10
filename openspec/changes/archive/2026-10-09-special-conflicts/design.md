## Context

`repo-browser` ships a minimal non-text panel. This change replaces it with a dispatcher that selects a panel per `ConflictClass`. Text conflicts still go to `<MergeEditor>`.

## Goals / Non-Goals

**Goals:**
- Every conflict type resolvable inside MergeIQ.
- Safe defaults: no command runs without confirmation; bytes written exactly.

**Non-Goals:**
- Visual image diffing/overlay modes (future).
- Semantic lockfile merging beyond go.sum.

## Decisions

**Classification in Rust (`mergeiq-git/src/classify.rs`).** Uses stage modes from index, first 8 KB of blobs for binary/LFS detection, blob sizes from object headers (no full read for oversized). Lockfile kinds by file name.

**Renames (`renames.rs`).** Only computed for conflicted paths and lazily (on panel open) since rename detection can be slow in big repos; cached per operation head.

**Writing exact bytes.** Binary/LFS/symlink paths bypass `mergeiq-core` and write stage blobs directly; symlink creation via `std::os::unix::fs::symlink` / Windows `CreateSymbolicLinkW` with fallback per `core.symlinks`.

**Submodule ops.** `git update-index --cacheinfo` for gitlinks; ancestry via `git -C <submodule> merge-base --is-ancestor` when the submodule is checked out, else "unknown".

**Image preview.** Backend returns blob bytes base64 (cap 20 MB) with MIME by extension; UI renders `<img src="data:...">`. SVG only via `<img>` so scripts never run.

**Lockfile commands.** `lockfiles.rs` defines defaults; executed via `tokio::process::Command` with args split by `shell-words` (no shell), cwd = lockfile dir, streamed to UI via events, cancellable.

**UI (`apps/desktop/src/special/`)**: `SpecialConflictPanel.tsx` dispatcher; `ModifyDeletePanel`, `BinaryPanel`, `SymlinkPanel`, `SubmodulePanel`, `LfsPanel`, `OversizedPanel`, `RenamePanel`, `LockfilePanel`.

## As built

- **Classification** (`classify.rs`): one streaming `git cat-file --batch` run per listing reads each blob's size (object header) and first 8 KB; results are cached by object id, which is content-addressed and so never stale. "Binary" is the engine's own `decode` verdict on that prefix (a cut multi-byte character is not binary). A text SVG therefore stays a text conflict and opens in the merge editor; `is_image` only matters for content the engine refuses. `go.sum` is `Lockfile { kind: GoSum }`. The class is serialised with a `class` tag (not `kind`) so a lockfile's own `kind` field does not collide.
- **Exact bytes** (`take.rs`): picking a side streams the stage blob into the working tree and stages that very object with `git update-index --cacheinfo`, so no filter (autocrlf, LFS smudge) touches it. `accept_side`, and with it bulk Accept Left/Right, routes every non-text class through the same path. Symlinks are recreated unless `core.symlinks=false` or the OS refuses (then the target text is the file content, still staged as mode 120000); submodules only update the index entry.
- **Renames** (`renames.rs`): modern git stores the *merged text with conflict markers* in both stages of a rename/rename conflict, so contents are read from the commits (base from the original path, ours and theirs from the two destinations). Choosing a path removes the three involved paths, then either stages the chosen path (identical content) or rebuilds a three-way conflict for it with `update-index --index-info` and regenerates the working file with `git checkout -m`, which the merge editor then opens as an ordinary text conflict. Detection runs a whole-tree `git diff -M` per side, lazily, cached per operation head.
- **Lockfile runs** use `std::process` and threads (no tokio): the program is resolved first (a bad command changes nothing on disk), output is streamed as per-window `lockfile-output` / `lockfile-finished` events, cancelling kills the process, and the file is staged only on exit 0. The effective commands live in `settings.json` and are edited in Settings › Lockfiles. After a successful run the panel stays open (output, "Done") and records the resolution itself, because the backend suppresses its own watcher events and would otherwise look like an outside change.
- **Dispatch** (`special/SpecialConflictPanel.tsx`): modify/delete and rename are decided by conflict type (rename candidates are found with `conflict_details`), everything else by class. The old minimal panel remains only for combinations with no dedicated panel (text added on one or both sides, both deleted). Conflict rows show a class badge, and a lockfile opens its panel first; "Merge by hand" switches the tab to the editor.
- **Wording**: panels say "Deleted on right", not the board's "Deleted by them" (ours/theirs stay internal).

## UI reference

Approved screens are in `ui/`; how to read them and precedence rules: `openspec/UI.md`. Every panel opens in a repo-window tab with a header (← Conflicts link, mono path with muted directory, class badge, conflict type), a one-line title saying what happened, and a short explanation.

- `ui/ModifyDelete.dc.html` → `ModifyDeletePanel`: two cards (Left / Right) with status (Modified `--info-fg`, Deleted `--danger-fg`) and the commit that did it; a diff of the surviving side vs base; three action cards: Keep modified (primary), Delete file (danger outline), Keep and edit.
- `ui/ImageConflict.dc.html` → `BinaryPanel`: three cards Base / Left / Right, each with a checkerboard preview, Dimensions, Size, Blob; Use Left / Use Right on the side cards; Base is "Reference only". Non-image binaries show the same cards without the preview.
- `ui/PickSide.dc.html` → `SymlinkPanel`, `LfsPanel`, `OversizedPanel`, drawn side by side for review. In the app each is its own full panel using the same card: rows Base / Left / Right (link target or "regular file"/"deleted"; `oid … · size`; size), a one-line consequence note, Use Left / Use Right, plus "Open in default app" for oversized. On Windows without symlink permission, the symlink note explains the `core.symlinks` fallback.
- `ui/Submodule.dc.html` → `SubmodulePanel`: Base → Left → Right commit cards (SHA, subject, date) in ancestry order, a status line (`--ok-*`) stating the relation, action cards with "Recommended" on the descendant, and the exact `git update-index` command in small mono. When the submodule is not checked out, cards show SHA only and the status line says "Ancestry unknown (submodule not checked out)"; no recommendation.
- `ui/Rename.dc.html` → `RenamePanel`: base path, a radio card per candidate path with side label and commit, an info note when contents also differ, then "Use this path and merge content" (opens the merge editor) or "Use this path" when contents match.
- `ui/GoSum.dc.html`: go.sum panel with "Auto-merge (union) and stage" (primary) and "Merge by hand", a legend (L+ / R+ / − / unchanged counts) and a result preview where removed lines are struck through.
- `ui/Lockfile.dc.html` → `LockfilePanel`, variant `outcome` (success / failed):
  - Side choice as two radio cards ("Take Left and regenerate" / "Take Right and regenerate").
  - Confirmation card: editable command, working directory, the "asked every time / staged only on exit 0" note, Cancel / Run command.
  - Output panel: status bar (exit code, duration, staged or not) above a terminal-style log.
  - Not drawn: while running, the status bar shows a spinner and a Cancel button. The confirmation card also warns that the command may change other files (see Risks).

## Risks / Trade-offs

- [Rename detection mismatch vs git's ort strategy] → info only; resolution acts on actual index entries.
- [Regeneration modifies other files (e.g. node_modules)] → warn in confirmation dialog; only lockfile is staged.
- [Windows symlink privileges] → follow `core.symlinks`, explain in UI.

## Open Questions

- Should go.sum union run automatically on open? Default no; one-click.
