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
