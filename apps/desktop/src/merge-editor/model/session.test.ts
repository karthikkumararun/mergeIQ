import { undo, redo } from "@codemirror/commands";
import {
  EditorState,
  type Transaction,
  type TransactionSpec,
} from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { fixture } from "../__fixtures__";
import {
  acceptWholeSide,
  applyNonConflicting,
  applySide,
  canResolveSimple,
  counter,
  counterLabel,
  hasManualEdits,
  ignoreSide,
  resolveSimple,
  revertChunk,
} from "./actions";
import { chunksOf } from "./session";
import { createResultState } from "./state";

function run(state: EditorState, spec: TransactionSpec | null): EditorState {
  expect(spec).not.toBeNull();
  return state.update(spec!).state;
}

function press(state: EditorState, cmd: typeof undo): EditorState {
  let next = state;
  cmd({ state, dispatch: (tr: Transaction) => (next = tr.state) });
  return next;
}

function type(state: EditorState, pos: number, text: string): EditorState {
  return state.update({
    changes: { from: pos, insert: text },
    userEvent: "input.type",
  }).state;
}

const text = (s: EditorState) => s.doc.toString();

describe("Initial result content", () => {
  it("Default open", () => {
    const a = fixture("mixed-changes");
    const s = createResultState(a);
    expect(text(s)).toBe(a.base.text);
    expect(counter(s)).toEqual({ changes: 3, conflicts: 1 });
    expect(chunksOf(s).every((c) => c.resolution === "none")).toBe(true);
  });

  it("Auto-apply enabled", () => {
    const a = fixture("mixed-changes");
    const s = createResultState(a, { autoApply: true });
    expect(text(s)).toBe("l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n");
    expect(counter(s)).toEqual({ changes: 1, conflicts: 1 });
    const undone = press(s, undo);
    expect(text(undone)).toBe(a.base.text);
    expect(counter(undone).changes).toBe(3);
  });

  it("Applying all non-conflicting changes is a single undo step", () => {
    const a = fixture("mixed-changes");
    const base = createResultState(a);
    const applied = run(base, applyNonConflicting(base, a, "all"));
    expect(counter(applied).changes).toBe(1);
    const undone = press(applied, undo);
    expect(text(undone)).toBe(a.base.text);
    expect(counter(undone).changes).toBe(3);
  });
});

