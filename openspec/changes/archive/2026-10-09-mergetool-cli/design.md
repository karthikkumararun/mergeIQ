## Context

git mergetool spawns the configured command and waits for it to exit, then trusts the exit code. Tauri apps are GUI binaries; on macOS the CLI must be the binary inside the `.app` bundle; on Windows it's a GUI-subsystem exe. Users may run several mergetool invocations while the app is open.

## Goals / Non-Goals

**Goals:**
- Exact `git mergetool` compatibility with blocking semantics.
- One running app process; many windows.

**Non-Goals:**
- `git difftool` support (future).
- VS Code extension (docs only for now; extension later).

## Decisions

**CLI in the same binary.** `main.rs` parses args with clap before building Tauri. Subcommands produce a `Request { kind: Merge{..} | Resolve{path} | Open{dir}, cwd }`.

**Routing via `interprocess` local sockets instead of tauri-plugin-single-instance.** The plugin forwards argv but makes the second process exit immediately — we need it to block and return a code. Flow:
1. Try connect to `<runtime dir>/mergeiq-<uid>.sock` (macOS/Linux) or `\\.\pipe\mergeiq-<user-sid>` (Windows).
2. Connected → send `{v:1, request}`; await `{v:1, exitCode}`; exit with it.
3. Not connected → become primary: bind listener, start Tauri, handle own request, serve others.
Primary window per request has a `oneshot::Sender<i32>`; on close it sends the code to the waiting client.

**Lifecycle.** `launchedForCli` flag: primary started by a CLI request exits after its last window closes; a primary started from Finder/Start menu stays alive per normal app behavior.

**Window per request.** `WebviewWindow` with route `/merge/<requestId>`; the UI fetches the request's `MergeDocument` via `merge_request_load(requestId)`.

**Mergetool save semantics.** Write MERGED via atomic write (no `git add`; git mergetool stages after exit 0). Markers mode writes markers and returns 1.

**Windows console.** On `--help`/`--version`/errors call `AttachConsole(ATTACH_PARENT_PROCESS)` before printing.

**macOS PATH.** Symlink target: `MergeIQ.app/Contents/MacOS/mergeiq`. Executable name set in tauri config (`mainBinaryName: "mergeiq"`).

**Module layout (`src-tauri/src/cli/`)**: `args.rs` (clap), `ipc_socket.rs` (protocol + listener + client), `requests.rs` (request registry, window opening, exit-code channels), `install.rs` (symlink, PATH check), `git_config.rs`.

## UI reference

Approved screens are in `ui/`; how to read them and precedence rules: `openspec/UI.md`.

- `ui/CliSetup.dc.html` — Settings › Command line (settings shell: left nav General / Merge editor / Command line / Lockfiles / AI; content max-width ~820px).
  - Install section: "Install to" select (`~/.local/bin`, `/usr/local/bin (asks for admin password)`, Choose folder…) + primary Install. After install, a warning box (`--warn-*`) when the folder isn't on PATH, with the exact shell line to add.
  - Mergetool section: the exact `git config --global` commands in a code block; `mergetool.keepBackup false` greyed until its checkbox is ticked; primary "Run N commands…" opens a confirmation, plus "Copy commands".
  - Usage section: `open`, `resolve`, `merge` with one-line descriptions.
- Windows (not drawn): the Install section shows whether the install directory is on the user PATH and offers adding it; the mergetool section is identical.
- Home "Set up" cards (`../archive/2026-10-08-bootstrap-app/ui/Main.dc.html`) link here and show Not installed / Not configured status.
- Mergetool mode reuses the merge editor screens unchanged; the title bar shows the contextual labels or "Local"/"Remote" fallbacks.

## Risks / Trade-offs

- [App launched via `open` on macOS loses argv/stdio] → git invokes the inner binary directly, not `open`; docs show the inner path.
- [Hung client if primary crashes] → client detects socket EOF and exits 1.
- [Antivirus flagging named pipes] → standard per-user pipe; no elevation.

## Open Questions

