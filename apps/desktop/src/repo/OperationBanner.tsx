import type { RepoStatus } from "./repoApi";
import { operationName, operationSentence } from "./describe";
import styles from "./OperationBanner.module.css";

interface Props {
  status: RepoStatus;
  busy: boolean;
  gitOutput: string;
  opError: string | null;
  onContinue: () => void;
  onAbort: () => void;
  onSkip: () => void;
}

/** The slim strip describing the operation in progress, with Continue / Abort / Skip. */
export function OperationBanner({
  status,
  busy,
  gitOutput,
  opError,
  onContinue,
  onAbort,
  onSkip,
}: Props) {
  const { operation, conflicts } = status;
  const remaining = conflicts.length;
  const output = opError ?? gitOutput;
  const controllable = !["None", "Unknown"].includes(operation.kind);
  const name = operationName(operation);

  if (operation.kind === "None" && remaining === 0) {
    return (
      <section aria-label="Repository state" className={styles.banner}>
        <span className={styles.sentence}>
          {status.branch ? (
            <>
              On branch <span className={styles.mono}>{status.branch}</span>
            </>
          ) : (
            "No branch checked out"
          )}
        </span>
        {output && (
          <details className={styles.output} open={opError !== null}>
            <summary>Git output</summary>
            <pre className={opError ? styles.errorText : undefined}>
              {output}
            </pre>
          </details>
        )}
      </section>
    );
  }

  return (
    <section aria-label="Operation in progress" className={styles.banner}>
      <span className={styles.sentence}>
        {operationSentence(status).map((part, i) =>
          part.mono ? (
            <span key={i} className={styles.mono}>
              {part.text}
            </span>
          ) : (
            <span key={i}>{part.text}</span>
          ),
        )}
        <span className={remaining === 0 ? styles.ok : styles.sub}>
          {remaining === 0
            ? " · All conflicts resolved"
            : ` · ${remaining} conflicted ${remaining === 1 ? "file" : "files"} · Continue unlocks when none remain`}
        </span>
      </span>
      {busy && (
        <span role="status" className={styles.sub}>
          Working… git hooks may take a while; this cannot be cancelled.
        </span>
      )}
      {output && (
        <details className={styles.output} open={opError !== null}>
          <summary>Git output</summary>
          <pre className={opError ? styles.errorText : undefined}>{output}</pre>
        </details>
      )}
      {controllable && (
        <div className={styles.actions}>
          {operation.kind === "Rebase" && (
            <button
              type="button"
              className={styles.btn}
              disabled={busy}
              onClick={onSkip}
            >
              Skip commit…
            </button>
          )}
          <button
            type="button"
            className={`${styles.btn} ${styles.danger}`}
            disabled={busy}
            onClick={onAbort}
          >
            Abort {name}…
          </button>
          <button
            type="button"
            className={`${styles.btn} ${styles.primary}`}
            disabled={busy || remaining > 0}
            onClick={onContinue}
          >
            Continue
          </button>
        </div>
      )}
    </section>
  );
}
