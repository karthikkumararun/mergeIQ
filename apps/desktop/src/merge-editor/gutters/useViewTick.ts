import type { EditorView } from "@codemirror/view";
import { useEffect, useState } from "react";

/**
 * Returns a counter that increments (at most once per animation frame) whenever any of
 * `views` scrolls or changes size, so overlays can recompute their geometry.
 */
export function useViewTick(views: readonly (EditorView | null)[]): number {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let frame = 0;
    const bump = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        setTick((t) => t + 1);
      });
    };
    const cleanups: (() => void)[] = [];
    for (const view of views) {
      if (!view) continue;
      view.scrollDOM.addEventListener("scroll", bump, { passive: true });
      const ro =
        typeof ResizeObserver === "undefined" ? null : new ResizeObserver(bump);
      ro?.observe(view.scrollDOM);
      ro?.observe(view.contentDOM);
      cleanups.push(() => {
        view.scrollDOM.removeEventListener("scroll", bump);
        ro?.disconnect();
      });
    }
    bump();
    return () => {
      cleanups.forEach((c) => c());
      if (frame) cancelAnimationFrame(frame);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, views);
  return tick;
}
