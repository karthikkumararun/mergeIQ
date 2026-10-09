import type { ConflictLoad, StageMeta } from "../ipc/bindings";
import { typeLabel } from "../repo/describe";
import type { RepoApi } from "../repo/repoApi";
import type { ResolutionMethod } from "../repo/repoStore";
import { abbreviateOid, formatBytes, leftName, rightName } from "./format";
import { useAction, useAsync } from "./hooks";
import { PanelShell, type Tone } from "./PanelShell";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  onBack: () => void;
  onResolved: (method: ResolutionMethod) => Promise<void>;
}

type Kind = "symlink" | "lfs" | "oversized";

interface Spec {
  kind: Kind;
  badge: string;
  tone: Tone;
  label: string;
  title: string;
  note: string;
  value: (meta: StageMeta | undefined, stage: number) => string;
}

const isWindows = () =>
  typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

function specFor(kind: Kind): Spec {
  switch (kind) {
    case "symlink":
      return {
        kind,
        badge: "Symlink",
        tone: "neutral",
        label: "Symlink conflict",
        title: "Both sides changed this link",
        note: isWindows()
          ? "The link is recreated and staged with mode 120000. Without symlink permission (core.symlinks), the target is written as the file’s content instead."
          : "The link is recreated and staged with mode 120000.",
        value: (m, stage) =>
          !m
            ? stage === 1
              ? "not present"
              : "deleted"
            : m.mode === "120000"
              ? `→ ${m.symlinkTarget ?? ""}`
              : "regular file",
      };
    case "lfs":
      return {
        kind,
        badge: "Git LFS pointer",
        tone: "info",
        label: "Git LFS conflict",
        title: "Both sides changed this large file",
        note: "Stages that side’s pointer file. LFS fetches the content on checkout.",
        value: (m, stage) =>
          !m
            ? stage === 1
              ? "not present"
              : "deleted"
            : m.lfs
              ? `oid ${abbreviateOid(m.lfs.oid)} · ${formatBytes(m.lfs.size)}`
              : "regular file",
      };
    case "oversized":
      return {
        kind,
        badge: "Too large to open · over 20 MB",
        tone: "warn",
        label: "Oversized file conflict",
        title: "This file is too large to open here",
        note: "MergeIQ won’t load this file into the editor.",
        value: (m, stage) =>
          !m ? (stage === 1 ? "not present" : "deleted") : formatBytes(m.size),
      };
  }
}

/** Side-by-side rows plus Use Left / Use Right for conflicts that cannot be merged by line. */
function PickSidePanel({
  load,
  api,
  onBack,
  onResolved,
  kind,
}: Props & { kind: Kind }) {
  const spec = specFor(kind);
  const { entry, labels } = load;
  const details = useAsync(() => api.details(entry.path), entry.path);
  const { busy, error, run } = useAction();
  const stages = details.state === "ready" ? details.value.stages : [];
  const stage = (n: number) => stages.find((s) => s.stage === n);
  const left = leftName(labels);
  const right = rightName(labels);

  const pick = (side: "Ours" | "Theirs") =>
    run(async () => {
      await api.useSide(entry.path, side);
      await onResolved(side === "Ours" ? "Accepted Left" : "Accepted Right");
    });

  return (
    <PanelShell
      label={spec.label}
      display={entry.display}
      badge={spec.badge}
      tone={spec.tone}
      subtitle={typeLabel(entry.conflictType)}
      title={spec.title}
      lead={
        kind === "oversized"
          ? "Pick the version to keep or open the working-tree file in another application."
          : "These can’t be merged line by line. Pick the version to keep."
      }
      maxWidth={760}
      onBack={onBack}
    >
      <section
        className={`${shell.card} ${styles.pickCard}`}
        aria-label={entry.display}
      >
        <div
          className={shell.cardHead}
          style={{ justifyContent: "flex-start", gap: 12 }}
        >
          <span className={styles.tag}>{spec.badge}</span>
          <h2 className={shell.path} style={{ margin: 0 }}>
            {entry.display}
          </h2>
        </div>
        {details.state === "loading" && (
          <p className={shell.loading}>Loading…</p>
        )}
        {details.state === "error" && (
          <p role="alert" className={shell.alert} style={{ margin: 16 }}>
            {details.message}
          </p>
        )}
        {details.state === "ready" && (
          <dl className={styles.rows}>
            <dt>Base</dt>
            <dd>{spec.value(stage(1), 1)}</dd>
            <dt>Left · {left}</dt>
            <dd>{spec.value(stage(2), 2)}</dd>
            <dt>Right · {right}</dt>
            <dd>{spec.value(stage(3), 3)}</dd>
          </dl>
        )}
        <p className={styles.cardNote}>{spec.note}</p>
        {error && (
          <p
            role="alert"
            className={shell.alert}
            style={{ margin: "0 16px 14px" }}
          >
            {error}
          </p>
        )}
        <div className={styles.cardActions}>
          <button
            type="button"
            className={shell.btn}
            disabled={busy}
            onClick={() => void pick("Ours")}
          >
            Use Left
          </button>
          <button
            type="button"
            className={shell.btn}
            disabled={busy}
            onClick={() => void pick("Theirs")}
          >
            Use Right
          </button>
          {kind === "oversized" && (
            <button
              type="button"
              className={shell.btn}
              onClick={() => void run(() => api.openWorkingFile(entry.path))}
            >
              Open in default app
            </button>
          )}
        </div>
      </section>
    </PanelShell>
  );
}

export const SymlinkPanel = (p: Props) => (
  <PickSidePanel {...p} kind="symlink" />
);
export const LfsPanel = (p: Props) => <PickSidePanel {...p} kind="lfs" />;
export const OversizedPanel = (p: Props) => (
  <PickSidePanel {...p} kind="oversized" />
);
