import {
  StateEffect,
  StateField,
  type Extension,
  type EditorState,
} from "@codemirror/state";
import {
  EditorView,
  gutter,
  GutterMarker,
  ViewPlugin,
  type PluginValue,
} from "@codemirror/view";
import { chunksOf } from "../../model/session";

/** A chunk that has a structural proposal; `proposal` indexes the store's proposals. */
export interface Mark {
  chunkId: number;
  proposal: number;
  open: boolean;
}

export const setMarks = StateEffect.define<Mark[]>();

const marksField = StateField.define<Mark[]>({
  create: () => [],
  update(marks, tr) {
    for (const e of tr.effects) if (e.is(setMarks)) return e.value;
    return marks;
  },
});

class Indicator extends GutterMarker {
  constructor(
    readonly proposal: number,
    readonly open: boolean,
    readonly onOpen: (proposal: number) => void,
  ) {
    super();
  }

  eq(other: GutterMarker): boolean {
    return (
      other instanceof Indicator &&
      other.proposal === this.proposal &&
      other.open === this.open
    );
  }

  toDOM(): HTMLElement {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "cm-structural-indicator";
    button.textContent = "S";
    button.setAttribute("aria-label", "Show structural proposal");
    button.setAttribute("aria-haspopup", "dialog");
    button.setAttribute("aria-expanded", String(this.open));
    // Keep the editor selection and focus where they are.
    button.addEventListener("mousedown", (e) => e.preventDefault());
    button.addEventListener("click", () => this.onOpen(this.proposal));
    return button;
  }
}

const theme = EditorView.baseTheme({
  ".cm-structural-gutter .cm-gutterElement": {
    padding: "0 2px",
    boxSizing: "border-box",
  },
  ".cm-structural-indicator": {
    width: "20px",
    height: "20px",
    marginTop: "1px",
    padding: "0",
    borderRadius: "4px",
    border: "1px solid color-mix(in srgb, var(--mod-fg) 55%, transparent)",
    background: "var(--mod-bg)",
    color: "var(--mod-fg)",
    font: '600 10px var(--font-code, "JetBrains Mono", monospace)',
    cursor: "pointer",
  },
  ".cm-structural-indicator:hover": { background: "var(--mod-em)" },
  ".cm-structural-indicator:focus-visible": {
    outline: "2px solid var(--accent)",
    outlineOffset: "1px",
  },
  ".cm-structural-indicator[aria-expanded=true]": {
    background: "var(--mod-em)",
    borderColor: "var(--mod-fg)",
  },
});

/**
 * CodeMirror marks its whole gutter container `aria-hidden`, which would hide our focusable
 * buttons from assistive technology. Expose the container and hide only the line numbers.
 */
export const exposeGutter = ViewPlugin.fromClass(
  class implements PluginValue {
    constructor(view: EditorView) {
      this.fix(view);
      // The gutter container is created after plugins are constructed on first render.
      requestAnimationFrame(() => this.fix(view));
    }

    update(update: { view: EditorView }) {
      this.fix(update.view);
    }

    fix(view: EditorView) {
      const gutters = view.dom.querySelector(".cm-gutters");
      if (gutters?.getAttribute("aria-hidden") === "true") {
        gutters.removeAttribute("aria-hidden");
        view.dom
          .querySelector(".cm-lineNumbers")
          ?.setAttribute("aria-hidden", "true");
      }
    }
  },
);

function markFor(state: EditorState, from: number): Mark | undefined {
  const marks = state.field(marksField);
  if (marks.length === 0) return undefined;
  const chunks = chunksOf(state);
  return marks.find((m) =>
    chunks.some((c) => c.id === m.chunkId && c.from === from),
  );
}

/** An "S" button in the Result gutter on the first line of every chunk with a proposal. */
export function structuralGutter(
  onOpen: (proposal: number) => void,
): Extension {
  return [
    marksField,
    gutter({
      class: "cm-structural-gutter",
      lineMarker(view, line) {
        const mark = markFor(view.state, line.from);
        return mark ? new Indicator(mark.proposal, mark.open, onOpen) : null;
      },
      lineMarkerChange: (u) =>
        u.docChanged ||
        u.transactions.some((t) => t.effects.some((e) => e.is(setMarks))),
      initialSpacer: () => new Indicator(0, false, onOpen),
    }),
    exposeGutter,
    theme,
  ];
}
