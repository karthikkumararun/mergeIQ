import { describe, expect, it } from "vitest";
import type { LineHunk } from "../ipc/bindings";
import { unifiedDiff } from "./diff";

const hunk = (b: [number, number], a: [number, number]): LineHunk => ({
  before: { start: b[0], end: b[1] },
  after: { start: a[0], end: a[1] },
});

const lines = (n: number) => Array.from({ length: n }, (_, i) => `l${i + 1}`);
const text = (l: string[]) => l.map((x) => `${x}\n`).join("");

describe("Modify/delete diff", () => {
  it("shows a change with context and old/new line numbers", () => {
    const before = [
      "data class Coupon(",
      "    val code: String,",
      "    val expiresAt: LocalDateTime,",
      "    val percent: Int,",
      ")",
    ];
    const after = [
      "data class Coupon(",
      "    val code: String,",
      "    val expiresAt: Instant,",
      "    val zone: ZoneId = ZoneOffset.UTC,",
      "    val percent: Int,",
      ")",
    ];
    const d = unifiedDiff(text(before), text(after), [hunk([2, 3], [2, 4])]);
    expect(d.changes).toBe(1);
    expect(d.added).toBe(2);
    expect(d.removed).toBe(1);
    expect(
      d.rows.map((r) => `${r.kind} ${r.oldNo ?? "-"}/${r.newNo ?? "-"}`),
    ).toEqual([
      "ctx 1/1",
      "ctx 2/2",
      "del 3/-",
      "add -/3",
      "add -/4",
      "ctx 4/5",
      "ctx 5/6",
    ]);
    // The paired lines emphasise only the changed token.
    const del = d.rows[2];
    expect(del.text.slice(...del.emphasis!)).toBe("LocalDateTime");
    const add = d.rows[3];
    expect(add.text.slice(...add.emphasis!)).toBe("Instant");
    // The unpaired new line is emphasised without its indentation.
    expect(d.rows[4].text.slice(...d.rows[4].emphasis!)).toBe(
      "val zone: ZoneId = ZoneOffset.UTC,",
    );
  });

  it("limits context to three lines and elides long unchanged gaps", () => {
    const before = lines(40);
    const after = [...before];
    after[1] = "CHANGED-A";
    after[30] = "CHANGED-B";
    const d = unifiedDiff(text(before), text(after), [
      hunk([1, 2], [1, 2]),
      hunk([30, 31], [30, 31]),
    ]);
    const shown = d.rows
      .filter((r) => r.kind === "ctx" && r.oldNo !== null)
      .map((r) => r.oldNo);
    expect(shown).toEqual([1, 3, 4, 5, 28, 29, 30, 32, 33, 34]);
    expect(d.rows.some((r) => r.text === "⋯")).toBe(true);
  });

  it("keeps gaps shorter than twice the context whole", () => {
    const before = lines(12);
    const after = [...before];
    after[2] = "X";
    after[8] = "Y";
    const d = unifiedDiff(text(before), text(after), [
      hunk([2, 3], [2, 3]),
      hunk([8, 9], [8, 9]),
    ]);
    expect(d.rows.some((r) => r.text === "⋯")).toBe(false);
    expect(d.rows.filter((r) => r.kind === "ctx").map((r) => r.oldNo)).toEqual([
      1, 2, 4, 5, 6, 7, 8, 10, 11, 12,
    ]);
  });

  it("numbers lines after an insertion by the shift it caused", () => {
    const d = unifiedDiff(
      text(["a", "b", "c"]),
      text(["a", "NEW1", "NEW2", "b", "c"]),
      [hunk([1, 1], [1, 3])],
    );
    expect(
      d.rows.map((r) => `${r.kind} ${r.oldNo ?? "-"}/${r.newNo ?? "-"}`),
    ).toEqual(["ctx 1/1", "add -/2", "add -/3", "ctx 2/4", "ctx 3/5"]);
  });

  it("handles a file with no changes or no lines", () => {
    expect(unifiedDiff("", "", [])).toEqual({
      rows: [],
      changes: 0,
      added: 0,
      removed: 0,
    });
    expect(unifiedDiff(text(["a"]), text(["a"]), []).rows).toHaveLength(1);
  });
});
