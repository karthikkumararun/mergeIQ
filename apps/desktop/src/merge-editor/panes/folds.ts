import {
  codeFolding,
  foldEffect,
  unfoldEffect,
  foldedRanges,
} from "@codemirror/language";
import type {
  EditorState,
  Extension,
  StateEffect,
  Text,
} from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

export const FOLD_MIN_LINES = 8;
export const FOLD_CONTEXT = 3;

/** Extension rendering folded regions as an aligned "⋯ N unchanged lines" row. */
export const foldPlaceholder: Extension = codeFolding({
  preparePlaceholder: (state, range) =>
    state.doc.lineAt(range.to).number - state.doc.lineAt(range.from).number + 1,
  placeholderDOM(_view, onclick, prepared) {
    const el = document.createElement("span");
    el.className = "cm-foldPlaceholder";
    el.textContent = `⋯ ${prepared as number} unchanged lines`;
    el.setAttribute("role", "button");
    el.setAttribute("tabindex", "0");
    el.setAttribute(
      "aria-label",
      `Expand ${prepared as number} unchanged lines`,
    );
    el.onclick = onclick;
    el.onkeydown = (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onclick(e);
      }
    };
    return el;
  },
});

/**
 * Document ranges to fold for the unchanged regions in `anchors`
 * (`[0, c.start, c.end, …, total]`): regions longer than `FOLD_MIN_LINES` lines keep
 * `FOLD_CONTEXT` lines at each end; the rest collapses into a single row.
 */
export function foldRanges(
  doc: Text,
  anchors: readonly number[],
): { from: number; to: number }[] {
  const out: { from: number; to: number }[] = [];
  for (let i = 0; i + 1 < anchors.length; i += 2) {
    const start = anchors[i];
    const end = Math.min(anchors[i + 1], doc.lines);
    if (end - start <= FOLD_MIN_LINES) continue;
    const hideFrom = start + FOLD_CONTEXT;
    const hideTo = end - FOLD_CONTEXT;
    if (hideTo - hideFrom < 1) continue;
    out.push({ from: doc.line(hideFrom + 1).from, to: doc.line(hideTo).to });
  }
  return out;
}

export function foldEffects(
  state: EditorState,
  anchors: readonly number[],
): StateEffect<unknown>[] {
  return foldRanges(state.doc, anchors).map((r) => foldEffect.of(r));
}

export function unfoldAllEffects(view: EditorView): StateEffect<unknown>[] {
  const effects: StateEffect<unknown>[] = [];
  foldedRanges(view.state).between(0, view.state.doc.length, (from, to) => {
    effects.push(unfoldEffect.of({ from, to }));
  });
  return effects;
}
