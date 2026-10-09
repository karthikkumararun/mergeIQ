import { useMemo, useState } from "react";
import type { ConflictLoad, RenameInfo } from "../ipc/bindings";
import { typeLabel } from "../repo/describe";
import type { RepoApi, WorkingText } from "../repo/repoApi";
import type { ResolutionMethod } from "../repo/repoStore";
import { unifiedDiff, type UnifiedRow } from "./diff";
import { ago, leftName, rightName } from "./format";
import { useAction, useAsync } from "./hooks";
import { PanelShell } from "./PanelShell";
import styles from "./Panels.module.css";
import { PlainEditor } from "./PlainEditor";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  onBack: () => void;
  onResolved: (method: ResolutionMethod) => Promise<void>;
  /** Removes the file (`git rm`) and closes the tab. */
  onDelete: () => void;
}

function Row({ row }: { row: UnifiedRow }) {
  const [from, to] = row.emphasis ?? [0, 0];
  const body =
    row.emphasis && to > from ? (
      <>
        {row.text.slice(0, from)}
        <span className={styles.em}>{row.text.slice(from, to)}</span>
        {row.text.slice(to)}
      </>
    ) : (
      row.text
    );
  const cls =
    row.kind === "del" ? styles.ddel : row.kind === "add" ? styles.dadd : "";
  return (
    <div className={`${styles.drow} ${cls}`}>
      <span>{row.oldNo ?? ""}</span>
      <span>{row.newNo ?? ""}</span>
      <span aria-hidden="true">
        {row.kind === "del" ? "−" : row.kind === "add" ? "+" : ""}
      </span>
      <span>
        {row.kind === "del" && <span className={shell.sr}>Removed: </span>}
        {row.kind === "add" && <span className={shell.sr}>Added: </span>}
        {body}
      </span>
    </div>
  );
}

function renameText(r: RenameInfo): string {
  return `renamed ${r.from} to ${r.to}`;
}

