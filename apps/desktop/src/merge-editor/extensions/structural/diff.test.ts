import { describe, expect, it } from "vitest";
import { diffRows } from "./diff";

describe("Preview diff", () => {
  it("shows an unchanged line, the comma that was added and the new lines", () => {
    const rows = diffRows(
      ['    "build": "vite build"'],
      [
        '    "build": "vite build",',
        '    "lint": "eslint .",',
        '    "test": "vitest"',
      ],
    );
    expect(rows.map((r) => r.kind)).toEqual(["del", "add", "add", "add"]);
    // The paired line only emphasises the comma.
    expect(rows[0].emphasis).toBeUndefined();
    const comma = rows[1];
    expect(comma.text.slice(...comma.emphasis!)).toBe(",");
    // Unpaired new lines are emphasised without their indentation.
    expect(rows[2].text.slice(...rows[2].emphasis!)).toBe(
      '"lint": "eslint .",',
    );
    expect(rows[3].text.slice(...rows[3].emphasis!)).toBe('"test": "vitest"');
  });

  it("keeps common lines as context", () => {
    const rows = diffRows(["a", "b", "c"], ["a", "x", "c"]);
    expect(rows.map((r) => `${r.kind}:${r.text}`)).toEqual([
      "ctx:a",
      "del:b",
      "add:x",
      "ctx:c",
    ]);
    expect(rows[1].emphasis).toEqual([0, 1]);
    expect(rows[2].emphasis).toEqual([0, 1]);
  });

  it("handles pure additions, pure removals and identical input", () => {
    expect(diffRows([], ["x"]).map((r) => r.kind)).toEqual(["add"]);
    expect(diffRows(["x"], []).map((r) => r.kind)).toEqual(["del"]);
    expect(diffRows(["x"], ["x"]).map((r) => r.kind)).toEqual(["ctx"]);
    expect(diffRows([], [])).toEqual([]);
  });

  it("falls back to replace-all for huge inputs", () => {
    const a = Array.from({ length: 700 }, (_, i) => `a${i}`);
    const b = Array.from({ length: 700 }, (_, i) => `b${i}`);
    const rows = diffRows(a, b);
    expect(rows).toHaveLength(1400);
    expect(rows.filter((r) => r.kind === "del")).toHaveLength(700);
  });
});
