import type { ReactNode } from "react";
import type { DiffRow } from "../structural/diff";
import styles from "./Ai.module.css";

/** `text` with backtick runs shown as `<code>`. */
export function WithCode({ text }: { text: string }): ReactNode {
  return text
    .split(/`([^`]+)`/)
    .map((part, i) => (i % 2 === 1 ? <code key={i}>{part}</code> : part));
}

/** One row of a diff, with the changed tokens emphasised. */
export function DiffRowView({ row }: { row: DiffRow }) {
  const [from, to] = row.emphasis ?? [0, 0];
  const body =
    row.emphasis && to > from ? (
      <>
        {row.text.slice(0, from)}
        <span className={styles.em}>{row.text.slice(from, to)}</span>
        {row.text.slice(to)}
      </>
    ) : (
      row.text
    );
  return (
    <div
      className={`${styles.row} ${row.kind === "del" ? styles.del : row.kind === "add" ? styles.add : ""}`}
    >
      <span className={styles.mark} aria-hidden="true">
        {row.kind === "del" ? "−" : row.kind === "add" ? "+" : ""}
      </span>
      <span>
        {row.kind === "del" ? (
          <span className={styles.sr}>Removed: </span>
        ) : null}
        {row.kind === "add" ? <span className={styles.sr}>Added: </span> : null}
        {body}
      </span>
    </div>
  );
}
