import type { EditorState } from "@codemirror/state";
import type {
  Analysis,
  LineRange,
  SideText,
  Terminator,
} from "../../ipc/bindings";
import { isResolved, chunksOf } from "./session";
import { eolString, sideLines } from "./text";
import type {
  MergeLabels,
  ResultLine,
  SaveMode,
  SaveResult,
  UnresolvedConflict,
} from "./types";

function lastTerm(side: SideText): Terminator | null {
  const last = side.lines[side.lines.length - 1];
  return last ? last.term : null;
}

/** True when no side ends with a line terminator (so the saved file should not either). */
export function lacksFinalNewline(analysis: Analysis): boolean {
  return [analysis.base, analysis.ours, analysis.theirs].every((s) => {
    const t = lastTerm(s);
    return t === null || t === "None";
  });
}

function sideResultLines(side: SideText, range: LineRange): ResultLine[] {
  const lines = sideLines(side);
  const out: ResultLine[] = [];
  for (let i = range.start; i < range.end; i++) {
    out.push({
      text: lines[i],
      term:
        side.lines[i].term === "None" ? analysisEol(side) : side.lines[i].term,
    });
  }
  return out;
}

function analysisEol(side: SideText): Terminator {
  return side.lines.find((l) => l.term !== "None")?.term ?? "Lf";
}

/**
 * The Result document as engine lines. Terminators are normalized to the file's
 * dominant line ending (the pane edits `\n`-normalized text); the final line has none
 * when no side had a trailing newline or the user removed it.
 */
export function resultLines(
  state: EditorState,
  analysis: Analysis,
): ResultLine[] {
  const pieces = state.doc.toString().split("\n");
  const endsWithNewline = pieces[pieces.length - 1] === "";
  if (endsWithNewline) pieces.pop();
  const eol = analysis.dominant_eol === "None" ? "Lf" : analysis.dominant_eol;
  return pieces.map((text, i) => {
    const last = i === pieces.length - 1;
    const term: Terminator =
      last && (!endsWithNewline || lacksFinalNewline(analysis)) ? "None" : eol;
    return { text, term };
  });
}

const label = (l: MergeLabels["left"]) => l.refName ?? l.role;

/** Builds the payload handed to the host's `onSave`. */
export function buildSaveResult(
  state: EditorState,
  analysis: Analysis,
  labels: MergeLabels,
  mode: SaveMode,
): SaveResult {
  const unresolved = chunksOf(state).filter((c) => !isResolved(c));
  const base: SaveResult = {
    lines: resultLines(state, analysis),
    unresolvedIds: unresolved.map((c) => c.id),
    unresolved: [],
    mode,
    encoding: analysis.encoding,
  };
  if (mode !== "markers") return base;

  // Unresolved conflicts become marker blocks; their current Result lines are dropped.
  const conflicts = unresolved.filter((c) => c.kind === "Conflict");
  const all = resultLines(state, analysis);
  const lineOf = (pos: number) => state.doc.lineAt(pos).number - 1;
  const lines: ResultLine[] = [];
  const blocks: UnresolvedConflict[] = [];
  let cursor = 0;
  for (const c of conflicts) {
    const chunk = analysis.chunks.find((x) => x.id === c.id)!;
    const startLine = lineOf(c.from);
    const endLine = c.to > c.from ? lineOf(c.to) : startLine;
    lines.push(...all.slice(cursor, startLine));
    blocks.push({
      chunkId: c.id,
      at: lines.length,
      oursLabel: label(labels.left),
      oursLines: sideResultLines(analysis.ours, chunk.ours),
      theirsLabel: label(labels.right),
      theirsLines: sideResultLines(analysis.theirs, chunk.theirs),
    });
    cursor = endLine;
  }
  lines.push(...all.slice(cursor));
  return { ...base, lines, unresolved: blocks };
}

function pushLines(out: string[], lines: readonly ResultLine[]) {
  for (const l of lines)
    out.push(l.text, l.term === "None" ? "" : eolString(l.term));
}

/** Renders a `SaveResult` to text (mirrors `mergeiq_core::serialize`; markers are `\n`-terminated). */
export function renderSaveText(result: SaveResult): string {
  const out: string[] = [];
  let cursor = 0;
  const blocks =
    result.mode === "markers"
      ? [...result.unresolved].sort((a, b) => a.at - b.at)
      : [];
  for (const u of blocks) {
    pushLines(out, result.lines.slice(cursor, u.at));
    cursor = u.at;
    out.push(`<<<<<<< ${u.oursLabel}\n`);
    pushLines(out, u.oursLines);
    out.push("=======\n");
    pushLines(out, u.theirsLines);
    out.push(`>>>>>>> ${u.theirsLabel}\n`);
  }
  pushLines(out, result.lines.slice(cursor));
  return out.join("");
}
