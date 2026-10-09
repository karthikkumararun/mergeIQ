import { useEffect, useRef, useState } from "react";
import type { CommitSummary, SideLabel } from "../../ipc/bindings";
import { relativeTime } from "./relativeTime";
import styles from "./SideHeader.module.css";

interface Props {
  side: "left" | "right";
  label: SideLabel;
  commits: readonly CommitSummary[];
}

const ROLE_CHIP = { left: "Left", right: "Right" } as const;

/** Pane header with the contextual side label; clicking opens the commit popover. */
export function SideHeader({ side, label, commits }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const name = label.refName ?? label.role;

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={root} className={styles.root} data-header={side}>
      <button
        type="button"
        className={styles.button}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span className={styles.line}>
          <span className={styles.name} data-testid={`${side}-label`}>
            {name}
          </span>
          <span className={styles.chip}>
            {ROLE_CHIP[side]} · {label.gitTerm} · read-only
          </span>
        </span>
        <span className={styles.sub}>
          {label.shortSha ? (
            <span className={styles.sha}>{label.shortSha}</span>
          ) : null}{" "}
          {label.subject ?? label.role}
        </span>
      </button>
      {open ? (
        <CommitPopover name={name} commits={commits} side={side} />
      ) : null}
    </div>
  );
}

function CommitPopover({
  name,
  commits,
  side,
}: {
  name: string;
  commits: readonly CommitSummary[];
  side: "left" | "right";
}) {
  const [copied, setCopied] = useState<string | null>(null);
  async function copy(sha: string) {
    try {
      await navigator.clipboard.writeText(sha);
      setCopied(sha);
    } catch {
      setCopied(null);
    }
  }
  return (
    <div
      className={`${styles.popover} ${side === "right" ? styles.popoverRight : ""}`}
      role="dialog"
      aria-label={`Commits on ${name} touching this file`}
    >
      <div className={styles.popHead}>
        {name} · {commits.length}{" "}
        {commits.length === 1 ? "commit touches" : "commits touch"} this file ·
        newest first
      </div>
      <ul className={styles.list}>
        {commits.map((c) => (
          <li key={c.sha} className={styles.commit}>
            <span className={styles.commitSha}>{c.shortSha}</span>
            <span className={styles.commitSubject}>{c.subject}</span>
            <span className={styles.commitMeta}>
              {c.author} · {relativeTime(c.date)}
            </span>
            <button
              type="button"
              className={styles.copy}
              aria-label={`Copy SHA ${c.shortSha}`}
              onClick={() => void copy(c.sha)}
            >
              {copied === c.sha ? "✓" : "⧉"}
            </button>
          </li>
        ))}
        {commits.length === 0 ? (
          <li className={styles.empty}>No commits recorded for this file.</li>
        ) : null}
      </ul>
    </div>
  );
}
