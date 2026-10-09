import type { EditorState } from "@codemirror/state";
import type { Analysis, ContextInput, SideLabel } from "../../../ipc/bindings";
import { isResolved, chunksOf } from "../../model/session";
import type { MergeDocument } from "../../model/types";

/** The contextual name of a side (its branch or ref, else its role). */
export function sideName(label: SideLabel): string {
  return label.refName ?? label.role;
}

/** Result-document line range `[start, end)` of a character range (whole lines). */
export function lineSpan(
  state: EditorState,
  from: number,
  to: number,
): { start: number; end: number } {
  const start = state.doc.lineAt(from).number - 1;
  if (to <= from) return { start, end: start };
  // `to` includes the last line break, so the last covered line is the one holding to - 1.
  return { start, end: state.doc.lineAt(to - 1).number };
}

/**
 * Everything the backend needs to describe one conflict. Commits are not sent: the backend
 * reads them from the repository itself.
 */
export function buildInput(
  doc: MergeDocument,
  analysis: Analysis,
  state: EditorState,
  chunkId: number,
): ContextInput | null {
  const chunk = analysis.chunks.find((c) => c.id === chunkId);
  const session = chunksOf(state).find((c) => c.id === chunkId);
  if (!chunk || !session) return null;
  return {
    path: doc.displayPath,
    base: analysis.base.text,
    left: {
      label: sideName(doc.labels.left),
      text: analysis.ours.text,
      commits: [],
    },
    right: {
      label: sideName(doc.labels.right),
      text: analysis.theirs.text,
      commits: [],
    },
    result: state.doc.toString(),
    chunk: {
      base: { start: chunk.base.start, end: chunk.base.end },
      left: { start: chunk.ours.start, end: chunk.ours.end },
      right: { start: chunk.theirs.start, end: chunk.theirs.end },
      result: lineSpan(state, session.from, session.to),
    },
  };
}

/** Unresolved conflicts, in document order. */
export function unresolvedConflicts(state: EditorState): number[] {
  return chunksOf(state)
    .filter((c) => c.kind === "Conflict" && !isResolved(c))
    .sort((a, b) => a.from - b.from)
    .map((c) => c.id);
}
