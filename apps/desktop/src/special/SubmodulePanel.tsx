import { useState } from "react";
import type { ConflictLoad, SubmoduleCommit } from "../ipc/bindings";
import type { RepoApi } from "../repo/repoApi";
import type { ResolutionMethod } from "../repo/repoStore";
import { baseName, leftName, rightName, shortDate, shortOid } from "./format";
import { useAction, useAsync } from "./hooks";
import { PanelShell } from "./PanelShell";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  onBack: () => void;
  onResolved: (method: ResolutionMethod) => Promise<void>;
}

const Arrow = () => (
  <svg
    className={styles.arrow}
    width="40"
    height="16"
    viewBox="0 0 40 16"
    aria-hidden="true"
  >
    <path d="M2 8h32" stroke="currentColor" strokeWidth="2" />
    <path d="m30 3 6 5-6 5" fill="none" stroke="currentColor" strokeWidth="2" />
  </svg>
);

function Node({
  eyebrow,
  commit,
  newest,
}: {
  eyebrow: React.ReactNode;
  commit: SubmoduleCommit | null;
  newest?: boolean;
}) {
  return (
    <div className={`${styles.node} ${newest ? styles.nodeNewest : ""}`}>
      <span className={shell.eyebrow}>{eyebrow}</span>
      {commit ? (
        <>
          <span className={styles.nodeSha}>{shortOid(commit.sha)}</span>
          {commit.subject && (
            <span className={styles.nodeSubject}>{commit.subject}</span>
          )}
          {commit.date && (
            <span className={styles.nodeDate}>{shortDate(commit.date)}</span>
          )}
        </>
      ) : (
        <span className={styles.nodeDate}>No commit on this side</span>
      )}
    </div>
  );
}

const check = (
  <svg
    width="18"
    height="18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="m5 12 5 5 9-10" />
  </svg>
);
const info = (
  <svg
    width="18"
    height="18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v6" />
    <path d="M12 7.5v.5" />
  </svg>
);

