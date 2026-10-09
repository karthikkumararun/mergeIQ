import { describe, expect, it } from "vitest";
import type { ConflictEntry, RepoStatus, SideLabel } from "../ipc/bindings";
import {
  classBadge,
  isLockfile,
  opensInEditor,
  operationSentence,
  sideChanges,
  splitPath,
  typeLabel,
} from "./describe";
import { visibleRows } from "./visibleRows";

const label = (over: Partial<SideLabel>): SideLabel => ({
  role: "r",
  refName: null,
  shortSha: null,
  subject: null,
  author: null,
  gitTerm: "ours",
  ...over,
});

const status = (over: Partial<RepoStatus>): RepoStatus => ({
  root: "/r",
  branch: "main",
  operation: { kind: "Merge" },
  labels: {
    ours: label({ refName: "main" }),
    theirs: label({
      refName: "feature",
      subject: "Add cache",
      shortSha: "e4f5a6b",
    }),
  },
  conflicts: [],
  ...over,
});

const text = (parts: ReturnType<typeof operationSentence>) =>
  parts.map((p) => p.text).join("");

describe("operationSentence", () => {
  it("merge", () => {
    expect(text(operationSentence(status({})))).toBe(
      "Merging feature into main",
    );
  });
  it("rebase with progress and subject", () => {
    const s = status({
      branch: null,
      operation: { kind: "Rebase", step: 3, total: 7, onto: "abc" },
    });
    expect(text(operationSentence(s))).toBe(
      "Rebasing feature onto main — commit 3 of 7: Add cache",
    );
    expect(
      operationSentence(s)
        .filter((p) => p.mono)
        .map((p) => p.text),
    ).toEqual(["feature", "main"]);
  });
  it("cherry-pick and revert", () => {
    expect(
      text(operationSentence(status({ operation: { kind: "CherryPick" } }))),
    ).toBe("Cherry-picking e4f5a6b Add cache");
    expect(
      text(operationSentence(status({ operation: { kind: "Revert" } }))),
    ).toBe("Reverting e4f5a6b Add cache");
  });
  it("no operation has no sentence", () => {
    expect(operationSentence(status({ operation: { kind: "None" } }))).toEqual(
      [],
    );
  });
});

describe("conflict descriptions", () => {
  it("per-side changes use left/right wording", () => {
    expect(sideChanges("DeletedByUs")).toEqual({
      left: "Deleted",
      right: "Modified",
    });
    expect(sideChanges("DeletedByThem")).toEqual({
      left: "Modified",
      right: "Deleted",
    });
    expect(sideChanges("AddedByThem")).toEqual({ left: "—", right: "Added" });
    expect(typeLabel("BothModified")).toBe("Both modified");
  });
  it("only plain both-modified text opens in the editor", () => {
    const entry = (over: Partial<ConflictEntry>): ConflictEntry => ({
      path: "p",
      display: "p",
      conflictType: "BothModified",
      stages: [],
      hasSymlink: false,
      hasGitlink: false,
      class: { class: "Text" },
      ...over,
    });
    expect(opensInEditor(entry({}))).toBe(true);
    expect(opensInEditor(entry({ conflictType: "DeletedByUs" }))).toBe(false);
    expect(opensInEditor(entry({ hasGitlink: true }))).toBe(false);
  });
  it("lockfiles open the editor only by hand; special classes never do", () => {
    const entry = (cls: ConflictEntry["class"]): ConflictEntry => ({
      path: "p",
      display: "p",
      conflictType: "BothModified",
      stages: [],
      hasSymlink: false,
      hasGitlink: false,
      class: cls,
    });
    expect(opensInEditor(entry({ class: "Lockfile", kind: "Pnpm" }))).toBe(
      true,
    );
    expect(isLockfile(entry({ class: "Lockfile", kind: "Pnpm" }))).toBe(true);
    expect(isLockfile(entry({ class: "Text" }))).toBe(false);
    for (const cls of [
      { class: "Binary", isImage: true },
      { class: "Symlink" },
      { class: "Submodule" },
      { class: "LfsPointer" },
      { class: "Oversized" },
    ] as const) {
      expect(opensInEditor(entry(cls))).toBe(false);
    }
  });
  it("badges name the class", () => {
    const badge = (cls: ConflictEntry["class"]) =>
      classBadge({ class: cls } as unknown as ConflictEntry);
    expect(badge({ class: "Text" })).toBeNull();
    expect(badge({ class: "Binary", isImage: true })).toBe("Binary · image");
    expect(badge({ class: "Binary", isImage: false })).toBe("Binary");
    expect(badge({ class: "Lockfile", kind: "Pnpm" })).toBe("Lockfile · pnpm");
    expect(badge({ class: "Lockfile", kind: "GoSum" })).toBe("go.sum");
    expect(badge({ class: "LfsPointer" })).toBe("Git LFS pointer");
    expect(badge({ class: "Oversized" })).toBe("Too large to open");
    expect(badge({ class: "Submodule" })).toBe("Submodule");
    expect(badge({ class: "Symlink" })).toBe("Symlink");
  });
  it("splits paths", () => {
    expect(splitPath("a/b/c.ts")).toEqual({ dir: "a/b/", file: "c.ts" });
    expect(splitPath("c.ts")).toEqual({ dir: "", file: "c.ts" });
  });
});

describe("visibleRows", () => {
  const e = (display: string): ConflictEntry => ({
    path: display,
    display,
    conflictType: "BothModified",
    stages: [],
    hasSymlink: false,
    hasGitlink: false,
    class: { class: "Text" },
  });
  const all = [e("web/b.ts"), e("a.ts"), e("web/a.ts"), e("src/x.ts")];
  it("sorts by path and filters case-insensitively", () => {
    expect(visibleRows(all, "", "flat").map((r) => r.key)).toEqual([
      "a.ts",
      "src/x.ts",
      "web/a.ts",
      "web/b.ts",
    ]);
    expect(visibleRows(all, "WEB/A", "flat").map((r) => r.key)).toEqual([
      "web/a.ts",
    ]);
  });
  it("groups by folder with counts", () => {
    const rows = visibleRows(all, "", "folders");
    expect(
      rows.map((r) => (r.kind === "header" ? `[${r.dir} ${r.count}]` : r.key)),
    ).toEqual([
      "[(repository root) 1]",
      "a.ts",
      "[src/ 1]",
      "src/x.ts",
      "[web/ 2]",
      "web/a.ts",
      "web/b.ts",
    ]);
  });
});
