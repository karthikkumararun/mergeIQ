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
