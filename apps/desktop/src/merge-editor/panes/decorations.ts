import {
  type EditorState,
  type Extension,
  type Range,
  StateEffect,
  StateField,
  Text,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  GutterMarker,
  WidgetType,
  gutter,
} from "@codemirror/view";
import type { Analysis } from "../../ipc/bindings";
import { chunkField } from "../model/session";
import { block, sideLines } from "../model/text";
import {
  type ChangeType,
  type PaneChunk,
  resultPaneChunks,
} from "./paneChunks";

export const setPaneChunks = StateEffect.define<PaneChunk[]>();
export const setCurrentChunk = StateEffect.define<number | null>();

const MARKS: Record<ChangeType, string> = {
  ins: "+",
  mod: "~",
  con: "!",
  del: "−",
  res: "✓",
};
const MARK_LABELS: Record<ChangeType, string> = {
  ins: "Inserted",
  mod: "Modified",
  con: "Conflict",
  del: "Deleted",
  res: "Resolved",
};

class MarkerWidget extends WidgetType {
  constructor(readonly type: ChangeType) {
    super();
  }
  eq(other: MarkerWidget) {
    return other.type === this.type;
  }
  toDOM() {
    const el = document.createElement("div");
    el.className = `cm-merge-marker cm-merge-marker-${this.type}`;
    el.setAttribute("aria-hidden", "true");
    return el;
  }
}

class ChunkMark extends GutterMarker {
  constructor(readonly type: ChangeType) {
    super();
  }
  eq(other: ChunkMark) {
    return other.type === this.type;
  }
  toDOM() {
    const el = document.createElement("span");
    el.className = `cm-merge-mark-${this.type}`;
    el.textContent = MARKS[this.type];
    el.setAttribute("role", "img");
    el.setAttribute("aria-label", MARK_LABELS[this.type]);
    return el;
  }
}

/** Reserves the gutter width without announcing anything to assistive tech. */
class SpacerMark extends GutterMarker {
  toDOM() {
    const el = document.createElement("span");
    el.textContent = "!";
    el.setAttribute("aria-hidden", "true");
    return el;
  }
}
const spacer = new SpacerMark();

const markCache = new Map<ChangeType, ChunkMark>();
function markFor(type: ChangeType) {
  let m = markCache.get(type);
  if (!m) markCache.set(type, (m = new ChunkMark(type)));
  return m;
}

function buildDecorations(
  chunks: readonly PaneChunk[],
  doc: Text,
): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  for (const c of chunks) {
    if (c.from > doc.length || c.to > doc.length) continue;
    if (c.from === c.to) {
      ranges.push(
        Decoration.widget({
          widget: new MarkerWidget(c.type),
          block: true,
          side: -1,
        }).range(c.from),
      );
      continue;
    }
    const first = doc.lineAt(c.from).number;
    const last = doc.lineAt(Math.max(c.from, c.to - 1)).number;
    for (let n = first; n <= last; n++) {
      const cls = [`cm-merge-${c.type}`];
      if (c.current) cls.push("cm-merge-current");
      ranges.push(
        Decoration.line({ class: cls.join(" ") }).range(doc.line(n).from),
      );
    }
    const emClass = `cm-merge-em-${c.type === "res" ? "del" : c.type}`;
    for (const e of c.emphasis) {
      if (e.to > e.from && e.to <= doc.length)
        ranges.push(Decoration.mark({ class: emClass }).range(e.from, e.to));
    }
  }
  return Decoration.set(ranges, true);
}

function gutterExtension(
  read: (state: EditorState) => readonly PaneChunk[],
): Extension {
  const byFirstLine = new WeakMap<
    readonly PaneChunk[],
    Map<number, PaneChunk>
  >();
  const lookup = (chunks: readonly PaneChunk[]) => {
    let m = byFirstLine.get(chunks);
    if (!m) {
      m = new Map(chunks.filter((c) => c.to > c.from).map((c) => [c.from, c]));
      byFirstLine.set(chunks, m);
    }
    return m;
  };
  return gutter({
    class: "cm-merge-marks",
    lineMarker(view, line) {
      const c = lookup(read(view.state)).get(line.from);
      return c ? markFor(c.type) : null;
    },
    lineMarkerChange: (update) =>
      update.docChanged ||
      update.transactions.some((tr) => tr.effects.length > 0),
    initialSpacer: () => spacer,
  });
}

/** Extensions for a read-only side/base pane whose chunk descriptors are pushed in. */
export function sidePaneDecorations(opts: { marks?: boolean } = {}): Extension {
  const field = StateField.define<PaneChunk[]>({
    create: () => [],
    update(value, tr) {
      for (const e of tr.effects) if (e.is(setPaneChunks)) return e.value;
      return value;
    },
  });
  const decos = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(value, tr) {
      for (const e of tr.effects)
        if (e.is(setPaneChunks)) return buildDecorations(e.value, tr.state.doc);
      return value;
    },
    provide: (f) => EditorView.decorations.from(f),
  });
  return opts.marks === false
    ? [field, decos]
    : [field, decos, gutterExtension((s) => s.field(field))];
}

/** Extensions for the Result pane: descriptors are derived from the chunk session. */
export function resultPaneDecorations(analysis: Analysis): Extension {
  const baseLines = sideLines(analysis.base);
  const baseBlock = (c: { base: { start: number; end: number } }) =>
    block(baseLines, c.base);
  const currentField = StateField.define<number | null>({
    create: () => null,
    update(value, tr) {
      for (const e of tr.effects) if (e.is(setCurrentChunk)) return e.value;
      return value;
    },
  });
  const field = StateField.define<PaneChunk[]>({
    create: (state) =>
      resultPaneChunks(
        analysis,
        state.field(chunkField),
        (a, b) => state.doc.sliceString(a, b),
        baseBlock,
        null,
      ),
    update(value, tr) {
      const chunksChanged =
        tr.state.field(chunkField) !== tr.startState.field(chunkField);
      const currentChanged = tr.effects.some((e) => e.is(setCurrentChunk));
      if (!tr.docChanged && !chunksChanged && !currentChanged) return value;
      return resultPaneChunks(
        analysis,
        tr.state.field(chunkField),
        (a, b) => tr.state.doc.sliceString(a, b),
        baseBlock,
        tr.state.field(currentField),
      );
    },
  });
  const decos = StateField.define<DecorationSet>({
    create: (state) => buildDecorations(state.field(field), state.doc),
    update(value, tr) {
      const next = tr.state.field(field);
      return next === tr.startState.field(field)
        ? value
        : buildDecorations(next, tr.state.doc);
    },
    provide: (f) => EditorView.decorations.from(f),
  });
  return [currentField, field, decos, gutterExtension((s) => s.field(field))];
}
