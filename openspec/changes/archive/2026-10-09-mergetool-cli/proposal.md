## Why

Most users hit conflicts in a terminal or in VS Code. Registering MergeIQ as `git mergetool` (and offering a `mergeiq <file>` launcher) puts the merge editor in their existing flow without changing IDE.

Depends on: `bootstrap-app`, `merge-engine`, `git-adapter`, `merge-editor-ui`.

## What Changes

- CLI parsing in the desktop binary (`clap`):
  - `mergeiq merge <BASE> <LOCAL> <REMOTE> <MERGED> [--wait]` — git mergetool contract.
  - `mergeiq resolve <PATH>` — open one conflicted file (from index stages if in a repo, otherwise by parsing conflict markers).
  - `mergeiq open [DIR]` — open repo browser (implemented in `repo-browser`; CLI wiring here).
  - `mergeiq --version`, `--help`.
- Single-instance handling: subsequent invocations hand the request to the running app over a local socket and block until the window closes, returning the exit code.
- Exit codes compatible with `mergetool.<tool>.trustExitCode = true`.
- In-app actions: "Install command-line tool" and "Configure as git mergetool" (shows exact commands, applies after confirmation).
- Docs for git config and VS Code integration.

## Capabilities

### New Capabilities
- `mergetool-cli`: Command-line entry points, git mergetool integration, single-instance request routing, exit codes, CLI install/config helpers.

### Modified Capabilities
<!-- none -->

## Impact

- `apps/desktop/src-tauri/src/cli/`, window management for per-request merge windows.
- Dependencies: `clap`, `interprocess` (local sockets / named pipes), `tauri-plugin-single-instance` (optional fallback).
- Windows: console attach for `--help`/`--version` output since the binary is a GUI-subsystem app.