/** A file one side deleted and the other modified: keep it, delete it, or keep and edit. */
export function ModifyDeletePanel({
  load,
  api,
  onBack,
  onResolved,
  onDelete,
}: Props) {
  const { entry, labels, context } = load;
  const view = useAsync(() => api.modifyDeleteView(entry.path), entry.path);
  const details = useAsync(() => api.details(entry.path), entry.path);
  const { busy, error, run } = useAction();
  const [editing, setEditing] = useState<WorkingText | null>(null);

  const left = leftName(labels);
  const right = rightName(labels);
  const deletedBy = view.state === "ready" ? view.value.deletedBy : null;
  // Fall back on the conflict type until the view has loaded.
  const leftDeleted = deletedBy
    ? deletedBy === "Ours"
    : entry.conflictType === "DeletedByUs";
  const survivorSide = leftDeleted ? "Theirs" : "Ours";
  const survivor = leftDeleted ? right : left;
  const deleter = leftDeleted ? left : right;
  const renames = details.state === "ready" ? details.value.renames : [];

  const diff = useMemo(() => {
    if (view.state !== "ready") return null;
    const { baseText, survivorText, hunks } = view.value;
    if (baseText === null || survivorText === null) return null;
    return unifiedDiff(baseText, survivorText, hunks);
  }, [view]);

  const commit = (side: "Ours" | "Theirs") => {
    const c = (side === "Ours" ? context.ours : context.theirs)[0];
    return c ? (
      <span className={styles.commit}>
        <span className={shell.sha}>{c.shortSha}</span> {c.subject} · {c.author}{" "}
        · {ago(c.date)}
      </span>
    ) : null;
  };
  const sideCard = (side: "Ours" | "Theirs", name: string, label: string) => {
    const deleted = (side === "Ours") === leftDeleted;
    const own = renames.filter((r) => r.side === side);
    return (
      <div
        className={`${styles.sideCard} ${deleted ? styles.sideCardDanger : ""}`}
      >
        <span className={shell.eyebrow}>
          {label} · <span className={shell.mono}>{name}</span>
        </span>
        <span
          className={`${styles.sideState} ${deleted ? styles.stateDeleted : styles.stateModified}`}
        >
          {deleted ? "Deleted" : "Modified"}
        </span>
        {commit(side)}
        {own.map((r) => (
          <span key={r.to} className={styles.renameNote}>
            {label} {renameText(r)}
          </span>
        ))}
      </div>
    );
  };

  if (editing) {
    return (
      <PanelShell
        label="Modify/delete conflict"
        display={entry.display}
        badge="Keep and edit"
        tone="info"
        subtitle={typeLabel(entry.conflictType)}
        title={
          <>
            Editing <span className={shell.mono}>{survivor}</span>’s version
          </>
        }
        lead="The file is saved unstaged. It is marked resolved when you save."
        onBack={onBack}
      >
        <PlainEditor
          display={entry.display}
          initial={editing}
          onCancel={() => setEditing(null)}
          onSave={async (text) => {
            await api.save(entry.path, text, editing.encoding, true);
            await onResolved("Kept modified");
          }}
        />
      </PanelShell>
    );
  }

  return (
    <PanelShell
      label="Modify/delete conflict"
      display={entry.display}
      badge={typeLabel(entry.conflictType)}
      tone="danger"
      title={
        <>
          <span className={shell.mono}>{deleter}</span> deleted this file, but{" "}
          <span className={shell.mono}>{survivor}</span> changed it
        </>
      }
      lead="Keep the modified file, delete it, or keep it and edit it first."
      onBack={onBack}
    >
      <div className={shell.grid} style={{ gap: 16 }}>
        {sideCard("Ours", left, "Left")}
        {sideCard("Theirs", right, "Right")}
      </div>

      <section aria-labelledby="diff-h" className={styles.diffBox}>
        <div className={styles.diffHead}>
          <h2 id="diff-h">
            What <span className={shell.mono}>{survivor}</span> changed since
            base
          </h2>
          {diff && (
            <span className={styles.diffStats}>
              {diff.changes} {diff.changes === 1 ? "change" : "changes"} · +
              {diff.added} −{diff.removed}
            </span>
          )}
        </div>
        <div className={styles.diffBody}>
          {view.state === "loading" && (
            <p className={shell.loading}>Loading…</p>
          )}
          {view.state === "error" && (
            <p role="alert" className={shell.alert}>
              {view.message}
            </p>
          )}
          {view.state === "ready" && diff && diff.rows.length > 0 && (
            <div role="group" aria-label="Changes since base">
              {diff.rows.map((row, i) => (
                <Row key={i} row={row} />
              ))}
            </div>
          )}
          {view.state === "ready" && !diff && (
            <p className={shell.lead} style={{ padding: "0 14px" }}>
              {view.value.note
                ? `No diff to show: ${view.value.note}.`
                : "No diff to show."}
            </p>
          )}
          {view.state === "ready" && diff && diff.rows.length === 0 && (
            <p className={shell.lead} style={{ padding: "0 14px" }}>
              The file is unchanged since the base.
            </p>
          )}
        </div>
      </section>

      <section
        aria-label="Resolution"
        style={{ display: "flex", flexDirection: "column", gap: 12 }}
      >
        <h2 className={shell.sectionTitle}>Resolve</h2>
        {error && (
          <p role="alert" className={shell.alert}>
            {error}
          </p>
        )}
        <div className={shell.optionCards}>
          <div className={shell.optionCard}>
            <button
              type="button"
              className={`${shell.btn} ${shell.primary}`}
              style={{ alignSelf: "flex-start" }}
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  await api.useSide(entry.path, survivorSide);
                  await onResolved("Kept modified");
                })
              }
            >
              Keep modified
            </button>
            <p>
              Write <span className={shell.mono}>{survivor}</span>’s version and
              stage it.
            </p>
          </div>
          <div className={shell.optionCard}>
            <button
              type="button"
              className={`${shell.btn} ${shell.danger}`}
              style={{ alignSelf: "flex-start" }}
              disabled={busy}
              onClick={onDelete}
            >
              Delete file
            </button>
            <p>
              Remove from working tree and index (
              <span className={shell.mono}>git rm</span>).
            </p>
          </div>
          <div className={shell.optionCard}>
            <button
              type="button"
              className={shell.btn}
              style={{ alignSelf: "flex-start" }}
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  setEditing(await api.keepAndEdit(entry.path, survivorSide));
                })
              }
            >
              Keep and edit
            </button>
            <p>
              Open <span className={shell.mono}>{survivor}</span>’s version in
              an editor tab. Marked resolved when you save.
            </p>
          </div>
        </div>
      </section>
    </PanelShell>
  );
}
