import { useRef } from "react";
import styles from "./ResizeHandle.module.css";

interface Props {
  edge: "left" | "right";
  label: string;
  /** Share of the pane row (percent) the pane on the left of the handle occupies. */
  value: number;
  /** Called with the horizontal drag distance in pixels since the last call. */
  onResize: (dx: number) => void;
}

/** Draggable (and arrow-key operable) splitter on a gutter edge. */
export function ResizeHandle({ edge, label, value, onResize }: Props) {
  const last = useRef<number | null>(null);
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={Math.round(value)}
      aria-valuemin={0}
      aria-valuemax={100}
      tabIndex={0}
      className={`${styles.handle} ${edge === "left" ? styles.left : styles.right}`}
      onPointerDown={(e) => {
        last.current = e.clientX;
        e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (last.current === null) return;
        const dx = e.clientX - last.current;
        last.current = e.clientX;
        if (dx !== 0) onResize(dx);
      }}
      onPointerUp={(e) => {
        last.current = null;
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") {
          e.preventDefault();
          onResize(-24);
        } else if (e.key === "ArrowRight") {
          e.preventDefault();
          onResize(24);
        }
      }}
    />
  );
}
