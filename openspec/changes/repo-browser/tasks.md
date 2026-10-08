## 1. Backend window management

- [ ] 1.1 `RepoRegistry`: canonical root → window; `repo_open` focuses or creates window; per-window watcher wiring for `repo-changed`
- [ ] 1.2 Recents in settings (max 15, dedupe, missing-path detection); commands `recents_list`, `recents_remove`
- [ ] 1.3 Wire `mergeiq open <dir>` request to `repo_open`

## 2. Home view

- [ ] 2.1 HomeView: folder picker (tauri dialog plugin), drag-and-drop, recents list with remove; not-a-repo error
- [ ] 2.2 Playwright tests for open flows with IPC mock

## 3. Repository window

- [ ] 3.1 `repoStore` with status/conflicts/tabs/resolved log; refresh on `repo-changed` preserving selection; Vitest
- [ ] 3.2 OperationBanner with labels, progress, Continue/Abort/Skip, confirmations, stderr panel
- [ ] 3.3 ConflictsPanel: virtualized list, type + per-side description, filter, group-by-folder, multi-select, row actions, batch confirm
- [ ] 3.4 Minimal non-text conflict panel (Accept Left / Accept Right / Delete)
- [ ] 3.5 ResolvedSection with method and Reopen conflict

## 4. Editor tabs

- [ ] 4.1 EditorTabs hosting `<MergeEditor>`; focus existing; dirty dot; cap 10 with LRU of clean tabs
- [ ] 4.2 Auto-advance after resolved save; external-change notice on tab
- [ ] 4.3 Unsaved-work guards for tab close, window close, abort, continue

## 5. End-to-end

- [ ] 5.1 tauri-driver E2E (Linux CI): scripted merge with 3 conflicts → resolve via editor + batch accept → Continue → merge commit exists
- [ ] 5.2 E2E rebase with 2 conflicting steps → banner progresses → rebase completes

## 6. UI fidelity

- [ ] 6.1 Match `bootstrap-app/ui/Main.dc.html` (home body) and `ui/RepoSplit.dc.html` (see design.md › UI reference): slim banner and its states, resizable side panel with two-line rows and selection actions, collapsible resolved section, empty editor state, external-change toast with Close tab + Reload; Playwright screenshots for conflicts-remaining and all-resolved states in dark + light
