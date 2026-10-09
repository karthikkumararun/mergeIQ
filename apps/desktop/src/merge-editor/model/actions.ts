import {
  EditorState,
  type ChangeSpec,
  type TransactionSpec,
} from "@codemirror/state";
import type { Analysis, Chunk, LineRange } from "../../ipc/bindings";
import {
  chunksOf,
  isResolved,
  lineOffsets,
  pendingStatus,
  type ChunkPatch,
  patchChunks,
} from "./session";
import { block, normalizeEol, sideLines } from "./text";
import type { ChunkState, ChunkStatus, Side } from "./types";

const other = (side: Side): Side => (side === "left" ? "right" : "left");

function statusKey(side: Side) {
  return side === "left" ? "leftStatus" : "rightStatus";
}

function sideRange(chunk: Chunk, side: Side): LineRange {
  return side === "left" ? chunk.ours : chunk.theirs;
}

function sideBlock(analysis: Analysis, chunk: Chunk, side: Side): string {
  const text = side === "left" ? analysis.ours : analysis.theirs;
  return block(sideLines(text), sideRange(chunk, side));
}

function find(state: EditorState, analysis: Analysis, id: number) {
  const cs = chunksOf(state).find((c) => c.id === id);
  const chunk = analysis.chunks.find((c) => c.id === id);
  return cs && chunk ? { cs, chunk } : null;
}

const MANUAL = new Set(["edited", "auto", "whole-file"]);

function withResolution(
  cs: ChunkState,
  status: Partial<ChunkStatus>,
): ChunkStatus {
  const merged = { ...cs, ...status };
  const resolution = MANUAL.has(merged.resolution)
    ? merged.resolution
    : merged.leftStatus !== "pending" && merged.rightStatus !== "pending"
      ? "applied"
      : "none";
  return {
    leftStatus: merged.leftStatus,
    rightStatus: merged.rightStatus,
    resolution,
  };
}

function spec(
  changes: ChangeSpec,
  patches: ChunkPatch[],
  userEvent: string,
): TransactionSpec {
  return {
    changes,
    effects: patchChunks.of(patches),
    userEvent,
    scrollIntoView: false,
  };
}

/** Whether `side` of chunk `id` can still be applied/appended/ignored. */
export function canActOn(state: EditorState, id: number, side: Side): boolean {
  const cs = chunksOf(state).find((c) => c.id === id);
  if (!cs || isResolved(cs)) return false;
  return cs[statusKey(side)] === "pending";
}

/** `append` when the chunk is a conflict whose other side was already applied. */
export function isAppend(state: EditorState, id: number, side: Side): boolean {
  const cs = chunksOf(state).find((c) => c.id === id);
  return (
    !!cs && cs.kind === "Conflict" && cs[statusKey(other(side))] === "applied"
  );
}

export function applySide(
  state: EditorState,
  analysis: Analysis,
  id: number,
  side: Side,
): TransactionSpec | null {
  const hit = find(state, analysis, id);
  if (!hit || !canActOn(state, id, side)) return null;
  const { cs, chunk } = hit;
  const text = sideBlock(analysis, chunk, side);
  const append = isAppend(state, id, side);
  const changes: ChangeSpec = append
    ? { from: cs.to, insert: text }
    : { from: cs.from, to: cs.to, insert: text };
  const status: Partial<ChunkStatus> = { [statusKey(side)]: "applied" };
  if (cs.kind === "BothSame") status[statusKey(other(side))] = "applied";
  return spec(
    changes,
    [{ id, status: withResolution(cs, status) }],
    "merge.apply",
  );
}

export function ignoreSide(
  state: EditorState,
  analysis: Analysis,
  id: number,
  side: Side,
): TransactionSpec | null {
  const hit = find(state, analysis, id);
  if (!hit || !canActOn(state, id, side)) return null;
  const { cs } = hit;
  const status: Partial<ChunkStatus> = { [statusKey(side)]: "ignored" };
  if (cs.kind === "BothSame") status[statusKey(other(side))] = "ignored";
  return spec([], [{ id, status: withResolution(cs, status) }], "merge.ignore");
}

/** Ignores every still-pending side of a chunk (one step). */
export function ignoreChunk(
  state: EditorState,
  analysis: Analysis,
  id: number,
): TransactionSpec | null {
  const hit = find(state, analysis, id);
  if (!hit || isResolved(hit.cs)) return null;
  const status: Partial<ChunkStatus> = {};
  if (hit.cs.leftStatus === "pending") status.leftStatus = "ignored";
  if (hit.cs.rightStatus === "pending") status.rightStatus = "ignored";
  return spec(
    [],
    [{ id, status: withResolution(hit.cs, status) }],
    "merge.ignore",
  );
}

export function canRevert(cs: ChunkState): boolean {
  return (
    isResolved(cs) ||
    cs.leftStatus === "applied" ||
    cs.rightStatus === "applied"
  );
}

export function revertChunk(
  state: EditorState,
  analysis: Analysis,
  id: number,
): TransactionSpec | null {
  const hit = find(state, analysis, id);
  if (!hit || !canRevert(hit.cs)) return null;
  const { cs, chunk } = hit;
  const baseText = block(sideLines(analysis.base), chunk.base);
  return spec(
    { from: cs.from, to: cs.to, insert: baseText },
    [{ id, status: pendingStatus(cs.kind) }],
    "merge.revert",
  );
}

