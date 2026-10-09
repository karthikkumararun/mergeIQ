import type { Analysis, Chunk, LineRange, SideText } from "../../ipc/bindings";
import { isResolved, lineOffsets } from "../model/session";
import { eolString, sideLines } from "../model/text";
import type { ChunkState, Side } from "../model/types";

/** Visual change type of a chunk in one pane. `res` = resolved (muted). */
export type ChangeType = "ins" | "mod" | "con" | "del" | "res";

export interface PaneChunk {
  id: number;
  from: number;
  to: number;
  type: ChangeType;
  resolved: boolean;
  current: boolean;
  /** Document ranges to emphasize inside the chunk (word-level diff). */
  emphasis: { from: number; to: number }[];
}

export type PaneRole = Side | "base" | "result";

function rangeType(chunk: Chunk, sideRange: LineRange): ChangeType {
  if (chunk.kind === "Conflict") return "con";
  if (sideRange.start === sideRange.end) return "del";
  if (chunk.base.start === chunk.base.end) return "ins";
  return "mod";
}

/** The change type a chunk has in the Result pane (resolved handled by the caller). */
export function resultType(chunk: Chunk): ChangeType {
  return rangeType(
    chunk,
    chunk.kind === "TheirsOnly" ? chunk.theirs : chunk.ours,
  );
}

/** Whether `side` changed relative to base for this chunk (and so is highlighted). */
export function sideChanged(chunk: Chunk, side: Side): boolean {
  return side === "left"
    ? chunk.kind !== "TheirsOnly"
    : chunk.kind !== "OursOnly";
}

function sideData(
  analysis: Analysis,
  role: "left" | "right" | "base",
): { text: SideText; pick: (c: Chunk) => LineRange } {
  switch (role) {
    case "left":
      return { text: analysis.ours, pick: (c) => c.ours };
    case "right":
      return { text: analysis.theirs, pick: (c) => c.theirs };
    case "base":
      return { text: analysis.base, pick: (c) => c.base };
  }
}

/**
 * Converts an offset inside the engine's chunk text (original terminators, UTF-16)
 * into an offset in the `\n`-normalized document: a CRLF pair counts once.
 */
function normalizedOffset(original: string, rel: number): number {
  let removed = 0;
  for (let i = 0; i < rel && i < original.length; i++) {
    if (original[i] === "\r" && original[i + 1] === "\n") removed++;
  }
  return rel - removed;
}

/** The engine's chunk text for `range`: lines with their terminators, minus the last one. */
function originalChunkText(side: SideText, range: LineRange): string {
  const lines = sideLines(side);
  let out = "";
  for (let i = range.start; i < range.end; i++) {
    out += lines[i];
    if (i < range.end - 1) out += eolString(side.lines[i].term);
  }
  return out;
}

function emphasisFor(
  side: SideText,
  range: LineRange,
  rels: { start: number; end: number }[],
  docFrom: number,
): { from: number; to: number }[] {
  if (rels.length === 0) return [];
  const original = originalChunkText(side, range);
  return rels.map((r) => ({
    from: docFrom + normalizedOffset(original, r.start),
    to: docFrom + normalizedOffset(original, r.end),
  }));
}

/**
 * Chunk descriptors for a read-only side (or base) pane. Positions come straight from
 * the analysis line ranges, so they never change; only `resolved`/`current` do.
 */
export function sidePaneChunks(
  analysis: Analysis,
  role: "left" | "right" | "base",
  states: readonly ChunkState[],
  currentId: number | null,
): PaneChunk[] {
  const { text, pick } = sideData(analysis, role);
  const offsets = lineOffsets(sideLines(text));
  const stateById = new Map(states.map((s) => [s.id, s]));
  const out: PaneChunk[] = [];
  for (const chunk of analysis.chunks) {
    if (role !== "base" && !sideChanged(chunk, role)) continue;
    const range = pick(chunk);
    const from = offsets[range.start];
    const to = offsets[range.end];
    const resolved = stateById.has(chunk.id)
      ? isResolved(stateById.get(chunk.id)!)
      : false;
    const type = resolved
      ? "res"
      : role === "base"
        ? resultType(chunk)
        : rangeType(chunk, range);
    let emphasis: { from: number; to: number }[] = [];
    if (!resolved && chunk.fine) {
      if (role === "base") {
        const ranges = [
          ...chunk.fine.ours.base_ranges,
          ...chunk.fine.theirs.base_ranges,
        ];
        emphasis = emphasisFor(analysis.base, range, ranges, from);
      } else {
        const fine = role === "left" ? chunk.fine.ours : chunk.fine.theirs;
        emphasis = emphasisFor(text, range, fine.side_ranges, from);
      }
    }
    out.push({
      id: chunk.id,
      from,
      to,
      type,
      resolved,
      current: chunk.id === currentId,
      emphasis,
    });
  }
  return out;
}

/**
 * Chunk descriptors for the Result pane, from live chunk states. Word emphasis is only
 * shown while the chunk still holds its untouched base text (offsets would otherwise
 * point into edited text).
 */
export function resultPaneChunks(
  analysis: Analysis,
  states: readonly ChunkState[],
  docSlice: (from: number, to: number) => string,
  baseBlockOf: (chunk: Chunk) => string,
  currentId: number | null,
): PaneChunk[] {
  const chunkById = new Map(analysis.chunks.map((c) => [c.id, c]));
  return states.map((s) => {
    const chunk = chunkById.get(s.id)!;
    const resolved = isResolved(s);
    let emphasis: { from: number; to: number }[] = [];
    if (
      !resolved &&
      chunk.fine &&
      docSlice(s.from, s.to) === baseBlockOf(chunk)
    ) {
      const ranges = [
        ...chunk.fine.ours.base_ranges,
        ...chunk.fine.theirs.base_ranges,
      ];
      emphasis = emphasisFor(analysis.base, chunk.base, ranges, s.from);
    }
    return {
      id: s.id,
      from: s.from,
      to: s.to,
      type: resolved ? "res" : resultType(chunk),
      resolved,
      current: s.id === currentId,
      emphasis,
    };
  });
}
