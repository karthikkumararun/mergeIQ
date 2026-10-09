import { Dialog } from "./Dialog";
import styles from "./Dialog.module.css";

interface Props {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Two-button confirmation; Cancel is focused by default so Enter never destroys work. */
export function ConfirmDialog({
  title,
  message,
  confirmLabel,
  cancelLabel = "Keep editing",
  onConfirm,
  onCancel,
}: Props) {
  return (
    <Dialog titleId="confirm-dialog-title" onClose={onCancel}>
      <div>
        <h2 id="confirm-dialog-title" className={styles.title}>
          {title}
        </h2>
        <p className={styles.body}>{message}</p>
      </div>
      <div className={styles.row}>
        <button
          type="button"
          data-autofocus
          className={styles.btn}
          onClick={onCancel}
        >
          {cancelLabel}
        </button>
        <button
          type="button"
          className={`${styles.btn} ${styles.primary}`}
          data-confirm
          onClick={onConfirm}
        >
          {confirmLabel}
        </button>
      </div>
    </Dialog>
  );
}
