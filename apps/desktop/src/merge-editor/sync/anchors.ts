import type { Text } from "@codemirror/state";
import type { Analysis } from "../../ipc/bindings";
import type { ChunkState } from "../model/types";

export type PaneId = "left" | "right" | "base" | "result";

/**
 * Corresponding line numbers across panes: `[0, c0.start, c0.end, c1.start, c1.end, …, total]`.
 * Segment `2k..2k+1` is unchanged (1:1); `2k+1..2k+2` is chunk `k` (interpolated).
 */
export function sideAnchors(
  analysis: Analysis,
  id: Exclude<PaneId, "result">,
  total: number,
): number[] {
  const pick = (c: Analysis["chunks"][number]) =>
    id === "left" ? c.ours : id === "right" ? c.theirs : c.base;
  const out = [0];
  for (const c of analysis.chunks) {
    const r = pick(c);
    out.push(r.start, r.end);
  }
  out.push(total);
  return out;
}

export function resultAnchors(
  chunks: readonly ChunkState[],
  doc: Text,
): number[] {
  const line = (pos: number) => doc.lineAt(pos).number - 1;
  const out = [0];
  for (const c of chunks) out.push(line(c.from), line(c.to));
  out.push(doc.lines - 1);
  return out;
}

/** Maps a (fractional) line `x` in anchor space `from` to the same place in `to`. */
export function mapLine(
  x: number,
  from: readonly number[],
  to: readonly number[],
): number {
  const n = Math.min(from.length, to.length);
  if (n < 2) return x;
  if (x <= from[0]) return to[0] + (x - from[0]);
  for (let i = 0; i < n - 1; i++) {
    const a = from[i];
    const b = from[i + 1];
    if (x >= a && x < b) {
      return to[i] + ((x - a) / (b - a)) * (to[i + 1] - to[i]);
    }
  }
  return to[n - 1] + (x - from[n - 1]);
}