export type NonConflictingScope = "all" | "left" | "right";

/** Unresolved non-conflicting chunks that `scope` would apply, with the side used. */
function nonConflicting(state: EditorState, scope: NonConflictingScope) {
  const out: { cs: ChunkState; side: Side }[] = [];
  for (const cs of chunksOf(state)) {
    if (isResolved(cs) || cs.kind === "Conflict") continue;
    if (cs.kind === "OursOnly" && scope !== "right")
      out.push({ cs, side: "left" });
    else if (cs.kind === "TheirsOnly" && scope !== "left")
      out.push({ cs, side: "right" });
    else if (cs.kind === "BothSame")
      out.push({ cs, side: scope === "right" ? "right" : "left" });
  }
  return out;
}

export function countNonConflicting(
  state: EditorState,
  scope: NonConflictingScope,
): number {
  return nonConflicting(state, scope).length;
}

export function applyNonConflicting(
  state: EditorState,
  analysis: Analysis,
  scope: NonConflictingScope,
): TransactionSpec | null {
  const todo = nonConflicting(state, scope);
  if (todo.length === 0) return null;
  const changes: ChangeSpec[] = [];
  const patches: ChunkPatch[] = [];
  for (const { cs, side } of todo) {
    const chunk = analysis.chunks.find((c) => c.id === cs.id)!;
    changes.push({
      from: cs.from,
      to: cs.to,
      insert: sideBlock(analysis, chunk, side),
    });
    const status: Partial<ChunkStatus> =
      cs.kind === "BothSame"
        ? { leftStatus: "applied", rightStatus: "applied" }
        : { [statusKey(side)]: "applied" };
    patches.push({ id: cs.id, status: withResolution(cs, status) });
  }
  return spec(changes, patches, "merge.apply-all");
}

function simpleText(chunk: Chunk): string | null {
  const s = chunk.simple;
  if (!s || s === "Unresolvable") return null;
  const text = normalizeEol(s.Resolved);
  return text === "" ? "" : `${text}\n`;
}

function simpleCandidates(state: EditorState, analysis: Analysis) {
  const out: { cs: ChunkState; text: string }[] = [];
  for (const cs of chunksOf(state)) {
    if (isResolved(cs) || cs.kind !== "Conflict") continue;
    const chunk = analysis.chunks.find((c) => c.id === cs.id);
    const text = chunk ? simpleText(chunk) : null;
    if (text !== null) out.push({ cs, text });
  }
  return out;
}

export function canResolveSimple(
  state: EditorState,
  analysis: Analysis,
): boolean {
  return simpleCandidates(state, analysis).length > 0;
}

export function resolveSimple(
  state: EditorState,
  analysis: Analysis,
): TransactionSpec | null {
  const todo = simpleCandidates(state, analysis);
  if (todo.length === 0) return null;
  return spec(
    todo.map(({ cs, text }) => ({ from: cs.from, to: cs.to, insert: text })),
    todo.map(({ cs }) => ({
      id: cs.id,
      status: { resolution: "auto" as const },
    })),
    "merge.resolve-simple",
  );
}

/** Replaces the whole Result with one side and marks every chunk resolved. */
export function acceptWholeSide(
  state: EditorState,
  analysis: Analysis,
  side: Side,
): TransactionSpec {
  const lines = sideLines(side === "left" ? analysis.ours : analysis.theirs);
  const offsets = lineOffsets(lines);
  const text = lines.map((l) => `${l}\n`).join("");
  const patches: ChunkPatch[] = analysis.chunks.map((chunk) => {
    const r = sideRange(chunk, side);
    return {
      id: chunk.id,
      status: {
        leftStatus: side === "left" ? "applied" : "ignored",
        rightStatus: side === "right" ? "applied" : "ignored",
        resolution: "whole-file",
      },
      range: { from: offsets[r.start], to: offsets[r.end] },
    } satisfies ChunkPatch;
  });
  return {
    changes: { from: 0, to: state.doc.length, insert: text },
    effects: patchChunks.of(patches),
    userEvent: "merge.accept-side",
    scrollIntoView: false,
  };
}

/** Whether the user has hand-edited any chunk (used to confirm destructive bulk actions). */
export function hasManualEdits(state: EditorState): boolean {
  return chunksOf(state).some((c) => c.resolution === "edited");
}

export interface Counter {
  changes: number;
  conflicts: number;
}

export function counter(state: EditorState): Counter {
  let changes = 0;
  let conflicts = 0;
  for (const c of chunksOf(state)) {
    if (isResolved(c)) continue;
    changes++;
    if (c.kind === "Conflict") conflicts++;
  }
  return { changes, conflicts };
}

export function counterLabel({ changes, conflicts }: Counter): string {
  if (changes === 0) return "All changes processed";
  const parts = [`${changes} ${changes === 1 ? "change" : "changes"}`];
  if (conflicts > 0)
    parts.push(`${conflicts} ${conflicts === 1 ? "conflict" : "conflicts"}`);
  return `${parts.join(" · ")} left`;
}
