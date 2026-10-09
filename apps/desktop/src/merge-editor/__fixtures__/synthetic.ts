import type { Analysis, Chunk, ChunkKind, Line, SideText } from "../../ipc/bindings";

const KINDS: ChunkKind[] = ["OursOnly", "TheirsOnly", "Conflict", "BothSame"];

function sideText(lines: string[]): SideText {
  let pos = 0;
  const table: Line[] = lines.map((l) => {
    const line: Line = { start: pos, end: pos + l.length, term: "Lf" };
    pos += l.length + 1;
    return line;
  });
  return { text: lines.map((l) => `${l}\n`).join(""), lines: table };
}

/**
 * Builds an engine-shaped `Analysis` for large-file tests: `total` base lines with a
 * chunk every `every` lines (kinds cycle ours-only, theirs-only, conflict, both-same).
 * With `shift`, ours-only chunks insert extra lines and theirs-only chunks delete their
 * line, so line numbers drift between panes. Fine diffs are omitted.
 */
export function syntheticAnalysis(total: number, every: number, shift = false): Analysis {
  const base: string[] = [];
  const ours: string[] = [];
  const theirs: string[] = [];
  const chunks: Chunk[] = [];
  let i = 0;
  let k = 0;
  while (i < total) {
    if (i === k * every + 5 && i < total) {
      const kind = KINDS[k % KINDS.length];
      const b = { start: base.length, end: base.length + 1 };
      base.push(`row ${i}`);
      const o = { start: ours.length, end: ours.length };
      const t = { start: theirs.length, end: theirs.length };
      const oursChanged = kind !== "TheirsOnly";
      const theirsChanged = kind !== "OursOnly";
      if (oursChanged) {
        ours.push(`row ${i} ours`);
        if (shift && kind === "OursOnly") ours.push(`row ${i} ours extra 1`, `row ${i} ours extra 2`, `row ${i} ours extra 3`);
      } else ours.push(`row ${i}`);
      o.end = ours.length;
      if (theirsChanged) {
        if (!(shift && kind === "TheirsOnly")) theirs.push(kind === "BothSame" ? `row ${i} ours` : `row ${i} theirs`);
      } else theirs.push(`row ${i}`);
      t.end = theirs.length;
      chunks.push({
        id: k,
        kind,
        base: b,
        ours: o,
        theirs: t,
        fine: null,
        fine_diff_skipped: false,
        simple: kind === "Conflict" ? "Unresolvable" : null,
      });
      k++;
    } else {
      base.push(`row ${i}`);
      ours.push(`row ${i}`);
      theirs.push(`row ${i}`);
    }
    i++;
  }
  return {
    base: sideText(base),
    ours: sideText(ours),
    theirs: sideText(theirs),
    chunks,
    encoding: { encoding: "Utf8", bom: false },
    dominant_eol: "Lf",
  };
}
