import { Dialog } from "../merge-editor/dialogs/Dialog";
import styles from "../merge-editor/dialogs/Dialog.module.css";
import local from "./dialogs.module.css";

const LIST_LIMIT = 10;

interface ConfirmProps {
  title: string;
  message: string;
  /** Files the action applies to, listed in the dialog. */
  files?: string[];
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Cancel (focused) / confirm, optionally listing the affected files. */
export function RepoConfirm({
  title,
  message,
  files,
  confirmLabel,
  danger,
  onConfirm,
  onCancel,
}: ConfirmProps) {
  return (
    <Dialog titleId="repo-confirm-title" onClose={onCancel}>
      <div>
        <h2 id="repo-confirm-title" className={styles.title}>
          {title}
        </h2>
        <p className={styles.body}>{message}</p>
        {files && files.length > 0 && (
          <ul className={local.files} aria-label="Affected files">
            {files.slice(0, LIST_LIMIT).map((f) => (
              <li key={f}>{f}</li>
            ))}
            {files.length > LIST_LIMIT && (
              <li className={local.more}>
                and {files.length - LIST_LIMIT} more
              </li>
            )}
          </ul>
        )}
      </div>
      <div className={styles.row}>
        <button
          type="button"
          data-autofocus
          className={styles.btn}
          onClick={onCancel}
        >
          Cancel
        </button>
        <button
          type="button"
          className={`${styles.btn} ${danger ? local.danger : styles.primary}`}
          onClick={onConfirm}
        >
          {confirmLabel}
        </button>
      </div>
    </Dialog>
  );
}

interface UnsavedProps {
  files: string[];
  /** What the user was about to do: "close this window", "abort the rebase"… */
  action: string;
  onSave: () => void;
  onDiscard: () => void;
  onCancel: () => void;
}

/** Save / Discard / Cancel before an action that would lose unsaved editor changes. */
export function UnsavedDialog({
  files,
  action,
  onSave,
  onDiscard,
  onCancel,
}: UnsavedProps) {
  return (
    <Dialog titleId="repo-unsaved-title" onClose={onCancel}>
      <div>
        <h2 id="repo-unsaved-title" className={styles.title}>
          Unsaved changes
        </h2>
        <p className={styles.body}>
          {files.length === 1
            ? `${files[0]} has unsaved changes.`
            : `${files.length} files have unsaved changes.`}{" "}
          Save them before you {action}?
        </p>
        {files.length > 1 && (
          <ul className={local.files} aria-label="Files with unsaved changes">
            {files.map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        )}
      </div>
      <div className={styles.row}>
        <button
          type="button"
          data-autofocus
          className={styles.btn}
          onClick={onCancel}
        >
          Cancel
        </button>
        <button type="button" className={styles.btn} onClick={onDiscard}>
          Discard changes
        </button>
        <button
          type="button"
          className={`${styles.btn} ${styles.primary}`}
          onClick={onSave}
        >
          Save…
        </button>
      </div>
    </Dialog>
  );
}