describe("Per-chunk actions", () => {
  it("Apply then append", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    s = run(s, applySide(s, a, 0, "left"));
    expect(text(s)).toBe("line1\nOURS\nline3\n");
    expect(counter(s).conflicts).toBe(1);
    s = run(s, applySide(s, a, 0, "right"));
    expect(text(s)).toBe("line1\nOURS\nTHEIRS\nline3\n");
    expect(counter(s)).toEqual({ changes: 0, conflicts: 0 });
    expect(chunksOf(s)[0]).toMatchObject({
      leftStatus: "applied",
      rightStatus: "applied",
    });
  });

  it("Ignore both sides", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    s = run(s, ignoreSide(s, a, 0, "left"));
    expect(counter(s).conflicts).toBe(1);
    s = run(s, ignoreSide(s, a, 0, "right"));
    expect(text(s)).toBe(a.base.text);
    expect(counter(s).conflicts).toBe(0);
  });

  it("Undo restores status", () => {
    const a = fixture("simple-conflict");
    const s0 = createResultState(a);
    const s1 = run(s0, applySide(s0, a, 0, "left"));
    const s2 = press(s1, undo);
    expect(text(s2)).toBe(a.base.text);
    expect(chunksOf(s2)[0]).toMatchObject({
      leftStatus: "pending",
      rightStatus: "pending",
      resolution: "none",
    });
    expect(chunksOf(s2)[0].from).toBe(chunksOf(s0)[0].from);
    const s3 = press(s2, redo);
    expect(text(s3)).toBe(text(s1));
    expect(chunksOf(s3)[0].leftStatus).toBe("applied");
  });

  it("Apply on a non-conflicting chunk resolves it", () => {
    const a = fixture("non-overlapping");
    let s = createResultState(a);
    s = run(s, applySide(s, a, 0, "left"));
    expect(text(s)).toBe("l1\nL2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n");
    expect(counter(s).changes).toBe(1);
    expect(applySide(s, a, 1, "left")).toBeNull();
  });

  it("Ignore a non-conflicting chunk leaves text unchanged", () => {
    const a = fixture("non-overlapping");
    let s = createResultState(a);
    s = run(s, ignoreSide(s, a, 0, "left"));
    expect(text(s)).toBe(a.base.text);
    expect(counter(s).changes).toBe(1);
  });

  it("Applying BothSame from either side resolves it", () => {
    const a = fixture("identical-edit");
    let s = createResultState(a);
    s = run(s, applySide(s, a, 0, "right"));
    expect(counter(s).changes).toBe(0);
  });

  it("Insertion-only chunks apply into an empty range", () => {
    const a = fixture("ours-only-insertion");
    let s = createResultState(a);
    const c = chunksOf(s)[0];
    expect(c.from).toBe(c.to);
    s = run(s, applySide(s, a, 0, "left"));
    expect(text(s)).toBe(a.ours.text);
  });

  it("Deleting chunks apply into an empty result range", () => {
    const a = fixture("theirs-only-deletion");
    let s = createResultState(a);
    s = run(s, applySide(s, a, 0, "right"));
    expect(text(s)).toBe(a.theirs.text);
    const c = chunksOf(s)[0];
    expect(c.from).toBe(c.to);
  });

  it("Chunk positions follow edits above them", () => {
    const a = fixture("mixed-changes");
    let s = createResultState(a);
    const before = chunksOf(s)[1];
    s = type(s, 0, "x\ny\nz\n");
    const after = chunksOf(s)[1];
    expect(after.from).toBe(before.from + 6);
    expect(after.to).toBe(before.to + 6);
    expect(chunksOf(s).every((c) => c.resolution === "none")).toBe(true);
  });
});

describe("Manual edits", () => {
  it("Edit resolves chunk", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    const c = chunksOf(s)[0];
    s = type(s, c.from + 1, "!");
    expect(chunksOf(s)[0].resolution).toBe("edited");
    expect(counter(s).conflicts).toBe(0);
    expect(hasManualEdits(s)).toBe(true);
  });

  it("Edits outside chunks leave chunk states unchanged", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    s = type(s, 0, "zzz");
    expect(chunksOf(s)[0].resolution).toBe("none");
    expect(counter(s).conflicts).toBe(1);
  });

  it("Undo of an edit restores the unresolved status", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    s = type(s, chunksOf(s)[0].from, "!");
    s = press(s, undo);
    expect(chunksOf(s)[0].resolution).toBe("none");
    expect(text(s)).toBe(a.base.text);
  });

  it("Typing at an empty insertion point marks the chunk edited", () => {
    const a = fixture("ours-only-insertion");
    let s = createResultState(a);
    s = type(s, chunksOf(s)[0].from, "new\n");
    expect(chunksOf(s)[0].resolution).toBe("edited");
  });

  it("Revert restores base text", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    s = run(s, applySide(s, a, 0, "left"));
    s = type(s, chunksOf(s)[0].from, "?");
    s = run(s, revertChunk(s, a, 0));
    expect(text(s)).toBe(a.base.text);
    expect(chunksOf(s)[0]).toMatchObject({
      leftStatus: "pending",
      rightStatus: "pending",
      resolution: "none",
    });
  });

  it("Revert restores each straddled chunk independently", () => {
    const a = fixture("multi-conflict-file");
    let s = createResultState(a);
    const [c0, c1] = chunksOf(s);
    // select from inside the first chunk through the second chunk and replace it
    s = s.update({
      changes: { from: c0.from + 1, to: c1.to - 1, insert: "X" },
      userEvent: "input.paste",
    }).state;
    expect(chunksOf(s).map((c) => c.resolution)).toEqual(["edited", "edited"]);
    s = run(s, revertChunk(s, a, 0));
    expect(chunksOf(s)[0].resolution).toBe("none");
    expect(chunksOf(s)[1].resolution).toBe("edited");
  });
});

