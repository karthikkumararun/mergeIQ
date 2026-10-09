import { failureMessage, type AiFailure, type AiStatus } from "../../../ai/api";
import { PROVIDER_NAME } from "../../../ai/format";
import styles from "./Ai.module.css";
import type { AiHost, AiStore } from "./store";

/** A failure shown in the panel (`--danger-*` box with the message). */
export function ErrorBox({
  failure,
  children,
}: {
  failure: AiFailure;
  children?: React.ReactNode;
}) {
  const message = failureMessage(failure);
  return (
    <div role="alert" className={styles.errorBox}>
      <span className={styles.errorTitle}>
        {failure.code === "refused"
          ? "The model declined this request"
          : failure.code === "truncated"
            ? "The response was cut off"
            : failure.code === "excluded"
              ? "File excluded from AI by your settings"
              : "The request failed"}
      </span>
      {failure.code === "refused" ||
      failure.code === "truncated" ||
      failure.code === "excluded" ? (
        <span>
          {failure.code === "refused"
            ? failure.category
              ? `Category: ${failure.category}. No suggestion was produced.`
              : "No suggestion was produced."
            : failure.code === "truncated"
              ? "The reply reached the output limit before it finished. Try again, or resolve this conflict by hand."
              : "Change the exclusion list in Settings › AI if this file should be sent."}
        </span>
      ) : (
        <span>{message}</span>
      )}
      {children}
    </div>
  );
}

interface CardProps {
  failure: AiFailure;
  status: AiStatus | null;
  host: AiHost;
  store: AiStore;
}

/** What to do before an AI action can run. */
export function GateCard({ failure, status, host, store }: CardProps) {
  const provider = status ? PROVIDER_NAME[status.provider] : "the provider";
  const where =
    status?.local === true
      ? "your local model server"
      : status
        ? status.provider === "anthropic"
          ? "Anthropic (api.anthropic.com)"
          : provider
        : "the AI provider";
  const s = store.getState();

  switch (failure.code) {
    case "notConfigured":
      return (
        <div className={styles.card} role="region" aria-label="Set up AI">
          <h3 className={styles.cardTitle}>Set up AI</h3>
          <p className={styles.cardText}>
            Choose a provider and add a key to get explanations and suggested
            resolutions. A local Ollama model needs no key.
          </p>
          <div className={styles.cardActions}>
            <button
              type="button"
              className={`${styles.btn} ${styles.primary}`}
              onClick={() => void host.api.openSettings("ai")}
            >
              Open AI settings
            </button>
          </div>
        </div>
      );
    case "noticeRequired":
      return (
        <div
          className={styles.card}
          role="region"
          aria-label="Data-sharing notice"
        >
          <h3 className={styles.cardTitle}>
            {status?.local
              ? "Requests stay on this computer"
              : "Your code leaves this computer"}
          </h3>
          <p className={styles.cardText}>
            To explain or resolve a conflict, MergeIQ sends the following to{" "}
            {where}:
          </p>
          <ul className={styles.cardList}>
            <li>the conflicting lines from base, left and right</li>
            <li>the surrounding lines, and the whole file when it fits</li>
            <li>commit messages that touched the file</li>
          </ul>
          <p className={styles.cardText}>
            Files matching your exclusion list are never sent, and you can
            preview every request first.
          </p>
          <div className={styles.cardActions}>
            <button
              type="button"
              className={`${styles.btn} ${styles.primary}`}
              onClick={() => void s.acceptNotice()}
            >
              Accept and continue
            </button>
            <button type="button" className={styles.btn} onClick={s.close}>
              Not now
            </button>
          </div>
        </div>
      );
    case "repoUnasked":
      return (
        <div
          className={styles.card}
          role="region"
          aria-label="Allow AI in this repository"
        >
          <h3 className={styles.cardTitle}>Use AI in {host.scopeName}?</h3>
          <p className={styles.cardText}>
            Code from {host.scopeName} will be sent to {where} when you ask for
            an explanation or a suggestion. You are asked once per repository
            and can change it later in Settings › AI.
          </p>
          <div className={styles.cardActions}>
            <button
              type="button"
              className={`${styles.btn} ${styles.primary}`}
              onClick={() => void s.decideRepo("Allowed")}
            >
              Allow AI here
            </button>
            <button
              type="button"
              className={styles.btn}
              onClick={() => void s.decideRepo("Declined")}
            >
              Don't allow
            </button>
          </div>
        </div>
      );
    case "repoDeclined":
      return (
        <div
          className={styles.card}
          role="region"
          aria-label="AI is turned off here"
        >
          <h3 className={styles.cardTitle}>
            AI is turned off for {host.scopeName}
          </h3>
          <p className={styles.cardText}>
            Nothing has been sent. You can turn it back on in Settings › AI.
          </p>
          <div className={styles.cardActions}>
            <button
              type="button"
              className={styles.btn}
              onClick={() => void host.api.openSettings("ai")}
            >
              Open AI settings
            </button>
          </div>
        </div>
      );
    case "excluded":
      return (
        <ErrorBox failure={failure}>
          <button
            type="button"
            className={styles.link}
            onClick={() => void host.api.openSettings("ai")}
          >
            Open AI settings
          </button>
        </ErrorBox>
      );
    default:
      return <ErrorBox failure={failure} />;
  }
}
