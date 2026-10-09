# git-adapter Specification

## Purpose
TBD - created by archiving change git-adapter. Update Purpose after archive.

## Requirements

### Requirement: Git executable discovery
The adapter SHALL locate `git` via the configured path setting or PATH and SHALL verify version ≥ 2.30. If missing or too old it SHALL return `GitError::GitNotFound` or `GitError::GitTooOld { found }`.

#### Scenario: Git missing
- **WHEN** no git executable is found
- **THEN** `GitError::GitNotFound` is returned and the UI can show install guidance

### Requirement: Repository discovery
The adapter SHALL open a repository given any path inside a working tree, including linked worktrees and submodules, and report the worktree root and git dir. Bare repositories SHALL be rejected with `GitError::Bare`.

#### Scenario: Nested path
- **WHEN** opening `<repo>/src/deep/dir`
- **THEN** worktree root is `<repo>`

#### Scenario: Linked worktree
- **WHEN** opening a path in a worktree created by `git worktree add`
- **THEN** operation state is read from that worktree's private git dir

### Requirement: Operation detection
The adapter SHALL report the in-progress operation as one of `Merge`, `Rebase { step, total, onto }`, `CherryPick`, `Revert`, `Am`, `None`, based on git dir state files (`MERGE_HEAD`, `rebase-merge/`, `rebase-apply/`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`). When conflicts exist but no operation file is present (e.g. `git stash pop`), it SHALL report `Unknown`.

#### Scenario: Interactive rebase step
- **WHEN** a rebase stops with conflicts at step 3 of 7
- **THEN** operation is `Rebase { step: 3, total: 7, onto: <sha> }`

### Requirement: Contextual side labels
For each operation the adapter SHALL produce labels for the two sides, each with `role` (what the side means), `ref_name` (if any), `short_sha`, `subject`, `author`. Mapping:
- Merge: ours = current branch (HEAD) "Your branch"; theirs = MERGE_HEAD "Incoming: <branch from MERGE_MSG or sha>"
- Rebase: ours = HEAD "Upstream (rebasing onto <onto ref>)"; theirs = commit being replayed "Your commit being replayed"
- Cherry-pick: ours = HEAD "Current branch"; theirs = CHERRY_PICK_HEAD "Cherry-picked commit"
- Revert: ours = HEAD "Current branch"; theirs = "Revert of <sha>"
The raw git terms ("ours"/"theirs") SHALL be included as secondary info only.

#### Scenario: Rebase labels are swapped correctly
- **WHEN** user on `feature` runs `git rebase main` and hits a conflict
- **THEN** stage-2 label role says upstream `main` and stage-3 label shows the feature commit's subject

#### Scenario: Merge labels
- **WHEN** user on `main` runs `git merge feature` with conflicts
- **THEN** ours label ref is `main` and theirs label ref is `feature`

### Requirement: Conflicted file listing
The adapter SHALL list all unmerged index entries grouped by path with `ConflictType` derived from present stages: `BothModified` (1,2,3), `BothAdded` (2,3), `DeletedByUs` (1,3), `DeletedByThem` (1,2), `AddedByUs` (2), `AddedByThem` (3), `BothDeleted` (1). Each entry SHALL include the file mode per stage and whether any stage is a symlink or gitlink (submodule).

#### Scenario: Modify/delete conflict
- **WHEN** ours deleted `a.txt` and theirs modified it
- **THEN** `a.txt` is listed with `DeletedByUs`

#### Scenario: Non-UTF-8 path
- **WHEN** a conflicted path contains bytes invalid in UTF-8
- **THEN** it is listed with a lossy display name and an opaque path token usable in other calls

### Requirement: Stage blob reading
The adapter SHALL return raw bytes for stage 1, 2 and 3 of a path (absent stages as `None`) and the current working-tree bytes.

#### Scenario: Read three stages
- **WHEN** a path is `BothModified`
- **THEN** three byte buffers are returned matching `git show :1:<path>`, `:2:`, `:3:`

### Requirement: File commit context
The adapter SHALL return, for a conflicted path, the commits that touched it on each side since the merge base (max 50 per side), each with sha, subject, author, date. For rebase/cherry-pick, the "theirs" side SHALL be the single replayed commit.

#### Scenario: Merge context
- **WHEN** both branches modified `a.txt` in two commits each since merge base
- **THEN** each side lists exactly those two commits newest first

### Requirement: Writing resolutions
The adapter SHALL support:
- `save_resolved(path, bytes)`: write bytes atomically to working tree (preserving file mode) and run `git add -- <path>`
- `save_unresolved(path, bytes)`: write bytes without staging
- `accept_side(path, Ours|Theirs)`: write that stage's bytes (or `git rm` if absent on that side) and stage
- `restore_conflict(path)`: `git checkout -m -- <path>` to recreate the conflicted state
All git invocations SHALL use argument arrays (never a shell), `--` before paths, and `GIT_TERMINAL_PROMPT=0`.

#### Scenario: Save resolved
- **WHEN** `save_resolved` is called with merged bytes
- **THEN** working tree file equals bytes and path has only stage 0 in index

#### Scenario: Accept deleted side
- **WHEN** `accept_side(path, Ours)` and ours deleted the file
- **THEN** file is removed from working tree and index

#### Scenario: Restore conflict
- **WHEN** a resolved path is restored
- **THEN** stages 1–3 are back in index and working tree has conflict markers

### Requirement: Operation control
The adapter SHALL run `continue`, `abort` and (rebase only) `skip` for the current operation using the matching git command, with `core.editor=true` so default messages are accepted. It SHALL refuse `continue` while unmerged paths remain (`GitError::UnresolvedPaths`). Git stderr SHALL be returned on failure.

#### Scenario: Continue blocked
- **WHEN** continue is requested with 1 conflicted path remaining
- **THEN** `GitError::UnresolvedPaths { count: 1 }` is returned and git is not invoked

#### Scenario: Merge continue
- **WHEN** all conflicts resolved during a merge and continue is requested
- **THEN** a merge commit is created with the default message

### Requirement: Change watching
The adapter SHALL watch the git dir index file and operation state files and emit a debounced (250 ms) `RepoChanged` event so UIs refresh conflict lists after external changes.

#### Scenario: External resolution
- **WHEN** user runs `git add a.txt` in a terminal
- **THEN** a `RepoChanged` event is emitted within 1 s
