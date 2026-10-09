import { undo } from "@codemirror/commands";
import type { EditorState, Transaction } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { fixture } from "../__fixtures__";
import { structuralFixtures } from "../__fixtures__/structural";
import { toReplacement } from "../extensions/structural/store";
import {
  applyReplacement,
  applyReplacements,
  applySide,
  canApplyReplacement,
  ignoreChunk,
  counter,
  replacedText,
} from "./actions";
import { chunksOf } from "./session";
import { createResultState } from "./state";

function press(state: EditorState, cmd: typeof undo): EditorState {
  let next = state;
  cmd({ state, dispatch: (tr: Transaction) => (next = tr.state) });
  return next;
}

function proposalsOf(name: string) {
  const f = structuralFixtures[name];
  const outcome = f.resolve.outcome;
  if (typeof outcome === "string") throw new Error("no proposals");
  return { f, proposals: outcome.Proposals };
}

describe("Editor integration", () => {
  it("Toolbar bulk structural", () => {
    const { f, proposals } = proposalsOf("structural-ts");
    expect(f.analysis.chunks).toHaveLength(4);
    expect(proposals).toHaveLength(3);
    const state = createResultState(f.analysis);

    const spec = applyReplacements(
      state,
      f.analysis,
      proposals.map(toReplacement),
      "structural",
    );
    expect(spec).not.toBeNull();
    const after = state.update(spec!).state;

    expect(after.doc.toString()).toBe(
      [
        'import { a, b, c } from "./a";',
        "",
        "export interface Options {",
        "  name: string;",
        "  verbose: boolean;",
        "  retries: number;",
        "}",
        "",
        "export enum Mode {",
        "  Fast,",
        "  Safe,",
        "  Auto,",
        "  Debug",
        "}",
        "",
        "export function run(): number {",
        "  return 1;",
        "}",
        "",
      ].join("\n"),
    );
    // Three chunks resolved with kind `structural`, one conflict remains.
    expect(
      chunksOf(after)
        .filter((c) => c.resolution === "structural")
        .map((c) => c.id),
    ).toEqual([0, 1, 2]);
    expect(counter(after)).toEqual({ changes: 1, conflicts: 1 });

    // One undo reverts all three.
    const undone = press(after, undo);
    expect(undone.doc.toString()).toBe(f.analysis.base.text);
    expect(counter(undone)).toEqual({ changes: 4, conflicts: 4 });
    expect(chunksOf(undone).every((c) => c.resolution === "none")).toBe(true);
  });

  it("Preview apply replaces exactly the previewed text", () => {
    const { f, proposals } = proposalsOf("structural-package-json");
    const state = createResultState(f.analysis);
    const item = toReplacement(proposals[0]);

    expect(replacedText(state, f.analysis, item)).toBe(
      '    "build": "vite build"\n',
    );
    const next = state.update(
      applyReplacement(state, f.analysis, item, "structural")!,
    ).state;
    expect(next.doc.toString()).toContain(
      '    "build": "vite build",\n    "lint": "eslint .",\n    "test": "vitest"\n  },',
    );
    // Other conflicts are untouched.
    expect(counter(next).conflicts).toBe(2);
  });

  it("Proposal spans two chunks", () => {
    const a = fixture("mixed-changes");
    const state = createResultState(a);
    const base = a.base.text.split("\n");
    expect(a.chunks.map((c) => c.kind)).toEqual([
      "OursOnly",
      "Conflict",
      "TheirsOnly",
    ]);
    // A proposal over base lines 1..5 covers the ours-only chunk and the conflict.
    const item = {
      chunkIds: [0, 1],
      baseRange: { start: 1, end: 5 },
      text: "A\r\nB\r\nC\r\n",
    };
    expect(canApplyReplacement(state, a, item)).toBe(true);
    expect(replacedText(state, a, item)).toBe(
      base.slice(1, 5).join("\n") + "\n",
    );
    const next = state.update(
      applyReplacement(state, a, item, "structural")!,
    ).state;
    expect(next.doc.toString()).toBe(
      ["l1", "A", "B", "C", ...base.slice(5)].join("\n"),
    );
    expect(chunksOf(next).map((c) => c.resolution)).toEqual([
      "structural",
      "structural",
      "none",
    ]);
    expect(counter(next).changes).toBe(1);
    // Applying again is rejected: its chunks are resolved.
    expect(canApplyReplacement(next, a, item)).toBe(false);
    expect(applyReplacement(next, a, item, "structural")).toBeNull();
    // One undo reverts both chunks.
    expect(press(next, undo).doc.toString()).toBe(a.base.text);
  });

  it("Stale proposals are skipped", () => {
    const { f, proposals } = proposalsOf("structural-package-json");
    let state = createResultState(f.analysis);
    // The user settles the first conflict on their own.
    state = state.update(ignoreChunk(state, f.analysis, 0)!).state;
    const spec = applyReplacements(
      state,
      f.analysis,
      proposals.map(toReplacement),
      "structural",
    );
    const next = state.update(spec!).state;
    expect(chunksOf(next).map((c) => c.resolution)).toEqual([
      "applied",
      "structural",
      "none",
    ]);
    expect(
      applyReplacements(
        next,
        f.analysis,
        proposals.map(toReplacement),
        "structural",
      ),
    ).toBeNull();
  });

  it("A partly applied conflict still gets the proposal and previews the current text", () => {
    const { f, proposals } = proposalsOf("structural-package-json");
    let state = createResultState(f.analysis);
    state = state.update(applySide(state, f.analysis, 0, "left")!).state;
    expect(chunksOf(state)[0].resolution).toBe("none");
    const item = toReplacement(proposals[0]);
    // The preview diffs against what the Result holds now (left's change applied).
    expect(replacedText(state, f.analysis, item)).toContain(
      '"lint": "eslint ."',
    );
    const next = state.update(
      applyReplacement(state, f.analysis, item, "structural")!,
    ).state;
    expect(next.doc.toString()).toContain(
      '"lint": "eslint .",\n    "test": "vitest"',
    );
    expect(next.doc.toString()).not.toContain('"lint": "eslint ."\n    "test"');
  });

  it("Chunk statuses keep not-applicable sides", () => {
    const a = fixture("mixed-changes");
    const state = createResultState(a);
    const spec = applyReplacement(
      state,
      a,
      { chunkIds: [0], baseRange: { start: 1, end: 2 }, text: "X\n" },
      "ai",
    )!;
    const next = state.update(spec).state;
    const c = chunksOf(next)[0];
    expect(c.rightStatus).toBe("na");
    expect(c.leftStatus).toBe("applied");
    expect(c.resolution).toBe("ai");
    expect(counter(next).changes).toBe(2);
  });
});
