# special-conflicts Specification

## Purpose
TBD - created by archiving change special-conflicts. Update Purpose after archive.

## Requirements

### Requirement: Extended classification
For each conflicted path the adapter SHALL compute a `ConflictClass` in priority order: `Submodule` (any stage gitlink), `Symlink` (any stage mode 120000), `LfsPointer` (any stage is a git-lfs pointer), `Binary` (engine binary detection on any stage), `Oversized` (any stage > 20 MB), `Lockfile` (file name matches known lockfiles), `Text`. Independently it SHALL report `ConflictType` (from `git-adapter`) and optional `RenameInfo`.

#### Scenario: PNG conflict
- **WHEN** both sides modify `logo.png`
- **THEN** class is `Binary` with `is_image = true`

#### Scenario: Lockfile
- **WHEN** `pnpm-lock.yaml` is `BothModified`
- **THEN** class is `Lockfile { kind: Pnpm }`

### Requirement: Modify/delete resolution
For `DeletedByUs` and `DeletedByThem`, the UI SHALL show which side deleted the file (with label and commit) and a diff of the surviving side vs base, and offer **Keep modified** (write surviving side, stage), **Delete** (`git rm`), **Keep and edit** (write surviving side unstaged and open it in a plain editor tab, then mark resolved on save).

#### Scenario: Delete chosen
- **WHEN** user chooses Delete on a `DeletedByThem` file
- **THEN** the file is removed from working tree and index and leaves the conflict list

### Requirement: Binary and image resolution
For `Binary`, the UI SHALL show per side: size, blob SHA, and for images (png, jpg, jpeg, gif, webp, svg, ico) a preview of base, left and right with dimensions, and offer **Use Left** / **Use Right**. SVG SHALL be rendered as an image in a sandboxed `<img>` (never inline DOM).

#### Scenario: Image preview
- **WHEN** opening a conflicting PNG
- **THEN** three previews with pixel dimensions are shown and Use Right writes the right blob bytes exactly

### Requirement: Symlink resolution
For `Symlink`, the UI SHALL show each side's link target (or "regular file"/"deleted") and offer Use Left / Use Right, recreating the symlink (or file) accordingly and staging it. On Windows without symlink permission, it SHALL follow `core.symlinks` behavior (write target as file content).

#### Scenario: Symlink targets differ
- **WHEN** left targets `../a` and right targets `../b`
- **THEN** choosing Use Right results in the link pointing to `../b` and staged with mode 120000

### Requirement: Submodule resolution
For `Submodule`, the UI SHALL show the commit for base/left/right, with subject and date when the submodule is checked out, and the ancestry relation (left ancestor of right, right ancestor of left, diverged). It SHALL offer Use Left / Use Right (updating the index gitlink via `git update-index --cacheinfo 160000,<sha>,<path>`) and recommend the descendant when one side is an ancestor of the other.

#### Scenario: Fast-forwardable submodule
- **WHEN** left's submodule commit is an ancestor of right's
- **THEN** the UI marks Use Right as recommended

### Requirement: LFS pointer resolution
For `LfsPointer`, the UI SHALL show oid and size per side and offer Use Left / Use Right, writing the pointer file of that side and staging it so LFS smudges the right content on checkout.

#### Scenario: LFS pick
- **WHEN** user chooses Use Left for an LFS-tracked file
- **THEN** the staged blob equals left's pointer content

### Requirement: Oversized files
For `Oversized`, the UI SHALL NOT load contents into the editor; it SHALL show sizes and offer Use Left / Use Right / Open in external editor (system default app with the working-tree file).

#### Scenario: 50 MB text file
- **WHEN** a conflicted 50 MB CSV is opened
- **THEN** the oversized panel appears and no editor is created

### Requirement: Rename awareness
The adapter SHALL detect renames involved in a conflict by running rename detection (`git diff --name-status -M` from merge base to each side) for conflicted paths and report `RenameInfo { side, from, to }`. For rename/rename to different paths, the UI SHALL show both candidate paths, let the user choose the final path, merge content as a text conflict if contents also differ, and remove the other path from index and working tree.

#### Scenario: Rename/rename
- **WHEN** left renames `a.ts`→`b.ts` and right renames `a.ts`→`c.ts`
- **THEN** UI asks for final path among `b.ts`, `c.ts`, and after choosing `c.ts`, only `c.ts` exists and is staged

### Requirement: go.sum union merge
For `go.sum`, the app SHALL offer **Auto-merge (union)**: result is the sorted, deduplicated union of lines from left and right, minus lines removed by one side and unchanged by the other.

#### Scenario: go.sum union
- **WHEN** both sides add different module checksums
- **THEN** auto-merge result contains both, sorted, and the file is staged

### Requirement: Lockfile regeneration
For `Lockfile` kinds npm (`package-lock.json`), pnpm (`pnpm-lock.yaml`), yarn (`yarn.lock`), poetry (`poetry.lock`), cargo (`Cargo.lock`), gradle (`gradle.lockfile`), the UI SHALL offer **Take <side> and regenerate**, showing the exact command (defaults: `npm install --package-lock-only`, `pnpm install --lockfile-only`, `yarn install --mode update-lockfile`, `poetry lock --no-update`, `cargo update --workspace`, `./gradlew dependencies --write-locks`) and working directory before running. Commands SHALL run only after explicit user confirmation each time, with output streamed and failures shown without staging. Commands SHALL be editable in settings.

#### Scenario: Regenerate pnpm lockfile
- **WHEN** user chooses Take Right and regenerate for `pnpm-lock.yaml` and confirms the command
- **THEN** right side is written, `pnpm install --lockfile-only` runs in the lockfile's directory, and on exit 0 the lockfile is staged

#### Scenario: Regeneration failure
- **WHEN** the command exits non-zero
- **THEN** output is shown, the file is not staged, and the conflict remains listed
