import { useState } from "react";
import { STANDALONE_SCOPE, type RepoDecision } from "../../ai/api";
import { globProblem } from "../../ai/defaults";
import styles from "./AiSettings.module.css";

interface Props {
  globs: string[];
  repos: Record<string, RepoDecision>;
  noticeAccepted: boolean;
  onGlobs: (globs: string[]) => void;
  onDecision: (scope: string, decision: RepoDecision | null) => void;
  onNotice: (accepted: boolean) => void;
}

function repoLabel(scope: string): { name: string; path: string | null } {
  if (scope === STANDALONE_SCOPE)
    return { name: "Standalone merges", path: null };
  const name = scope.split(/[\\/]/).filter(Boolean).pop() ?? scope;
  return { name, path: scope };
}

/** Exclusion globs, per-repository decisions and the data-sharing notice. */
export function PrivacyForm({
  globs,
  repos,
  noticeAccepted,
  onGlobs,
  onDecision,
  onNotice,
}: Props) {
  const [draft, setDraft] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  const add = () => {
    const value = draft.trim();
    if (!value) return;
    const bad = globProblem(value);
    if (bad) {
      setProblem(bad);
      return;
    }
    if (!globs.includes(value)) onGlobs([...globs, value]);
    setDraft("");
    setProblem(null);
  };

  const entries = Object.entries(repos).sort(([a], [b]) => a.localeCompare(b));

  return (
    <section className={styles.section} aria-labelledby="ai-privacy-title">
      <h2 id="ai-privacy-title" className={styles.h2}>
        Privacy
      </h2>

      <div className={styles.fld}>
        <span id="ai-globs-label">Never send files matching</span>
        <div
          className={styles.chips}
          role="group"
          aria-labelledby="ai-globs-label"
        >
          {globs.map((g) => (
            <span key={g} className={styles.chip}>
              {g}
              <button
                type="button"
                aria-label={`Remove ${g}`}
                onClick={() => onGlobs(globs.filter((x) => x !== g))}
              >
                ×
              </button>
            </span>
          ))}
          <input
            className={styles.addGlob}
            type="text"
            placeholder="Add glob"
            aria-label="Add exclusion glob"
            spellCheck={false}
            value={draft}
            onChange={(e) => {
              setDraft(e.target.value);
              setProblem(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
            onBlur={add}
          />
        </div>
        {problem ? (
          <p role="alert" className={styles.error}>
            {problem}
          </p>
        ) : null}
      </div>

      <div className={styles.fld}>
        <span>Repositories</span>
        <div className={styles.repos}>
          {entries.length === 0 ? (
            <div className={styles.empty}>
              No repositories yet. Each one asks the first time you use an AI
              action in it.
            </div>
          ) : (
            entries.map(([scope, decision]) => {
              const { name, path } = repoLabel(scope);
              const allowed = decision === "Allowed";
              return (
                <div key={scope} className={styles.repo}>
                  <span className={styles.repoName} title={path ?? undefined}>
                    {name}
                    {path ? (
                      <span className={styles.repoPath}>{path}</span>
                    ) : null}
                  </span>
                  <span className={allowed ? styles.ok : styles.where}>
                    {allowed ? "AI allowed" : "Declined"}
                  </span>
                  <button
                    type="button"
                    className={`${styles.btn} ${styles.small}`}
                    aria-label={`${allowed ? "Disable" : "Enable"} AI for ${name}`}
                    onClick={() =>
                      onDecision(scope, allowed ? "Declined" : "Allowed")
                    }
                  >
                    {allowed ? "Disable" : "Enable"}
                  </button>
                </div>
              );
            })
          )}
        </div>
        <p className={styles.hint}>
          Each repository asks once, the first time you use an AI action in it.
        </p>
      </div>

      <div className={styles.row}>
        <span className={styles.text} data-testid="ai-notice-state">
          Data-sharing notice:{" "}
          {noticeAccepted
            ? "accepted"
            : "not accepted yet (shown on first use)"}
        </span>
        {noticeAccepted ? (
          <button
            type="button"
            className={`${styles.btn} ${styles.small}`}
            onClick={() => onNotice(false)}
          >
            Show it again
          </button>
        ) : null}
      </div>
    </section>
  );
}
