import type { EditorView } from "@codemirror/view";

export const GUTTER_WIDTH = 48;

export interface Span {
  top: number;
  bottom: number;
}

/** Vertical extent of `[from, to)` in `view`, relative to the top of its scroll viewport. */
export function rangeSpan(view: EditorView, from: number, to: number): Span {
  const offset = view.documentPadding.top - view.scrollDOM.scrollTop;
  const first = view.lineBlockAt(from);
  if (to <= from)
    return { top: first.top + offset, bottom: first.top + offset };
  const last = view.lineBlockAt(Math.max(from, to - 1));
  return { top: first.top + offset, bottom: last.bottom + offset };
}

/**
 * SVG path for the band joining a side-pane span to a Result-pane span across a
 * `GUTTER_WIDTH`-wide gutter, with cubic Bézier edges. `sideX`/`resultX` are the x
 * coordinates of the two edges (0 and `GUTTER_WIDTH` in either order).
 */
export function bandPath(
  side: Span,
  result: Span,
  sideX: number,
  resultX: number,
): { fill: string; top: string; bottom: string } {
  const mid = (sideX + resultX) / 2;
  const top = `M ${sideX} ${side.top} C ${mid} ${side.top} ${mid} ${result.top} ${resultX} ${result.top}`;
  const bottom = `M ${sideX} ${side.bottom} C ${mid} ${side.bottom} ${mid} ${result.bottom} ${resultX} ${result.bottom}`;
  return {
    fill: `${top} L ${resultX} ${result.bottom} C ${mid} ${result.bottom} ${mid} ${side.bottom} ${sideX} ${side.bottom} Z`,
    top,
    bottom,
  };
}
