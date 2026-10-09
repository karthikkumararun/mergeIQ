import { describe, expect, it } from "vitest";
import { applyNonConflicting } from "../model/actions";
import { chunksOf } from "../model/session";
import { createResultState } from "../model/state";
import { syntheticAnalysis } from "./synthetic";

describe("synthetic analysis fixture", () => {
  it("has consistent chunk ranges", () => {
    const a = syntheticAnalysis(200, 20, true);
    expect(a.chunks.length).toBe(10);
    const s = createResultState(a);
    expect(chunksOf(s)).toHaveLength(10);
    for (const c of a.chunks) {
      expect(c.base.end - c.base.start).toBe(1);
    }
    const next = s.update(applyNonConflicting(s, a, "all")!).state;
    expect(next.doc.lines).toBeGreaterThan(s.doc.lines - 5);
  });

  it("scales to 20k lines with 200 chunks", () => {
    const a = syntheticAnalysis(20_000, 100);
    expect(a.base.lines).toHaveLength(20_000);
    expect(a.chunks).toHaveLength(200);
  });
});
