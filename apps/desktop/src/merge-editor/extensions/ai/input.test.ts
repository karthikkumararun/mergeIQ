import { describe, expect, it } from "vitest";
import { mockMergeDocument } from "../../../ipc/mock";
import { applyReplacement } from "../../model/actions";
import { chunksOf } from "../../model/session";
import { createResultState } from "../../model/state";
import { buildInput, lineSpan, sideName, unresolvedConflicts } from "./input";

const doc = mockMergeDocument("mixed-changes");
const state = createResultState(doc.analysis);

describe("buildInput", () => {
  it("describes a conflict with contextual labels and exact ranges", () => {
    const ids = unresolvedConflicts(state);
    expect(ids.length).toBeGreaterThan(0);
    const id = ids[0];
    const input = buildInput(doc, doc.analysis, state, id)!;
    const chunk = doc.analysis.chunks.find((c) => c.id === id)!;
    expect(input.path).toBe(doc.displayPath);
    expect(input.left.label).toBe("main");
    expect(input.right.label).toBe("feature");
    expect(input.left.text).toBe(doc.analysis.ours.text);
    expect(input.right.text).toBe(doc.analysis.theirs.text);
    expect(input.base).toBe(doc.analysis.base.text);
    expect(input.left.commits).toEqual([]);
    expect(input.chunk.left).toEqual({
      start: chunk.ours.start,
      end: chunk.ours.end,
    });
    expect(input.chunk.base.start).toBe(chunk.base.start);
    // An unresolved conflict still shows the base in the Result.
    const lines = input.result.split("\n");
    const { start, end } = input.chunk.result;
    expect(lines.slice(start, end).join("\n")).toBe(
      doc.analysis.base.text
        .split("\n")
        .slice(chunk.base.start, chunk.base.end)
        .join("\n"),
    );
  });

  it("returns null for an unknown chunk", () => {
    expect(buildInput(doc, doc.analysis, state, 9999)).toBeNull();
  });

  it("lists unresolved conflicts in document order and drops resolved ones", () => {
    const ids = unresolvedConflicts(state);
    const sorted = [...ids].sort(
      (a, b) =>
        chunksOf(state).find((c) => c.id === a)!.from -
        chunksOf(state).find((c) => c.id === b)!.from,
    );
    expect(ids).toEqual(sorted);
    const chunk = doc.analysis.chunks.find((c) => c.id === ids[0])!;
    const spec = applyReplacement(
      state,
      doc.analysis,
      { chunkIds: [ids[0]], baseRange: chunk.base, text: "resolved\n" },
      "ai",
    )!;
    const after = state.update(spec).state;
    expect(unresolvedConflicts(after)).toEqual(ids.slice(1));
  });

  it("computes line spans for empty and multi-line ranges", () => {
    const s = createResultState(doc.analysis);
    const first = s.doc.line(1);
    expect(lineSpan(s, first.from, first.from)).toEqual({ start: 0, end: 0 });
    expect(lineSpan(s, first.from, first.to + 1)).toEqual({ start: 0, end: 1 });
    const second = s.doc.line(2);
    expect(lineSpan(s, first.from, second.to + 1)).toEqual({
      start: 0,
      end: 2,
    });
  });

  it("falls back to the role when a side has no ref", () => {
    expect(sideName({ ...doc.labels.left, refName: undefined as never })).toBe(
      doc.labels.left.role,
    );
  });
});
