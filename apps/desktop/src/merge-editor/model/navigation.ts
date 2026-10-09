import { isResolved } from "./session";
import type { ChunkState } from "./types";

export type NavKind = "change" | "conflict";

export interface NavTarget {
  chunk: ChunkState;
  /** True when the search wrapped past the end (or start) of the document. */
  wrapped: boolean;
}

/**
 * Next/previous change (any chunk) or unresolved conflict relative to `head`, the
 * Result cursor offset. A chunk starting exactly at the cursor counts as ahead unless it
 * is the `currentId` the user already navigated to. Wraps around when nothing lies beyond.
 */
export function pickChunk(
  chunks: readonly ChunkState[],
  head: number,
  dir: 1 | -1,
  kind: NavKind,
  currentId: number | null = null,
): NavTarget | null {
  const pool = chunks.filter((c) =>
    kind === "change" ? true : c.kind === "Conflict" && !isResolved(c),
  );
  if (pool.length === 0) return null;
  if (dir === 1) {
    const hit = pool.find(
      (c) => c.from > head || (c.from === head && c.id !== currentId),
    );
    return hit
      ? { chunk: hit, wrapped: false }
      : { chunk: pool[0], wrapped: true };
  }
  for (let i = pool.length - 1; i >= 0; i--) {
    if (pool[i].from < head) return { chunk: pool[i], wrapped: false };
  }
  return { chunk: pool[pool.length - 1], wrapped: true };
}

/** The chunk containing `pos` (inclusive of both ends), if any. */
export function chunkAt(
  chunks: readonly ChunkState[],
  pos: number,
): ChunkState | null {
  return (
    chunks.find(
      (c) => c.from <= pos && pos <= c.to && (c.to > c.from || c.from === pos),
    ) ?? null
  );
}
