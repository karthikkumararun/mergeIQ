import { useEffect, useRef, type ReactNode } from "react";
import styles from "./Dialog.module.css";

interface Props {
  titleId: string;
  onClose: () => void;
  children: ReactNode;
}

/** Modal dialog: traps Tab, closes on Escape, returns focus on close. */
export function Dialog({ titleId, onClose, children }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const first =
      el?.querySelector<HTMLElement>("[data-autofocus]") ??
      el?.querySelector<HTMLElement>("button");
    first?.focus();
    return () => previous?.focus?.();
  }, []);

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== "Tab") return;
    const focusable = ref.current?.querySelectorAll<HTMLElement>(
      "button:not([disabled]), [href], input, select, textarea",
    );
    if (!focusable || focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  return (
    <div className={styles.backdrop}>
      <div
        ref={ref}
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        {children}
      </div>
    </div>
  );
}
