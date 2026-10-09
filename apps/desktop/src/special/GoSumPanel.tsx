import { useMemo } from "react";
import type { ConflictLoad, GoSumMark } from "../ipc/bindings";
import type { RepoApi } from "../repo/repoApi";
import type { ResolutionMethod } from "../repo/repoStore";
import { leftName, rightName } from "./format";
import { useAction, useAsync } from "./hooks";
import { PanelShell } from "./PanelShell";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  onBack: () => void;
  onResolved: (method: ResolutionMethod) => Promise<void>;
  /** Opens the merge editor instead. */
  onMergeByHand: () => void;
}

const TAGS: Record<GoSumMark, { tag: string; cls: string; sr: string }> = {
  Unchanged: { tag: "", cls: "", sr: "" },
  LeftAdded: { tag: "L+", cls: styles.sumL, sr: "Added by left: " },
  RightAdded: { tag: "R+", cls: styles.sumR, sr: "Added by right: " },
  Removed: { tag: "−", cls: styles.sumX, sr: "Removed: " },
};

/** go.sum: auto-merge as the sorted union, with a preview of what was added and removed. */
export function GoSumPanel({
  load,
  api,
  onBack,
  onResolved,
  onMergeByHand,
}: Props) {
  const { entry, labels } = load;
  const preview = useAsync(() => api.goSumPreview(entry.path), entry.path);
  const { busy, error, run } = useAction();
  const left = leftName(labels);
  const right = rightName(labels);

  const counts = useMemo(() => {
    const c = { LeftAdded: 0, RightAdded: 0, Removed: 0, Unchanged: 0 };
    if (preview.state === "ready")
      for (const l of preview.value.lines) c[l.mark]++;
    return c;
  }, [preview]);

  return (
    <PanelShell
      label="go.sum conflict"
      display={entry.display}
      badge="go.sum"
      tone="ok"
      onBack={onBack}
    >
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: 16,
          alignItems: "flex-end",
          justifyContent: "space-between",
        }}
      >
        <div className={shell.intro} style={{ maxWidth: 680 }}>
          <h1 className={shell.title}>Auto-merge checksums</h1>
          <p className={shell.lead}>
            Keeps every checksum either side added, drops ones a side removed
            when the other didn’t touch them, and sorts the result.
          </p>
        </div>
        <div className={shell.actions}>
          <button type="button" className={shell.btn} onClick={onMergeByHand}>
            Merge by hand
          </button>
          <button
            type="button"
            className={`${shell.btn} ${shell.primary}`}
            disabled={busy || preview.state !== "ready"}
            onClick={() =>
              void run(async () => {
                await api.goSumUnion(entry.path);
                await onResolved("Auto-merged");
              })
            }
          >
            Auto-merge (union) and stage
          </button>
        </div>
      </div>

      {error && (
        <p role="alert" className={shell.alert}>
          {error}
        </p>
      )}
      {preview.state === "loading" && <p className={shell.loading}>Loading…</p>}
      {preview.state === "error" && (
        <p role="alert" className={shell.alert}>
          {preview.message}
        </p>
      )}
      {preview.state === "ready" && (
        <>
          <div className={styles.legend2}>
            <span>
              <span className={styles.legendL}>L+</span> {counts.LeftAdded}{" "}
              added by {left}
            </span>
            <span>
              <span className={styles.legendR}>R+</span> {counts.RightAdded}{" "}
              added by {right}
            </span>
            <span>
              <span className={styles.legendD}>−</span> {counts.Removed} removed
            </span>
            <span>{counts.Unchanged} unchanged</span>
          </div>
          <section aria-label="Merged result preview" className={styles.sumBox}>
            <div className={styles.sumHead}>Result preview</div>
            <div className={styles.sumBody}>
              {preview.value.lines.map((l, i) => {
                const t = TAGS[l.mark];
                return (
                  <div key={i} className={`${styles.sumRow} ${t.cls}`}>
                    <span className={styles.sumTag} aria-hidden="true">
                      {t.tag}
                    </span>
                    <span className={styles.sumText}>
                      {t.sr && <span className={shell.sr}>{t.sr}</span>}
                      {l.text}
                    </span>
                  </div>
                );
              })}
            </div>
          </section>
        </>
      )}
    </PanelShell>
  );
}