- Also ship `mergeiq` via Homebrew cask / winget later.
- `mergeiq open` blocks until its window closes, like every other request (spec: "wait and exit with the code reported by that window"). A `code .`-style fire-and-forget may suit `repo-browser` better; revisit there.

## Implementation notes (resolved during apply)

- **Binary name.** `[[bin]] name = "mergeiq"` in `src-tauri/Cargo.toml` plus `mainBinaryName: "mergeiq"`, so `cargo build` and the bundle both produce `mergeiq` (the integration tests run it via `CARGO_BIN_EXE_mergeiq`).
- **Client path never starts Tauri.** `run()` parses args, tries the socket (2 s), and only when nothing answers prepares the request, binds the socket and starts Tauri. Requests that fail to prepare (unreadable file, "no conflicts in <PATH>") exit 2 with the message before any window exists; the same message travels back over the socket as `Response.message` when a running instance rejects it.
- **Wire format.** `u32` big-endian length + JSON; request `{v, kind, ..., cwd}`, response `{v, exitCode, message}`. Unknown/missing `v` gets exit 2 + message. Frames over 1 MiB are rejected. A server that dies mid-request closes the stream and the client exits 1.
- **Socket location (Unix).** `$MERGEIQ_SOCKET_DIR` (test hook), else `$XDG_RUNTIME_DIR/mergeiq`, else `<tmp>/mergeiq-<uid>`; directory created 0700 and verified to be ours (tightened if looser, refused if owned by someone else); socket 0600. macOS cannot set a socket's mode at creation (`interprocess` returns `Unsupported`), so there it is `chmod`ed right after bind; the 0700 parent directory means nobody else can reach it in that window. Windows: pipe `mergeiq-<USERNAME>` (`$MERGEIQ_PIPE_NAME` hook) with SDDL `D:P(A;;GA;;;OW)(A;;GA;;;SY)` (owner + SYSTEM only).
- **Window per request.** Labels `merge-<id>` / `repo-<id>`, URL `/merge/<id>` and `/repo/<id>`; the capability file allows IPC for `merge-*` and `repo-*`. The `main` window is no longer created from `tauri.conf.json` (`"create": false`); `setup` creates it unless the instance was started by a CLI request.
- **Exit code plumbing.** `merge_request_save` writes the file and records the code (Resolved/Force → 0, Markers → 1) on the prepared request; `request_close` destroys the window; the `Destroyed` event completes the request with the recorded code, or 1 if none (cancel, or closing the window with the title-bar button). `launchedForCli` instances exit with the *first* request's code when the last request window closes; if other clients' windows are still open when the first one closes, git waits until they are closed too.
- **Mergetool save** writes MERGED atomically (`mergeiq_git::write_file_atomic`, keeps permissions) and never stages. `resolve` on an unmerged index entry stages through `mergeiq-git` (`save_resolved`; markers mode uses `save_unresolved`); on a marker file it only writes the file. Marker files keep their detected encoding/BOM.
- **Labels.** In a repo with an operation (including `Unknown`, e.g. stash pop) the `git-adapter` labels and file context are used; otherwise `Local`/`Remote` (role) with the file name as subject.
- **Install helper.** Windows has no symlink step: the page checks the user PATH (via PowerShell) and offers "Add to PATH" (`[Environment]::SetEnvironmentVariable(..., 'User')`). The tasks' NSIS/WiX PATH checkbox was **not** built; the in-app button covers the same need without custom installer scripts that can't be verified here. "Choose folder…" is a text field (no dialog plugin dependency). On macOS the "on PATH" check also asks the login shell (`$SHELL -ilc`, 3 s timeout) because GUI apps start with a minimal PATH.
- **Testing seams.** `GitExec::with_env` (scratch `GIT_CONFIG_GLOBAL` for the git-config test), `Registry`/`Dispatcher`/`WindowHost` (a scripted host stands in for the webview in `tests/cli_flows.rs`, which drives the real `mergeiq` binary through a real `git mergetool`), `CliSetupApi` + `mockCliApi` (Settings tests and the `/dev/cli` Playwright route). `MERGEIQ_E2E_PORT` overrides the Playwright dev-server port.
