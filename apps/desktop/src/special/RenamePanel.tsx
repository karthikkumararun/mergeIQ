import { useState } from "react";
import type {
  ConflictLoad,
  RenameInfo,
  RenameOutcome,
  RenamePair,
} from "../ipc/bindings";
import type { RepoApi } from "../repo/repoApi";
import { ago, leftName, rightName } from "./format";
import { useAction, useAsync } from "./hooks";
import { PanelShell } from "./PanelShell";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  pair: RenamePair;
  onBack: () => void;
  /** The final path was chosen; the other paths are gone from the conflict list. */
  onChosen: (outcome: RenameOutcome) => Promise<void>;
}

const infoIcon = (
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

/** Both sides renamed a file to different paths: pick the final path. */
export function RenamePanel({ load, api, pair, onBack, onChosen }: Props) {
  const { entry, labels } = load;
  const left = leftName(labels);
  const right = rightName(labels);
  // The commit that did each rename: the newest commit touching that destination path.
  const oursDetails = useAsync(
    () => api.details(pair.ours.toPath),
    pair.ours.toPath,
  );
  const theirsDetails = useAsync(
    () => api.details(pair.theirs.toPath),
    pair.theirs.toPath,
  );
  const [chosen, setChosen] = useState<"Ours" | "Theirs" | null>(
    pair.ours.toPath === entry.path
      ? "Ours"
      : pair.theirs.toPath === entry.path
        ? "Theirs"
        : null,
  );
  const { busy, error, run } = useAction();

  const candidate = (
    side: "Ours" | "Theirs",
    info: RenameInfo,
    name: string,
    label: string,
  ) => {
    const d = side === "Ours" ? oursDetails : theirsDetails;
    const commit =
      d.state === "ready"
        ? side === "Ours"
          ? d.value.context.ours[0]
          : d.value.context.theirs[0]
        : undefined;
    const on = chosen === side;
    return (
      <label className={`${styles.opt} ${on ? styles.optChosen : ""}`}>
        <input
          type="radio"
          name="final-path"
          checked={on}
          onChange={() => setChosen(side)}
        />
        <span className={styles.optBody}>
          <span className={styles.optPath}>{info.to}</span>
          <span className={styles.optMeta}>
            {label} · <span className={shell.mono}>{name}</span>
            {commit && (
              <>
                {" · "}
                <span className={shell.mono}>{commit.shortSha}</span>{" "}
                {commit.subject} · {ago(commit.date)}
              </>
            )}
          </span>
        </span>
      </label>
    );
  };

  const chosenPath =
    chosen === "Ours" ? pair.ours : chosen === "Theirs" ? pair.theirs : null;
  const confirm = () =>
    chosenPath &&
    run(async () => onChosen(await api.renameChoose(chosenPath.toPath)));

  return (
    <PanelShell
      label="Rename conflict"
      display={pair.from}
      badge="Rename / rename"
      title="Both sides moved this file to different places"
      lead="Pick the final path. The other path is removed from the index and working tree."
      maxWidth={1000}
      onBack={onBack}
    >
      <div className={styles.basePath}>
        <span>Base</span>
        <span>{pair.from}</span>
      </div>

      <fieldset className={styles.fieldset}>
        <legend className={styles.legend}>Final path</legend>
        {candidate("Ours", pair.ours, left, "Left")}
        {candidate("Theirs", pair.theirs, right, "Right")}
      </fieldset>

      {pair.contentsDiffer && (
        <div role="note" className={styles.infoNote}>
          {infoIcon}
          <span>
            The contents differ too. After you pick the path, the file opens in
            the merge editor
            {chosenPath ? (
              <>
                {" "}
                at <span className={shell.mono}>{chosenPath.to}</span>
              </>
            ) : null}
            .
          </span>
        </div>
      )}
      {error && (
        <p role="alert" className={shell.alert}>
          {error}
        </p>
      )}
      <div className={styles.footerActions}>
        <button type="button" className={shell.btn} onClick={onBack}>
          Cancel
        </button>
        <button
          type="button"
          className={`${shell.btn} ${shell.primary}`}
          disabled={!chosenPath || busy}
          onClick={() => void confirm()}
        >
          {pair.contentsDiffer
            ? "Use this path and merge content"
            : "Use this path"}
        </button>
      </div>
    </PanelShell>
  );
}
