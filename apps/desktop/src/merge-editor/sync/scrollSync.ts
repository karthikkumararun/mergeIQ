import type { EditorView } from "@codemirror/view";
import { mapLine, type PaneId } from "./anchors";

type Anchors = Partial<Record<PaneId, number[]>>;

/** Fractional line index at the top of `view`'s viewport. */
export function topLine(view: EditorView): number {
  const y = view.scrollDOM.scrollTop - view.documentPadding.top;
  const block = view.lineBlockAtHeight(Math.max(0, y));
  const line = view.state.doc.lineAt(block.from).number - 1;
  const frac =
    block.height > 0
      ? Math.min(1, Math.max(0, (y - block.top) / block.height))
      : 0;
  return line + frac;
}

/** Scroll position that puts (fractional) `line` at the top of `view`. */
export function scrollTopForLine(view: EditorView, line: number): number {
  const doc = view.state.doc;
  const clamped = Math.max(0, Math.min(line, doc.lines));
  const idx = Math.min(Math.floor(clamped), doc.lines - 1);
  const frac = clamped - idx;
  const block = view.lineBlockAt(doc.line(idx + 1).from);
  return Math.max(
    0,
    block.top + frac * block.height + view.documentPadding.top,
  );
}

/**
 * Keeps panes aligned: scrolling one pane scrolls the others to the corresponding
 * lines (1:1 in unchanged regions, interpolated across chunks).
 */
export class ScrollSync {
  private views = new Map<PaneId, EditorView>();
  private listeners = new Map<PaneId, () => void>();
  private expected = new Map<PaneId, number>();
  enabled = true;

  constructor(private anchors: () => Anchors) {}

  attach(id: PaneId, view: EditorView) {
    this.detach(id);
    this.views.set(id, view);
    const onScroll = () => {
      const exp = this.expected.get(id);
      if (exp !== undefined) {
        this.expected.delete(id);
        if (Math.abs(view.scrollDOM.scrollTop - exp) <= 1) return;
      }
      if (this.enabled) this.alignFrom(id);
    };
    view.scrollDOM.addEventListener("scroll", onScroll, { passive: true });
    this.listeners.set(id, () =>
      view.scrollDOM.removeEventListener("scroll", onScroll),
    );
  }

  detach(id: PaneId) {
    this.listeners.get(id)?.();
    this.listeners.delete(id);
    this.views.delete(id);
    this.expected.delete(id);
  }

  destroy() {
    [...this.views.keys()].forEach((id) => this.detach(id));
  }

  /** Scrolls every other pane to match `source`'s current top line. */
  alignFrom(source: PaneId) {
    const src = this.views.get(source);
    const anchors = this.anchors();
    const from = anchors[source];
    if (!src || !from) return;
    const line = topLine(src);
    for (const [id, view] of this.views) {
      const to = anchors[id];
      if (id === source || !to) continue;
      const max = view.scrollDOM.scrollHeight - view.scrollDOM.clientHeight;
      const target = Math.min(
        Math.max(0, max),
        Math.round(scrollTopForLine(view, mapLine(line, from, to))),
      );
      if (Math.abs(view.scrollDOM.scrollTop - target) < 1) continue;
      this.expected.set(id, target);
      view.scrollDOM.scrollTop = target;
    }
  }
}
