import type { ConflictLoad } from "../ipc/bindings";
import { sideChanges, typeLabel } from "./describe";
import type { AcceptSide } from "./repoApi";
import styles from "./EditorTabs.module.css";

interface Props {
  load: ConflictLoad;
  onAccept: (side: AcceptSide) => void;
  onDelete: () => void;
}

/**
 * Minimal panel for conflicts the text editor cannot show (deleted on one side, added on
 * both, binary, symlink, submodule). `special-conflicts` replaces it with richer panels.
 */
export function NonTextPanel({ load, onAccept, onDelete }: Props) {
  const { entry, labels } = load;
  const changes = sideChanges(entry.conflictType);
  const left = labels.ours.refName ?? labels.ours.role;
  const right = labels.theirs.refName ?? labels.theirs.role;
  const reason = load.analysisError;
  return (
    <section className={styles.nonText} aria-label="Non-text conflict">
      <h2 className={styles.nonTextTitle}>{entry.display}</h2>
      <p className={styles.nonTextKind}>{typeLabel(entry.conflictType)}</p>
      <dl className={styles.sides}>
        <dt>Left ({left})</dt>
        <dd>{changes.left}</dd>
        <dt>Right ({right})</dt>
        <dd>{changes.right}</dd>
      </dl>
      {reason && entry.conflictType === "BothModified" && (
        <p className={styles.nonTextNote}>
          This file can’t be merged as text: {reason}.
        </p>
      )}
      <div className={styles.nonTextActions}>
        <button type="button" onClick={() => onAccept("Ours")}>
          Accept Left
        </button>
        <button type="button" onClick={() => onAccept("Theirs")}>
          Accept Right
        </button>
        <button
          type="button"
          className={styles.dangerButton}
          onClick={onDelete}
        >
          Delete file
        </button>
      </div>
    </section>
  );
}
