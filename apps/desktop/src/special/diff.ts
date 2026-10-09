import type { LineHunk } from "../ipc/bindings";
import { middle, trimmed } from "../merge-editor/extensions/structural/diff";
import { splitLines } from "../merge-editor/model/text";

/** One row of a unified diff with old/new line numbers. */
export interface UnifiedRow {
  kind: "ctx" | "del" | "add";
  /** 1-based line number in the base, if the line exists there. */
  oldNo: number | null;
  /** 1-based line number in the other version, if the line exists there. */
  newNo: number | null;
  text: string;
  /** Character range to emphasise (the changed tokens). */
  emphasis?: [number, number];
}

export interface UnifiedDiff {
  rows: UnifiedRow[];
  /** Number of changed regions. */
  changes: number;
  added: number;
  removed: number;
}

/**
 * Unified diff of `before` against `after` from precomputed line hunks, with `context`
 * unchanged lines around every change. Changed lines are emphasised: a removed line paired
 * with an added one shows only the differing tokens.
 */
export function unifiedDiff(
  before: string,
  after: string,
  hunks: readonly LineHunk[],
  context = 3,
): UnifiedDiff {
  const a = splitLines(before);
  const b = splitLines(after);
  const rows: UnifiedRow[] = [];
  let added = 0;
  let removed = 0;
  // Next line (0-based) not yet written, in the base and in the other version. Unchanged
  // lines advance both together, so `bi - ai` is constant across a gap.
  let ai = 0;
  let bi = 0;
  const unchanged = (from: number, to: number) => {
    const delta = bi - ai;
    for (let i = from; i < to; i++)
      rows.push({
        kind: "ctx",
        oldNo: i + 1,
        newNo: i + 1 + delta,
        text: a[i],
      });
  };
  hunks.forEach((h, k) => {
    const start = h.before.start;
    const gap = start - ai;
    if (k === 0) unchanged(Math.max(ai, start - context), start);
    else if (gap > context * 2) {
      unchanged(ai, ai + context);
      rows.push({ kind: "ctx", oldNo: null, newNo: null, text: "⋯" });
      unchanged(start - context, start);
    } else unchanged(ai, start);

    const dels = a.slice(h.before.start, h.before.end);
    const adds = b.slice(h.after.start, h.after.end);
    const pairs = Math.min(dels.length, adds.length);
    dels.forEach((text, d) =>
      rows.push({
        kind: "del",
        oldNo: h.before.start + d + 1,
        newNo: null,
        text,
        emphasis: d < pairs ? middle(text, adds[d]).a : trimmed(text),
      }),
    );
    adds.forEach((text, d) =>
      rows.push({
        kind: "add",
        oldNo: null,
        newNo: h.after.start + d + 1,
        text,
        emphasis: d < pairs ? middle(dels[d], text).b : trimmed(text),
      }),
    );
    removed += dels.length;
    added += adds.length;
    ai = h.before.end;
    bi = h.after.end;
  });
  unchanged(ai, Math.min(a.length, ai + context));
  return { rows, changes: hunks.length, added, removed };
}