/** A submodule pointer both sides moved: pick a commit; the descendant is recommended. */
export function SubmodulePanel({ load, api, onBack, onResolved }: Props) {
  const { entry, labels } = load;
  const details = useAsync(() => api.submoduleDetails(entry.path), entry.path);
  const { busy, error, run } = useAction();
  const [hover, setHover] = useState<"Ours" | "Theirs" | null>(null);
  const left = leftName(labels);
  const right = rightName(labels);
  const name = baseName(entry.display);

  const d = details.state === "ready" ? details.value : null;
  const relation = d?.relation ?? "Unknown";
  const recommended =
    relation === "LeftAncestorOfRight"
      ? "Theirs"
      : relation === "RightAncestorOfLeft"
        ? "Ours"
        : null;
  const leftSha = d?.left?.sha;
  const rightSha = d?.right?.sha;
  const commandSide = hover ?? recommended ?? "Theirs";
  const commandSha = commandSide === "Ours" ? leftSha : rightSha;

  const pick = (side: "Ours" | "Theirs") =>
    run(async () => {
      await api.useSide(entry.path, side);
      await onResolved(side === "Ours" ? "Accepted Left" : "Accepted Right");
    });

  const statusLine = () => {
    if (!d) return null;
    const l = <span className={shell.mono}>{shortOid(leftSha)}</span>;
    const r = <span className={shell.mono}>{shortOid(rightSha)}</span>;
    switch (relation) {
      case "LeftAncestorOfRight":
        return (
          <div role="status" className={shell.status}>
            {check}
            <span>
              {l} (left) is an ancestor of {r} (right). Using Right keeps both
              sides’ changes.
            </span>
          </div>
        );
      case "RightAncestorOfLeft":
        return (
          <div role="status" className={shell.status}>
            {check}
            <span>
              {r} (right) is an ancestor of {l} (left). Using Left keeps both
              sides’ changes.
            </span>
          </div>
        );
      case "Diverged":
        return (
          <div role="status" className={`${shell.status} ${shell.statusWarn}`}>
            {info}
            <span>
              The two commits have diverged: neither contains the other.
              Choosing one drops the other side’s commits from the pointer.
            </span>
          </div>
        );
      default:
        return (
          <div
            role="status"
            className={`${shell.status} ${shell.statusNeutral}`}
          >
            {info}
            <span>
              {d.checkedOut
                ? "Ancestry unknown (a commit is missing from the submodule)"
                : "Ancestry unknown (submodule not checked out)"}
            </span>
          </div>
        );
    }
  };

  const optionText = (side: "Ours" | "Theirs") => {
    if (relation === "LeftAncestorOfRight")
      return side === "Ours"
        ? `Drops ${d?.right?.subject ? `the “${d.right.subject}” commit` : "the right side’s commits"} from the pointer.`
        : "Descendant of both. Same as a fast-forward.";
    if (relation === "RightAncestorOfLeft")
      return side === "Theirs"
        ? `Drops ${d?.left?.subject ? `the “${d.left.subject}” commit` : "the left side’s commits"} from the pointer.`
        : "Descendant of both. Same as a fast-forward.";
    return side === "Ours"
      ? `Points at ${left}’s commit.`
      : `Points at ${right}’s commit.`;
  };

  const option = (side: "Ours" | "Theirs") => {
    const sha = side === "Ours" ? leftSha : rightSha;
    const isRec = recommended === side;
    return (
      <div
        className={`${shell.optionCard} ${isRec ? styles.recommended : ""}`}
        onMouseEnter={() => setHover(side)}
        onMouseLeave={() => setHover(null)}
      >
        <div className={styles.rowInline}>
          <button
            type="button"
            className={`${shell.btn} ${isRec ? shell.primary : ""}`}
            disabled={busy || !sha}
            onFocus={() => setHover(side)}
            onBlur={() => setHover(null)}
            onClick={() => void pick(side)}
          >
            {side === "Ours" ? "Use Left" : "Use Right"}
            {sha ? ` · ${shortOid(sha)}` : ""}
          </button>
          {isRec && <span className={styles.recommendedTag}>Recommended</span>}
        </div>
        <p>{optionText(side)}</p>
      </div>
    );
  };

  return (
    <PanelShell
      label="Submodule conflict"
      display={entry.display}
      badge="Submodule"
      tone="purple"
      subtitle="Both moved the pointer"
      title={<>Which {name} commit should this repository point to?</>}
      lead="Choosing updates the index entry only. The submodule’s own checkout is not changed."
      onBack={onBack}
    >
      {details.state === "loading" && <p className={shell.loading}>Loading…</p>}
      {details.state === "error" && (
        <p role="alert" className={shell.alert}>
          {details.message}
        </p>
      )}
      {d && (
        <>
          <section aria-label="Commit ancestry" className={styles.ancestry}>
            <Node eyebrow="Base" commit={d.base} />
            <Arrow />
            <Node
              eyebrow={
                <>
                  Left · <span className={shell.mono}>{left}</span>
                </>
              }
              commit={d.left}
              newest={relation === "RightAncestorOfLeft"}
            />
            <Arrow />
            <Node
              eyebrow={
                <>
                  Right · <span className={shell.mono}>{right}</span>
                </>
              }
              commit={d.right}
              newest={relation === "LeftAncestorOfRight"}
            />
          </section>
          {statusLine()}
          {error && (
            <p role="alert" className={shell.alert}>
              {error}
            </p>
          )}
          <div className={shell.optionCards}>
            {option("Ours")}
            {option("Theirs")}
          </div>
          {commandSha && (
            <p className={styles.command}>
              Runs: git update-index --cacheinfo 160000,{shortOid(commandSha)}…,
              {entry.display}
            </p>
          )}
        </>
      )}
    </PanelShell>
  );
}
