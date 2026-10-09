import type { ChangeDesc } from "@codemirror/state";
import type { ChunkState } from "./types";

/**
 * Maps every chunk range through `changes`. `from` sticks to the text before it and
 * `to` follows the text after it, so typing inside a range (or at either edge) grows
 * it. Chunks are kept ordered and non-overlapping afterwards (adjacent chunks may
 * touch).
 */
export function mapChunks(
  chunks: readonly ChunkState[],
  changes: ChangeDesc,
): ChunkState[] {
  const out: ChunkState[] = [];
  let floor = 0;
  for (const c of chunks) {
    let from = changes.mapPos(c.from, -1);
    let to = changes.mapPos(c.to, 1);
    if (from < floor) from = floor;
    if (to < from) to = from;
    out.push({ ...c, from, to });
    floor = to;
  }
  return out;
}

/** Whether any changed range of `changes` touches `[from, to]` (inclusive at both ends). */
export function changeTouches(
  changes: ChangeDesc,
  from: number,
  to: number,
): boolean {
  let hit = false;
  changes.iterChangedRanges((fromA, toA) => {
    if (fromA <= to && toA >= from) hit = true;
  });
  return hit;
}
