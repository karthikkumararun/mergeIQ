## 1. Argument parsing

- [x] 1.1 `cli/args.rs` with clap: merge, resolve, open, --version, --help; exit 2 on invalid args; unit tests
- [x] 1.2 Windows AttachConsole for text output; set `mainBinaryName` to `mergeiq`

## 2. Instance routing

- [x] 2.1 `cli/ipc_socket.rs`: length-prefixed JSON protocol v1, client + listener using `interprocess`, per-user path, 0600/0700 perms (unix), DACL (windows); tests for round-trip, version mismatch, stale socket
- [x] 2.2 Primary/secondary startup flow in `main.rs` with 2 s connect timeout
- [x] 2.3 `cli/requests.rs`: request registry, window per request, exit-code oneshot, `launchedForCli` lifecycle

## 3. Merge and resolve flows

- [x] 3.1 `merge` request: read 4 paths, empty base fallback, analyze, labels from git-adapter when MERGED is in a repo with an operation, else Local/Remote
- [x] 3.2 `merge_request_load` IPC + UI route `/merge/:requestId` embedding `<MergeEditor>`; save writes MERGED atomically; exit codes 0/1
- [x] 3.3 `resolve` request: index stages via git-adapter (save stages) or marker parse (save writes only); "no conflicts" exit 2
- [x] 3.4 `open` request: route to repo browser placeholder (filled by `repo-browser`)
- [x] 3.5 Integration test: script repo with conflict, run `git -c mergetool.mergeiq.cmd=... mergetool` with a test hook that auto-saves via IPC, assert exit and staged file

## 4. Helpers and docs

- [x] 4.1 `cli/install.rs`: symlink install (macOS/Linux), PATH check, admin prompt for /usr/local/bin; Windows installer PATH option in NSIS/WiX config
- [x] 4.2 `cli/git_config.rs`: show commands, execute after confirm; settings UI entries
- [x] 4.3 Docs `docs/git-integration.md`: git config snippets for macOS/Windows, VS Code (`git.mergeEditor` off + terminal `git mergetool`) usage

## 5. UI

- [x] 5.1 Settings › Command line per `ui/CliSetup.dc.html` (install select + PATH warning, git config command block + confirm, usage list) and home "Set up" card status; Playwright test with IPC mock for install-not-on-PATH and confirmed git config flows
