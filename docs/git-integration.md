# Using MergeIQ from git and the command line

MergeIQ ships one binary, `mergeiq`, that works as a normal app and as a command-line tool.
When MergeIQ is already running, commands are handed to that instance and open a new
window there; otherwise the command starts MergeIQ itself and it quits after the last
window it opened is closed.

## Commands

| Command                                  | What it does                                                                                                                                                                                                                                                                                                              |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mergeiq merge BASE LOCAL REMOTE MERGED` | git mergetool mode. LOCAL is the left pane, REMOTE the right, BASE the common ancestor, and the result is written to MERGED. A missing or empty BASE is an empty base. Blocks until the window closes.                                                                                                                    |
| `mergeiq resolve PATH`                   | Opens one conflicted file. If PATH is unmerged in the git index, the three stages are loaded from git and **Apply** stages the file. Otherwise, if the file contains conflict markers (merge or diff3 style), they are parsed and **Apply** only writes the file. With neither, it exits 2 with `no conflicts in <PATH>`. |
| `mergeiq open [DIR]`                     | Opens (or focuses) the repository window for the repository containing DIR (default: current directory) and returns immediately. Exits 2 if DIR is not inside a git working tree.                                                                                                                                         |
| `mergeiq --version`, `mergeiq --help`    | Print and exit.                                                                                                                                                                                                                                                                                                           |

### Exit codes (`merge` and `resolve`)

| Code | Meaning                                                                                                                                         |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`  | You saved a fully resolved result, or chose **Mark as resolved anyway**.                                                                        |
| `1`  | You cancelled, or saved with conflict markers still in the file.                                                                                |
| `2`  | Invalid arguments or an unusable request, including `open` on a folder that is not a git repository (usage or the reason is printed to stderr). |

## Installing the `mergeiq` command

**Settings › Command line** has an **Install** button.

- **macOS / Linux**: links the bundled binary as `mergeiq` in a folder you choose
  (default `~/.local/bin`; `/usr/local/bin` asks for your administrator password). If the
  folder is not on your `PATH`, the page shows the line to add to your shell start-up file.
- **Windows**: the binary lives in the install folder. The page shows whether that folder is
  on your user `PATH` and can add it. Open a new terminal afterwards.

You can also call the binary directly without installing it:

| Platform | Binary                                                   |
| -------- | -------------------------------------------------------- |
| macOS    | `/Applications/MergeIQ.app/Contents/MacOS/mergeiq`       |
| Windows  | `%LOCALAPPDATA%\MergeIQ\mergeiq.exe` (installer default) |

Always use the binary inside the app bundle in tool configuration, not `open -a MergeIQ`:
`open` loses arguments, stdio and the exit code.

## `git mergetool`

**Settings › Command line › Configure as git mergetool** shows the commands below and runs
them only after you confirm. To do it by hand:

```sh
git config --global merge.tool mergeiq
git config --global mergetool.mergeiq.cmd 'mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED"'
git config --global mergetool.mergeiq.trustExitCode true
# optional: don't leave .orig backup files behind
git config --global mergetool.keepBackup false
```

If `mergeiq` is not on `PATH`, put the full path in `mergetool.mergeiq.cmd`. Git runs
that value through a POSIX shell (also on Windows, via Git Bash), so use forward slashes
and quote paths with spaces:

```sh
# macOS
git config --global mergetool.mergeiq.cmd '/Applications/MergeIQ.app/Contents/MacOS/mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED"'
# Windows (Git Bash path style)
git config --global mergetool.mergeiq.cmd '"C:/Users/you/AppData/Local/MergeIQ/mergeiq.exe" merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED"'
```

Then, in a repository with conflicts:

```sh
git mergetool            # one window per conflicted file, in turn
git mergetool -- a.txt   # just one file
```

`trustExitCode true` lets git use the exit codes above: on `0` git stages MERGED and moves
on; on `1` it reports the merge as failed and leaves the file conflicted. MergeIQ itself
never stages in this mode.

During a rebase, cherry-pick or merge, the window headers show the real branches and
commits (for a rebase, the upstream you are rebasing onto on the left and the commit being
replayed on the right) instead of `LOCAL`/`REMOTE`. Outside a repository they fall back to
**Local** and **Remote** with the file names.

## VS Code

VS Code's built-in merge editor can stay as it is for the UI. To resolve with MergeIQ
instead, either:

1. Turn the built-in editor off in `settings.json` so conflicted files open as plain text:

   ```json
   { "git.mergeEditor": false }
   ```

   and then run `git mergetool` from the integrated terminal; or

2. Keep the built-in editor and run `mergeiq resolve ${file}` for a single file (for
   example from a VS Code task or the terminal).

A VS Code extension is planned; until then both options use only the commands above.

## Troubleshooting

- **`mergeiq: no such command` in git**: `mergeiq` isn't on the `PATH` git sees. Use the full
  binary path in `mergetool.mergeiq.cmd`.
- **git says the merge failed after you saved**: you saved with conflict markers (exit 1).
  Resolve all chunks, or pick **Mark as resolved anyway**.
- **Nothing happens / the command hangs**: a previous MergeIQ instance may have crashed.
  The next invocation removes a stale socket automatically; if a hung process is still
  running, quit it and retry.
