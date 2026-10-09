import type { SaveMode } from "../model/types";
import { Dialog } from "./Dialog";
import styles from "./Dialog.module.css";

interface Props {
  fileName: string;
  conflicts: number;
  changes: number;
  /** 1-based Result line of the first unresolved conflict, if any. */
  firstConflictLine: number | null;
  onChoose: (choice: "continue" | SaveMode) => void;
}

const plural = (n: number, one: string, many: string) =>
  `${n} ${n === 1 ? one : many}`;

/** Shown by Apply while chunks are unresolved. */
export function SaveDialog({
  fileName,
  conflicts,
  changes,
  firstConflictLine,
  onChoose,
}: Props) {
  const others = changes - conflicts;
  const parts = [
    conflicts > 0 ? plural(conflicts, "conflict", "conflicts") : null,
    others > 0 ? plural(others, "change", "changes") : null,
  ].filter(Boolean);
  const title = `${parts.join(" and ")} ${conflicts + others === 1 ? "is" : "are"} unresolved`;
  return (
    <Dialog titleId="save-dialog-title" onClose={() => onChoose("continue")}>
      <div>
        <h2 id="save-dialog-title" className={styles.title}>
          {title}
        </h2>
        <p className={styles.body}>
          <span className={styles.mono}>{fileName}</span>
          {conflicts > 0 && firstConflictLine !== null ? (
            <> still has a conflict at line {firstConflictLine}.</>
          ) : (
            <> still has unresolved changes.</>
          )}
        </p>
      </div>
      <div className={styles.options}>
        <button
          type="button"
          data-autofocus
          data-choice="continue"
          className={`${styles.opt} ${styles.optDefault}`}
          onClick={() => onChoose("continue")}
        >
          <span className={styles.optText}>
            <span className={styles.optTitle}>Continue resolving</span>
            <span className={styles.optHint}>
              Go back to the next unresolved chunk.
            </span>
          </span>
          <span className={styles.kbd}>↵</span>
        </button>
        <button
          type="button"
          data-choice="markers"
          className={styles.opt}
          onClick={() => onChoose("markers")}
        >
          <span className={styles.optText}>
            <span className={styles.optTitle}>Save with conflict markers</span>
            <span className={styles.optHint}>
              Writes{" "}
              <span className={styles.mono}>&lt;&lt;&lt;&lt;&lt;&lt;&lt;</span>{" "}
              markers around the unresolved region. Not staged.
            </span>
          </span>
        </button>
        <button
          type="button"
          data-choice="force"
          className={styles.opt}
          onClick={() => onChoose("force")}
        >
          <span className={styles.optText}>
            <span className={styles.optTitle}>Mark as resolved anyway</span>
            <span className={styles.optHint}>
              Saves the result exactly as it is now and stages the file.
            </span>
          </span>
        </button>
      </div>
    </Dialog>
  );
}
