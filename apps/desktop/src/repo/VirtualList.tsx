import { useEffect, useRef, useState, type ReactNode } from "react";

interface Props<T> {
  items: T[];
  rowHeight: number;
  /** Rendered height when the container cannot be measured (tests). */
  fallbackHeight?: number;
  overscan?: number;
  label: string;
  className?: string;
  getKey: (item: T) => string;
  render: (item: T, index: number) => ReactNode;
}

/** Fixed-row-height windowing: only rows near the viewport are in the DOM. */
export function VirtualList<T>({
  items,
  rowHeight,
  fallbackHeight = 600,
  overscan = 8,
  label,
  className,
  getKey,
  render,
}: Props<T>) {
  const ref = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(0);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setHeight(el.clientHeight);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const viewport = height || fallbackHeight;
  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const last = Math.min(
    items.length,
    Math.ceil((scrollTop + viewport) / rowHeight) + overscan,
  );
  return (
    <div
      ref={ref}
      role="list"
      aria-label={label}
      className={className}
      style={{ overflowY: "auto", position: "relative" }}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
    >
      <div style={{ height: items.length * rowHeight, position: "relative" }}>
        {items.slice(first, last).map((item, i) => (
          <div
            key={getKey(item)}
            role="listitem"
            style={{
              position: "absolute",
              top: (first + i) * rowHeight,
              height: rowHeight,
              left: 0,
              right: 0,
            }}
          >
            {render(item, first + i)}
          </div>
        ))}
      </div>
    </div>
  );
}
