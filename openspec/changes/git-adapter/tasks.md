## 1. Foundations

- [x] 1.1 `exec.rs`: locate git (setting → PATH), parse `git --version`, enforce ≥2.30, run with arg arrays, `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`; tests with fake binary path
- [x] 1.2 `paths.rs`: RepoPath + PathToken (base64url) + lossy display; tests incl. non-UTF-8 bytes (unix only)
- [x] 1.3 Test support: `tests/support/scenario.rs` builder that scripts temp repos (init, commits, branches, merge/rebase/cherry-pick/revert producing conflicts)

## 2. Repository and operation state

- [ ] 2.1 `repo.rs`: open from nested path, worktrees, submodules; reject bare; tests
- [ ] 2.2 `operation.rs`: detect Merge/Rebase(step,total,onto)/CherryPick/Revert/Am/Unknown/None; tests per scenario
- [ ] 2.3 Side labels per operation incl. rebase swap; tests for merge and rebase label scenarios

## 3. Conflicts and content

- [ ] 3.1 `conflicts.rs`: unmerged entries → ConflictType, modes, symlink/gitlink flags; tests for all 7 types
- [ ] 3.2 `blobs.rs`: stage bytes + working-tree bytes; test equality with `git show :N:path`
- [ ] 3.3 `context.rs`: per-side commits since merge base touching path (cap 50); rebase/cherry-pick single commit; tests

## 4. Mutations

- [ ] 4.1 `write.rs`: atomic write with mode preservation; EOL conversion per autocrlf/eol attrs; save_resolved/save_unresolved; tests incl. autocrlf=true
- [ ] 4.2 accept_side incl. delete handling; restore_conflict via `git checkout -m`; tests
- [ ] 4.3 `control.rs`: continue/abort/skip per operation; UnresolvedPaths guard; tests for merge continue and rebase continue

## 5. Watching and IPC

- [ ] 5.1 `watch.rs`: notify watcher on index + state files, 250 ms debounce, self-write suppression; test with external `git add`
- [ ] 5.2 Tauri commands in `src-tauri/src/commands/git.rs` + `repo-changed` event; regenerate TS bindings
- [ ] 5.3 `conflict_load` returns Analysis (via mergeiq-core) + labels + context; integration test over a scripted repo
