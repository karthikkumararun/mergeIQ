## 1. Classification

- [ ] 1.1 `classify.rs`: ConflictClass priority order, LFS pointer detection, size via object header, lockfile kinds; tests per class
- [ ] 1.2 `renames.rs`: lazy rename detection per side for conflicted paths, cache per operation head; tests for rename/rename and rename/delete
- [ ] 1.3 IPC: include class in conflict listing; `conflict_details(path)` with rename info, stage metadata

## 2. Panels: deletes, binary, links

- [ ] 2.1 Dispatcher `SpecialConflictPanel` replacing repo-browser minimal panel
- [ ] 2.2 ModifyDeletePanel (keep / delete / keep and edit) + backend actions; tests
- [ ] 2.3 BinaryPanel with image previews (sandboxed SVG), exact-byte writes; tests
- [ ] 2.4 SymlinkPanel incl. Windows `core.symlinks` fallback; tests (unix + windows CI)
- [ ] 2.5 SubmodulePanel with ancestry recommendation, `update-index --cacheinfo`; tests
- [ ] 2.6 LfsPanel and OversizedPanel (open externally via opener plugin)

## 3. Renames

- [ ] 3.1 RenamePanel: choose final path, text merge via MergeEditor when contents differ, cleanup other path; E2E test

## 4. Lockfiles

- [ ] 4.1 go.sum union merge (pure fn + tests) and UI action
- [ ] 4.2 `lockfiles.rs`: default commands, settings overrides, no-shell execution, streaming events, cancel
- [ ] 4.3 LockfilePanel with confirmation dialog showing command + cwd + warning; stage on success only; tests with fake command

## 5. UI fidelity

- [ ] 5.1 Match each panel to its board in `ui/` (ModifyDelete, ImageConflict, PickSide → Symlink/LFS/Oversized, Submodule incl. "not checked out" state, Rename, GoSum, Lockfile incl. running + success + failed); shared panel header component; Playwright screenshot per panel in dark + light
