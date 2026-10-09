import {
  RangeSetBuilder,
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  gutter,
  GutterMarker,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { chunksOf } from "../../model/session";
import { exposeGutter } from "../structural/gutter";

/** Chunks that offer AI actions. */
export const setAiMarks = StateEffect.define<number[]>();
/** The chunk the panel is showing (outlined in the Result). */
export const setAiActive = StateEffect.define<number | null>();

const marksField = StateField.define<number[]>({
  create: () => [],
  update(marks, tr) {
    for (const e of tr.effects) if (e.is(setAiMarks)) return e.value;
    return marks;
  },
});

const activeField = StateField.define<number | null>({
  create: () => null,
  update(active, tr) {
    for (const e of tr.effects) if (e.is(setAiActive)) return e.value;
    return active;
  },
});

class Sparkle extends GutterMarker {
  constructor(
    readonly chunkId: number,
    readonly active: boolean,
    readonly onOpen: (chunkId: number) => void,
  ) {
    super();
  }

  eq(other: GutterMarker): boolean {
    return (
      other instanceof Sparkle &&
      other.chunkId === this.chunkId &&
      other.active === this.active
    );
  }

  toDOM(): HTMLElement {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "cm-ai-indicator";
    button.setAttribute("aria-label", "AI assistant for this conflict");
    button.setAttribute("aria-expanded", String(this.active));
    button.innerHTML =
      '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z"/></svg>';
    button.addEventListener("mousedown", (e) => e.preventDefault());
    button.addEventListener("click", () => this.onOpen(this.chunkId));
    return button;
  }
}

function markFor(state: EditorState, from: number): number | undefined {
  const marks = state.field(marksField);
  if (marks.length === 0) return undefined;
  const chunks = chunksOf(state);
  return marks.find((id) => chunks.some((c) => c.id === id && c.from === from));
}

/** Outlines the active chunk's lines. */
const outline = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = this.build(view.state);
    }

    update(u: ViewUpdate) {
      this.decorations = this.build(u.state);
    }

    build(state: EditorState): DecorationSet {
      const id = state.field(activeField);
      const chunk =
        id === null ? null : chunksOf(state).find((c) => c.id === id);
      const builder = new RangeSetBuilder<Decoration>();
      if (!chunk) return builder.finish();
      // Insertion points have no lines; outline the line they sit on.
      const first = state.doc.lineAt(Math.min(chunk.from, state.doc.length));
      const last =
        chunk.to > chunk.from
          ? state.doc.lineAt(Math.max(chunk.from, chunk.to - 1))
          : first;
      for (let n = first.number; n <= last.number; n++) {
        const line = state.doc.line(n);
        const classes = ["cm-ai-active"];
        if (n === first.number) classes.push("cm-ai-active-first");
        if (n === last.number) classes.push("cm-ai-active-last");
        builder.add(
          line.from,
          line.from,
          Decoration.line({ class: classes.join(" ") }),
        );
      }
      return builder.finish();
    }
  },
  { decorations: (v) => v.decorations },
);

const accent = "var(--ai-accent)";
const theme = EditorView.baseTheme({
  ".cm-ai-gutter .cm-gutterElement": {
    padding: "0 2px",
    boxSizing: "border-box",
  },
  ".cm-ai-indicator": {
    width: "20px",
    height: "20px",
    marginTop: "1px",
    padding: "0",
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: "4px",
    border: "1px solid var(--ai-border)",
    background: "var(--ai-bg)",
    color: "var(--ai-fg)",
    cursor: "pointer",
  },
  ".cm-ai-indicator:hover": {
    background: "color-mix(in srgb, var(--ai-accent) 25%, var(--ai-bg))",
  },
  ".cm-ai-indicator:focus-visible": {
    outline: "2px solid var(--accent)",
    outlineOffset: "1px",
  },
  ".cm-ai-indicator[aria-expanded=true]": {
    borderColor: accent,
    background: "color-mix(in srgb, var(--ai-accent) 30%, var(--ai-bg))",
  },
  ".cm-ai-active": {
    boxShadow: `inset 1px 0 0 ${accent}, inset -1px 0 0 ${accent}`,
  },
  ".cm-ai-active-first": {
    boxShadow: `inset 1px 0 0 ${accent}, inset -1px 0 0 ${accent}, inset 0 1px 0 ${accent}`,
  },
  ".cm-ai-active-last": {
    boxShadow: `inset 1px 0 0 ${accent}, inset -1px 0 0 ${accent}, inset 0 -1px 0 ${accent}`,
  },
  ".cm-ai-active-first.cm-ai-active-last": {
    boxShadow: `inset 1px 0 0 ${accent}, inset -1px 0 0 ${accent}, inset 0 1px 0 ${accent}, inset 0 -1px 0 ${accent}`,
  },
});

/** A ✦ button in the Result gutter on the first line of each chunk that offers AI actions. */
export function aiGutter(onOpen: (chunkId: number) => void): Extension {
  return [
    marksField,
    activeField,
    gutter({
      class: "cm-ai-gutter",
      lineMarker(view, line) {
        const id = markFor(view.state, line.from);
        return id === undefined
          ? null
          : new Sparkle(id, view.state.field(activeField) === id, onOpen);
      },
      lineMarkerChange: (u) =>
        u.docChanged ||
        u.transactions.some((t) =>
          t.effects.some((e) => e.is(setAiMarks) || e.is(setAiActive)),
        ),
      initialSpacer: () => new Sparkle(0, false, onOpen),
    }),
    exposeGutter,
    outline,
    theme,
  ];
}
