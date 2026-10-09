import type {
  ConflictEntry,
  ConflictType,
  Operation,
  RepoStatus,
} from "../ipc/bindings";

/** What each side did to a file, from the conflict type (Left = ours, Right = theirs). */
export function sideChanges(type: ConflictType): {
  left: string;
  right: string;
} {
  switch (type) {
    case "BothModified":
      return { left: "Modified", right: "Modified" };
    case "BothAdded":
      return { left: "Added", right: "Added" };
    case "DeletedByUs":
      return { left: "Deleted", right: "Modified" };
    case "DeletedByThem":
      return { left: "Modified", right: "Deleted" };
    case "AddedByUs":
      return { left: "Added", right: "—" };
    case "AddedByThem":
      return { left: "—", right: "Added" };
    case "BothDeleted":
      return { left: "Deleted", right: "Deleted" };
  }
}

/** Short conflict-type text for the second line of a row. */
export function typeLabel(type: ConflictType): string {
  switch (type) {
    case "BothModified":
      return "Both modified";
    case "BothAdded":
      return "Both added";
    case "DeletedByUs":
      return "Deleted on left";
    case "DeletedByThem":
      return "Deleted on right";
    case "AddedByUs":
      return "Added on left";
    case "AddedByThem":
      return "Added on right";
    case "BothDeleted":
      return "Both deleted";
  }
}

/** `src/a/b.txt` → `{ dir: "src/a/", file: "b.txt" }`. */
export function splitPath(display: string): { dir: string; file: string } {
  const cut = display.lastIndexOf("/");
  return cut < 0
    ? { dir: "", file: display }
    : { dir: display.slice(0, cut + 1), file: display.slice(cut + 1) };
}

/**
 * Only text conflicts open in the merge editor (a lockfile does once the user chooses to
 * merge it by hand); everything else gets a special panel.
 */
export function opensInEditor(entry: ConflictEntry): boolean {
  return (
    entry.conflictType === "BothModified" &&
    !entry.hasSymlink &&
    !entry.hasGitlink &&
    (entry.class.class === "Text" || entry.class.class === "Lockfile")
  );
}

/** Lockfiles are offered their panel first (regenerate / go.sum union). */
export function isLockfile(entry: ConflictEntry): boolean {
  return entry.class.class === "Lockfile";
}

/** Short badge text for a conflict class, or `null` for plain text. */
export function classBadge(entry: ConflictEntry): string | null {
  const c = entry.class;
  switch (c.class) {
    case "Text":
      return null;
    case "Binary":
      return c.isImage ? "Binary · image" : "Binary";
    case "Symlink":
      return "Symlink";
    case "Submodule":
      return "Submodule";
    case "LfsPointer":
      return "Git LFS pointer";
    case "Oversized":
      return "Too large to open";
    case "Lockfile":
      return c.kind === "GoSum"
        ? "go.sum"
        : `Lockfile · ${lockfileName(c.kind)}`;
  }
}

/** The package manager behind a lockfile kind. */
export function lockfileName(
  kind: "Npm" | "Pnpm" | "Yarn" | "Poetry" | "Cargo" | "Gradle" | "GoSum",
): string {
  return kind === "GoSum" ? "go.sum" : kind.toLowerCase();
}

export interface BannerPart {
  text: string;
  /** Branch names and SHAs render monospace and bold. */
  mono?: boolean;
}

/** The banner's sentence for the operation in progress, with contextual labels. */
export function operationSentence(status: RepoStatus): BannerPart[] {
  const { operation, labels, branch } = status;
  const left = labels.ours.refName ?? labels.ours.role;
  const right = labels.theirs.refName ?? labels.theirs.role;
  const subject = labels.theirs.subject;
  const sha = labels.theirs.shortSha ?? "";
  const tail = (prefix: string): BannerPart[] =>
    subject ? [{ text: ` — ${prefix}${subject}` }] : [];
  switch (operation.kind) {
    case "Merge":
      return [
        { text: "Merging " },
        { text: right, mono: true },
        { text: " into " },
        { text: branch ?? left, mono: true },
      ];
    case "Rebase":
      return [
        { text: "Rebasing " },
        { text: right, mono: true },
        { text: " onto " },
        { text: left, mono: true },
        ...(operation.total > 0
          ? [
              {
                text: ` — commit ${operation.step} of ${operation.total}${subject ? `: ${subject}` : ""}`,
              },
            ]
          : tail("")),
      ];
    case "CherryPick":
      return [
        { text: "Cherry-picking " },
        { text: sha, mono: true },
        ...(subject ? [{ text: ` ${subject}` }] : []),
      ];
    case "Revert":
      return [
        { text: "Reverting " },
        { text: sha, mono: true },
        ...(subject ? [{ text: ` ${subject}` }] : []),
      ];
    case "Am":
      return [{ text: "Applying a patch series" }];
    case "Unknown":
      return [{ text: "Resolving conflicts" }];
    case "None":
      return [];
  }
}

/** The operation's name for button labels: "Abort rebase…". */
export function operationName(operation: Operation): string {
  switch (operation.kind) {
    case "Merge":
      return "merge";
    case "Rebase":
      return "rebase";
    case "CherryPick":
      return "cherry-pick";
    case "Revert":
      return "revert";
    case "Am":
      return "patch series";
    default:
      return "operation";
  }
}
