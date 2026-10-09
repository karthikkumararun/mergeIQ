import { invertedEffects } from "@codemirror/commands";
import {
  EditorState,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import type { Analysis } from "../../ipc/bindings";
import { changeTouches, mapChunks } from "./mapping";
import { documentText, sideLines } from "./text";
import type { ChunkState, ChunkStatus } from "./types";

/** A change to one chunk's session state, optionally pinning its Result range. */
export interface ChunkPatch {
  id: number;
  status?: Partial<ChunkStatus>;
  range?: { from: number; to: number };
}

export const patchChunks = StateEffect.define<ChunkPatch[]>();

export function isResolved(c: ChunkState): boolean {
  if (
    c.resolution === "edited" ||
    c.resolution === "auto" ||
    c.resolution === "whole-file"
  ) {
    return true;
  }
  return c.leftStatus !== "pending" && c.rightStatus !== "pending";
}

/** Initial side statuses for a chunk kind. */
export function pendingStatus(kind: ChunkState["kind"]): ChunkStatus {
  switch (kind) {
    case "OursOnly":
      return { leftStatus: "pending", rightStatus: "na", resolution: "none" };
    case "TheirsOnly":
      return { leftStatus: "na", rightStatus: "pending", resolution: "none" };
    default:
      return {
        leftStatus: "pending",
        rightStatus: "pending",
        resolution: "none",
      };
  }
}

/** Chunk states for an unmodified Result (= base text). */
export function initialChunks(analysis: Analysis): ChunkState[] {
  const lines = sideLines(analysis.base);
  const offsets = lineOffsets(lines);
  return analysis.chunks.map((c) => ({
    id: c.id,
    kind: c.kind,
    ...pendingStatus(c.kind),
    from: offsets[c.base.start],
    to: offsets[c.base.end],
  }));
}

/** Start offset of each line in a `\n`-terminated document, plus the end offset. */
export function lineOffsets(lines: string[]): number[] {
  const out: number[] = [];
  let pos = 0;
  for (const l of lines) {
    out.push(pos);
    pos += l.length + 1;
  }
  out.push(pos);
  return out;
}

function applyPatches(
  chunks: ChunkState[],
  patches: readonly ChunkPatch[],
): ChunkState[] {
  if (patches.length === 0) return chunks;
  const byId = new Map(patches.map((p) => [p.id, p]));
  return chunks.map((c) => {
    const p = byId.get(c.id);
    if (!p) return c;
    return { ...c, ...p.status, ...p.range };
  });
}

const USER_EDIT_EVENTS = ["input", "delete"];

export const chunkField = StateField.define<ChunkState[]>({
  create: () => [],
  update(chunks, tr) {
    let next = chunks;
    if (tr.docChanged) next = mapChunks(next, tr.changes);
    const patches = tr.effects.flatMap((e) =>
      e.is(patchChunks) ? e.value : [],
    );
    if (patches.length > 0) {
      next = applyPatches(next, patches);
    } else if (
      tr.docChanged &&
      USER_EDIT_EVENTS.some((ev) => tr.isUserEvent(ev))
    ) {
      // Typing inside (or touching) an unresolved chunk resolves it as `edited`.
      // Compare against the pre-change ranges, which `chunks` still holds.
      next = next.map((c, i) => {
        const before = chunks[i];
        if (isResolved(c) || !changeTouches(tr.changes, before.from, before.to))
          return c;
        return { ...c, resolution: "edited" };
      });
    }
    return next;
  },
});

function statusOf(c: ChunkState): ChunkStatus {
  return {
    leftStatus: c.leftStatus,
    rightStatus: c.rightStatus,
    resolution: c.resolution,
  };
}

function sameStatus(a: ChunkStatus, b: ChunkStatus): boolean {
  return (
    a.leftStatus === b.leftStatus &&
    a.rightStatus === b.rightStatus &&
    a.resolution === b.resolution
  );
}

/**
 * Makes chunk statuses (and explicitly pinned ranges) undoable together with the text
 * they belong to: the inverse of a transaction restores the previous statuses.
 */
const chunkHistory = invertedEffects.of((tr) => {
  const before = tr.startState.field(chunkField);
  const after = tr.state.field(chunkField);
  const pinned = new Set(
    tr.effects.flatMap((e) =>
      e.is(patchChunks) ? e.value.filter((p) => p.range).map((p) => p.id) : [],
    ),
  );
  const inverse: ChunkPatch[] = [];
  after.forEach((c, i) => {
    const old = before[i];
    const statusChanged = !sameStatus(statusOf(old), statusOf(c));
    if (!statusChanged && !pinned.has(c.id)) return;
    inverse.push({
      id: c.id,
      status: statusOf(old),
      range: pinned.has(c.id) ? { from: old.from, to: old.to } : undefined,
    });
  });
  return inverse.length > 0 ? [patchChunks.of(inverse)] : [];
});

export function chunkSession(analysis: Analysis): Extension {
  return [chunkField.init(() => initialChunks(analysis)), chunkHistory];
}

export function resultDocText(analysis: Analysis): string {
  return documentText(sideLines(analysis.base));
}

export function chunksOf(state: EditorState): ChunkState[] {
  return state.field(chunkField);
}
