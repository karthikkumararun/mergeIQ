import { useEffect, useRef } from "react";
import { failureMessage } from "../../../ai/api";
import { tokens } from "../../../ai/format";
import styles from "./Ai.module.css";
import type { PreviewState } from "./store";

/** "Preview request": the exact text that would be sent. */
export function PreviewDialog({
  preview,
  onClose,
}: {
  preview: PreviewState;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, []);
  const data = preview.data;
  return (
    <div className={styles.modalBackdrop}>
      <div
        ref={ref}
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-labelledby="ai-preview-title"
        tabIndex={-1}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onClose();
          }
        }}
      >
        <div className={styles.modalHead}>
          <h2 id="ai-preview-title" className={styles.modalTitle}>
            Preview request
          </h2>
          {data ? (
            <p className={styles.modalNotes}>
              Sent to {data.destination} · about {tokens(data.estimatedTokens)}{" "}
              input tokens
              {data.notes.length > 0 ? ` · ${data.notes.join(" · ")}` : ""}
              {data.overBudget ? " · still over the token budget" : ""}
            </p>
          ) : null}
        </div>
        {preview.phase === "loading" ? (
          <p className={styles.payload}>Building the request…</p>
        ) : preview.phase === "error" && preview.failure ? (
          <p className={styles.payload} role="alert">
            {failureMessage(preview.failure)}
          </p>
        ) : (
          <pre className={styles.payload} data-testid="ai-payload">
            {data?.text}
          </pre>
        )}
        <div className={styles.modalFoot}>
          <button type="button" className={styles.btn} onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