describe("Bulk actions", () => {
  it("Apply non-conflicting: all", () => {
    const a = fixture("mixed-changes");
    let s = createResultState(a);
    s = run(s, applyNonConflicting(s, a, "all"));
    expect(text(s)).toBe("l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n");
    expect(counter(s)).toEqual({ changes: 1, conflicts: 1 });
  });

  it("Apply non-conflicting: left only", () => {
    const a = fixture("mixed-changes");
    let s = createResultState(a);
    s = run(s, applyNonConflicting(s, a, "left"));
    expect(text(s)).toBe("l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n");
    expect(counter(s).changes).toBe(2);
  });

  it("Apply non-conflicting: right only", () => {
    const a = fixture("mixed-changes");
    let s = createResultState(a);
    s = run(s, applyNonConflicting(s, a, "right"));
    expect(text(s)).toBe("l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n");
  });

  it("Apply non-conflicting is null when nothing qualifies", () => {
    const a = fixture("simple-conflict");
    const s = createResultState(a);
    expect(applyNonConflicting(s, a, "all")).toBeNull();
  });

  it("Magic wand", () => {
    const a = fixture("simple-resolvable");
    let s = createResultState(a);
    expect(canResolveSimple(s, a)).toBe(true);
    s = run(s, resolveSimple(s, a));
    expect(text(s)).toBe("a\nfoo(x, y)\nb\nbar(x, y)\nc\nk = 1\nd\n");
    expect(counter(s)).toEqual({ changes: 1, conflicts: 1 });
    expect(chunksOf(s).map((c) => c.resolution)).toEqual([
      "auto",
      "auto",
      "none",
    ]);
    expect(canResolveSimple(s, a)).toBe(false);
    const undone = press(s, undo);
    expect(text(undone)).toBe(a.base.text);
    expect(counter(undone).conflicts).toBe(3);
  });

  it("Magic wand disabled when none qualify", () => {
    const a = fixture("simple-conflict");
    expect(canResolveSimple(createResultState(a), a)).toBe(false);
  });

  it("Accept Left replaces the whole result", () => {
    const a = fixture("mixed-changes");
    let s = createResultState(a);
    s = run(s, acceptWholeSide(s, a, "left"));
    expect(text(s)).toBe(a.ours.text);
    expect(counter(s).changes).toBe(0);
    const c = chunksOf(s)[1];
    expect(s.doc.sliceString(c.from, c.to)).toBe("OURS5\n");
    s = press(s, undo);
    expect(text(s)).toBe(a.base.text);
    expect(counter(s).changes).toBe(3);
  });

  it("Accept Right replaces the whole result", () => {
    const a = fixture("mixed-changes");
    let s = createResultState(a);
    s = run(s, acceptWholeSide(s, a, "right"));
    expect(text(s)).toBe(a.theirs.text);
  });
});

describe("Status counter", () => {
  it("Counter updates", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    expect(counterLabel(counter(s))).toBe("1 change · 1 conflict left");
    s = run(s, applySide(s, a, 0, "left"));
    s = run(s, ignoreSide(s, a, 0, "right"));
    expect(counterLabel(counter(s))).toBe("All changes processed");
  });

  it("Counter wording for several changes", () => {
    const a = fixture("mixed-changes");
    const s = createResultState(a);
    expect(counterLabel(counter(s))).toBe("3 changes · 1 conflict left");
    expect(counterLabel({ changes: 2, conflicts: 0 })).toBe("2 changes left");
  });
});
