import { Text } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { fixture } from "../__fixtures__";
import { chunksOf } from "../model/session";
import { createResultState } from "../model/state";
import { mapLine, resultAnchors, sideAnchors } from "./anchors";
import { foldRanges } from "../panes/folds";

describe("Synchronized scrolling", () => {
  it("Scroll alignment maps unchanged lines 1:1", () => {
    const a = fixture("mixed-changes");
    const s = createResultState(a);
    const left = sideAnchors(a, "left", a.ours.lines.length);
    const result = resultAnchors(chunksOf(s), s.doc);
    expect(mapLine(7, left, result)).toBe(7);
    expect(mapLine(0, left, result)).toBe(0);
  });

  it("Scroll alignment interpolates inside a chunk", () => {
    const a = fixture("kotlin-sample");
    const s = createResultState(a);
    const left = sideAnchors(a, "left", a.ours.lines.length);
    const result = resultAnchors(chunksOf(s), s.doc);
    // ours inserts 4 lines at base line 6 (empty result range): the whole ours chunk maps to one result line
    const c = a.chunks[0];
    expect(mapLine(c.ours.start, left, result)).toBe(c.base.start);
    expect(mapLine(c.ours.end, left, result)).toBe(c.base.end);
    expect(mapLine(c.ours.start + 2, left, result)).toBe(c.base.start);
    expect(mapLine(a.ours.lines.length, left, result)).toBe(
      a.base.lines.length,
    );
  });
});

describe("Base pane and unchanged folding", () => {
  it("Collapse unchanged folds regions longer than 8 lines keeping 3 context lines", () => {
    const lines = Array.from({ length: 30 }, (_, i) => `l${i}`);
    const doc = Text.of([...lines, ""]);
    const ranges = foldRanges(doc, [0, 10, 12, 30]);
    // region 0..10 (10 lines): hide lines 3..7 ; region 12..30 (18 lines): hide 15..27
    expect(ranges).toHaveLength(2);
    expect(doc.sliceString(ranges[0].from, ranges[0].to)).toBe(
      lines.slice(3, 7).join("\n"),
    );
    expect(doc.sliceString(ranges[1].from, ranges[1].to)).toBe(
      lines.slice(15, 27).join("\n"),
    );
  });

  it("Does not fold short regions", () => {
    const doc = Text.of([...Array.from({ length: 12 }, (_, i) => `l${i}`), ""]);
    expect(foldRanges(doc, [0, 8, 9, 12])).toEqual([]);
  });
});
