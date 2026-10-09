## Context

Users run MergeIQ against real repos in all states (mid-rebase, worktrees, submodules, odd encodings). Correctness and safety beat speed: never corrupt the index, never lose user edits.

## Goals / Non-Goals

**Goals:**
- Read-heavy operations fast via `gix` (no subprocess per file).
- Mutations via `git` CLI so hooks, config (`core.autocrlf`, filters, LFS), and credentials behave exactly like the user's git.
- Clear human labels for each side.

**Non-Goals:**
- Starting merges/rebases from the UI (future change).
- Rename detection for conflicts (`special-conflicts`).
- rerere (future).

## Decisions

**Reads with gix, writes with git CLI.** gix gives index stage access and blob reads in-process. Writes through libgit2/gix would bypass hooks, clean/smudge filters, and LFS → risky. `git add` applies filters correctly.

**Module layout (`crates/mergeiq-git/src/`)**
```
exec.rs        GitExec: locate binary, version check, run(args, cwd, env) -> Output; no shell
repo.rs        Repo::open(path) -> worktree root, git_dir, common_dir
operation.rs   detect Operation from git_dir files; side labels
conflicts.rs   list unmerged entries -> Vec<ConflictEntry>
blobs.rs       read stage blobs, working-tree bytes
context.rs     commits per side (gix rev walk, path filter)
write.rs       save_resolved, save_unresolved, accept_side, restore_conflict
control.rs     continue/abort/skip
watch.rs       notify-based watcher with debounce
paths.rs       RepoPath (bytes) + PathToken (base64 of bytes) for IPC
```

**Working tree bytes vs stage bytes.** Stage blobs are in repo form (LF, pre-smudge). The editor shows stage content; on save we write bytes and let `git add` apply clean filter. If `core.autocrlf=true` on Windows, we convert result to the working-tree EOL before writing: query `git check-attr eol text -- <path>` and `core.autocrlf`; apply CRLF conversion when needed. Document in tests.

**Atomic write.** Write to `<path>.mergeiq.tmp` in same dir, fsync, rename; copy permissions (exec bit) from existing file or stage mode.

**Labels.** Branch for theirs in merge comes from `MERGE_MSG` first line (`Merge branch 'x'`) fallback to `git name-rev --name-only MERGE_HEAD`. Rebase onto ref from `rebase-merge/onto` + `head-name`; replayed commit from `REBASE_HEAD` (fallback `rebase-merge/stopped-sha`).

**Paths over IPC.** `PathToken` = base64url of raw path bytes; `display` = lossy UTF-8. Avoids breaking on non-UTF-8 paths.

**IPC commands** (`src-tauri/src/commands/git.rs`): `repo_open`, `repo_status` (operation + labels + conflicts), `conflict_load(path)` → stage bytes run through `mergeiq-core::analyze` returning Analysis + labels + context, `conflict_save`, `conflict_accept_side`, `conflict_restore`, `op_continue`, `op_abort`, `op_skip`. Events: `repo-changed`.

**Testing.** Integration tests create temp repos via git CLI scripts (`tests/support/scenario.rs`) for each operation type and conflict type. Run on all CI OSes.

## Risks / Trade-offs

- [Filters/LFS content in stages] → LFS pointer files detected (`version https://git-lfs`) and flagged; full LFS support later.
- [Watcher noise during our own writes] → suppress events for 500 ms after own mutation.
- [Git output localization] → set `LC_ALL=C` for parsed commands.

## Open Questions

- Allow choosing a bundled git? Not now; require system git.

## Implementation notes (resolved during apply)

- **Reads use the `git` CLI, not `gix`.** Design said "reads with gix". Implemented with `git ls-files -u -z`, `git cat-file blob`, `git log`, `git rev-parse` instead: one dependency less (gix is large), identical semantics to the user's git (config, worktrees, submodules), and a handful of subprocesses per conflict load is well within budget. `gix` can replace `conflicts.rs`/`blobs.rs` internals later without API change. `GIT_LITERAL_PATHSPECS=1` is set for every invocation so paths are never globs.
- **Single open repository.** IPC keeps one `Repo` + watcher in `GitState`; `repo_open` replaces it. Multi-repo (repo browser, change 6) can key by root later.
- **`conflict_save(path, text, encoding, stage)`** takes editor text plus `EncodingInfo` and encodes with `mergeiq_core::encode`, rather than raw bytes, to avoid multi-MB JSON number arrays over IPC. `stage=true` is `save_resolved`, `false` is `save_unresolved`.
- **Absent stages analysed as empty text** in `conflict_load` (BothAdded, DeletedBy*). Dedicated handling is `special-conflicts` (change 8). Symlink/gitlink conflicts return `analysis: null` with `analysis_error`.
- **Core specta types**: `usize` fields in `mergeiq-core` IPC types are annotated `specta(type = u32)` (specta forbids BigInt export); text offsets and counts are far below 2^32.
- **Rebase `onto` label** is resolved with `git name-rev`; falls back to short SHA when no branch/tag names the commit exactly.
- **Not done:** LFS pointer detection (risk noted above) is deferred; `GitError` is the IPC error payload (`IpcError::Git`).
- `accept_side` uses `git checkout --ours/--theirs` + `git add` (applies filters/EOL like git) and `git rm` when that side deleted the file.
