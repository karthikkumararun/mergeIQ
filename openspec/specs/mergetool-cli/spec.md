# mergetool-cli Specification

## Purpose
TBD - created by archiving change mergetool-cli. Update Purpose after archive.

## Requirements

### Requirement: Mergetool command
`mergeiq merge <BASE> <LOCAL> <REMOTE> <MERGED>` SHALL open a merge editor window with LOCAL as left, REMOTE as right, BASE as base, and save the result to MERGED. A missing or empty BASE file SHALL be treated as empty base. The process SHALL block until the window closes.

#### Scenario: Invoked by git mergetool
- **WHEN** git is configured with `mergetool.mergeiq.cmd = mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED"` and `git mergetool` runs on a conflicted file
- **THEN** a merge window opens for that file and git waits until it closes

#### Scenario: Add/add conflict without base
- **WHEN** BASE path does not exist
- **THEN** editor opens with empty base and the whole file as one conflict

### Requirement: Contextual labels in mergetool mode
When MERGED is inside a git working tree with an operation in progress, the editor SHALL use `git-adapter` side labels and commit context; otherwise labels SHALL fall back to "Local" and "Remote" with file names.

#### Scenario: Mergetool during rebase
- **WHEN** `git mergetool` runs during a rebase
- **THEN** headers show the upstream and replayed-commit labels, not temp file names

### Requirement: Exit codes
The `merge` command SHALL exit `0` when the user saved a fully resolved result (or chose "Mark as resolved anyway"), and `1` when the user cancelled or saved with conflict markers. Invalid arguments SHALL exit `2` with usage on stderr.

#### Scenario: Cancel
- **WHEN** the user closes the window without saving
- **THEN** MERGED is unchanged and the process exits 1

#### Scenario: Resolved
- **WHEN** the user resolves all conflicts and clicks Apply
- **THEN** MERGED contains the result, it is not staged by MergeIQ, and the process exits 0

### Requirement: Resolve command
`mergeiq resolve <PATH>` SHALL open a merge editor for PATH. If PATH is an unmerged index entry, stages SHALL be loaded via `git-adapter` and saving SHALL stage it. Otherwise, if the file contains conflict markers, they SHALL be parsed via `merge-engine` and saving SHALL only write the file. If neither, it SHALL exit 2 with "no conflicts in <PATH>".

#### Scenario: File with markers outside a conflicted index
- **WHEN** `mergeiq resolve notes.txt` runs on a file containing diff3 markers that is not an unmerged entry
- **THEN** the editor opens with left/base/right from the markers and Apply writes the file without staging

### Requirement: Single-instance routing
If MergeIQ is already running, a new CLI invocation SHALL send its request to the running instance over a per-user local socket (Unix domain socket / Windows named pipe), which SHALL open a new window; the invoking process SHALL wait and exit with the code reported by that window. If no instance responds within 2 s, the invocation SHALL start its own instance. When an instance started only for CLI requests has no windows left, it SHALL exit.

#### Scenario: Two mergetool calls
- **WHEN** the app is open and `git mergetool` launches `mergeiq merge ...`
- **THEN** a new window opens in the existing app and the CLI process exits with that window's result code

#### Scenario: Stale socket
- **WHEN** a socket file exists but no instance is listening
- **THEN** the stale socket is removed and a new instance starts

### Requirement: Socket security
The local socket SHALL be accessible only to the current user (Unix: file mode 0600 inside a 0700 directory; Windows: named pipe DACL restricted to current user SID). Requests SHALL be length-prefixed JSON with a protocol version; unknown versions SHALL be rejected.

#### Scenario: Permissions
- **WHEN** the instance creates its socket on macOS
- **THEN** socket file mode is 0600 and its directory mode 0700

### Requirement: CLI installation helper
The app SHALL offer "Install command-line tool". On macOS it SHALL symlink the bundled binary as `mergeiq` into a user-chosen directory (default `~/.local/bin`, `/usr/local/bin` with admin prompt). On Windows the installer SHALL offer adding the install dir to user PATH. The helper SHALL report whether the chosen dir is on PATH.

#### Scenario: Install to ~/.local/bin
- **WHEN** user installs the CLI to `~/.local/bin`
- **THEN** `~/.local/bin/mergeiq` exists pointing at the bundled binary and the app warns if that dir isn't on PATH

### Requirement: Configure git mergetool helper
The app SHALL offer "Configure as git mergetool" which displays the exact `git config --global` commands (`merge.tool`, `mergetool.mergeiq.cmd`, `mergetool.mergeiq.trustExitCode true`, `mergetool.keepBackup false` optional) and executes them only after explicit confirmation.

#### Scenario: Confirmed configuration
- **WHEN** the user confirms
- **THEN** `git config --global merge.tool` returns `mergeiq`
