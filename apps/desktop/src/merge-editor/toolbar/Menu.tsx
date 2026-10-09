import { useEffect, useRef, useState, type ReactNode } from "react";
import styles from "./Toolbar.module.css";

export interface MenuItem {
  id: string;
  label: string;
  hint?: string;
  disabled?: boolean;
  onSelect: () => void;
}

interface Props {
  label: ReactNode;
  ariaLabel?: string;
  items: MenuItem[];
  disabled?: boolean;
  buttonClass?: string;
}

/** Button that opens a small menu of actions (arrow keys, Enter, Escape). */
export function Menu({
  label,
  ariaLabel,
  items,
  disabled,
  buttonClass,
}: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    list.current
      ?.querySelector<HTMLElement>("[role=menuitem]:not([disabled])")
      ?.focus();
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      setOpen(false);
      root.current?.querySelector<HTMLElement>("button")?.focus();
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const els = [
      ...(list.current?.querySelectorAll<HTMLElement>(
        "[role=menuitem]:not([disabled])",
      ) ?? []),
    ];
    const i = els.indexOf(document.activeElement as HTMLElement);
    const next =
      e.key === "ArrowDown"
        ? (i + 1) % els.length
        : (i - 1 + els.length) % els.length;
    els[next]?.focus();
  }

  return (
    <div ref={root} className={styles.menuRoot} onKeyDown={onKeyDown}>
      <button
        type="button"
        className={buttonClass ?? styles.btn}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {label}
      </button>
      {open ? (
        <div ref={list} className={styles.menu} role="menu">
          {items.map((it) => (
            <button
              key={it.id}
              type="button"
              role="menuitem"
              className={styles.menuItem}
              disabled={it.disabled}
              onClick={() => {
                setOpen(false);
                it.onSelect();
              }}
            >
              <span>{it.label}</span>
              {it.hint ? (
                <span className={styles.menuHint}>{it.hint}</span>
              ) : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
