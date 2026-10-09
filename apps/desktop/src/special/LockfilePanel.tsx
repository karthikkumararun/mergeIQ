import { useEffect, useRef, useState } from "react";
import type { ConflictLoad, LockfileKind } from "../ipc/bindings";
import { lockfileName } from "../repo/describe";
import type {
  LockfileRun,
  OutputStream,
  RegenerateResult,
  RepoApi,
} from "../repo/repoApi";
import { leftName, rightName } from "./format";
import { useAsync } from "./hooks";
import { PanelShell } from "./PanelShell";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  kind: LockfileKind;
  /** Absolute path of the repository root, for the working directory line. */
  repoRoot: string;
  onBack: () => void;
  /** The lockfile was staged: record it as resolved and refresh (the panel stays open). */
  onStaged: () => Promise<void>;
  /** Closes the panel after a successful run. */
  onDone: () => void;
  /** Opens the merge editor instead. */
  onMergeByHand: () => void;
}

interface Line {
  stream: OutputStream;
  text: string;
}

type Phase =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "finished"; result: RegenerateResult };

const MAX_LINES = 5000;

const tick = (
  <svg
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.4"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="m5 12 5 5 9-10" />
  </svg>
);
const warnIcon = (
  <svg
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.2"
    strokeLinecap="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v6" />
    <path d="M12 16.5v.5" />
  </svg>
);

const seconds = (ms: number) => `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`;

/** Take a side's lockfile, then regenerate it with the project's tooling after confirmation. */
export function LockfilePanel({
  load,
  api,
  kind,
  repoRoot,
  onBack,
  onStaged,
  onDone,
  onMergeByHand,
}: Props) {
  const { entry, labels } = load;
  const left = leftName(labels);
  const right = rightName(labels);
  const tool = lockfileName(kind);
  const commands = useAsync(() => api.lockfileCommands(), "commands");
  const [side, setSide] = useState<"Ours" | "Theirs" | null>(null);
  const [edited, setEdited] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [lines, setLines] = useState<Line[]>([]);
  const [startError, setStartError] = useState<string | null>(null);
  const [ran, setRan] = useState<string>("");
  const current = useRef<LockfileRun | null>(null);

  const configured =
    commands.state === "ready"
      ? commands.value.find((c) => c.kind === kind)
      : undefined;
  const command = edited ?? configured?.command ?? "";
  const dir = entry.display.includes("/")
    ? entry.display.slice(0, entry.display.lastIndexOf("/"))
    : "";
  const cwd = [repoRoot.replace(/[\\/]+$/, ""), dir].filter(Boolean).join("/");
  const running = phase.kind === "running";

  // Never leave a command running when the panel goes away.
  useEffect(() => () => current.current?.cancel(), []);

  const start = async () => {
    if (!side || !command.trim()) return;
    setStartError(null);
    setLines([]);
    setRan(command);
    setPhase({ kind: "running" });
    try {
      const run = await api.lockfileRegenerate(
        entry.path,
        side,
        command,
        (stream, text) =>
          setLines((prev) =>
            prev.length >= MAX_LINES ? prev : [...prev, { stream, text }],
          ),
      );
      current.current = run;
      const outcome = await run.done;
      current.current = null;
      if (outcome.result) {
        setPhase({ kind: "finished", result: outcome.result });
        if (outcome.result.staged) await onStaged();
      } else {
        setStartError(outcome.error ?? "The command could not be run.");
        setPhase({ kind: "idle" });
      }
    } catch (e) {
      setStartError(e instanceof Error ? e.message : String(e));
      setPhase({ kind: "idle" });
    }
  };

  const pick = (
    s: "Ours" | "Theirs",
    name: string,
    label: string,
    sha?: string,
  ) => (
    <button
      type="button"
      role="radio"
      aria-checked={side === s}
      className={`${styles.pick} ${side === s ? styles.pickOn : ""}`}
      disabled={running}
      onClick={() => setSide(s)}
    >
      <span className={styles.pickName}>Take {label} and regenerate</span>
      <span className={styles.pickMeta}>
        {name}
        {sha ? ` · ${sha}` : ""}
      </span>
    </button>
  );

  const result = phase.kind === "finished" ? phase.result : null;
  const ok = !!result?.staged;

  return (
    <PanelShell
      label="Lockfile conflict"
      display={entry.display}
      badge={`Lockfile · ${tool}`}
      tone="warn"
      title="Regenerate instead of merging by hand"
      lead={
        <>
          Take one side’s lockfile, then let {tool} rebuild it against the
          merged project files.
        </>
      }
      maxWidth={1000}
      onBack={onBack}
    >
      <div
        role="radiogroup"
        aria-label="Side to start from"
        className={styles.picks}
      >
        {pick("Ours", left, "Left", load.context.ours[0]?.shortSha)}
        {pick("Theirs", right, "Right", load.context.theirs[0]?.shortSha)}
      </div>

      <section aria-labelledby="cmd-h" className={styles.confirm}>
        <h2 id="cmd-h">MergeIQ will run this command</h2>
        <label className={styles.field}>
          Command
          <input
            type="text"
            value={command}
            disabled={running || commands.state !== "ready"}
            spellCheck={false}
            onChange={(e) => setEdited(e.target.value)}
          />
        </label>
        <div className={styles.field}>
          Working directory
          <span className={styles.cwd}>{cwd}</span>
        </div>
        <p className={styles.hint}>
          You’ll be asked every time. The file is staged only if the command
          exits 0. Change the default in Settings › Lockfiles.
        </p>
        <p className={styles.warn}>
          The command may change other files in this folder (for example
          installed packages). Only the lockfile is staged.
        </p>
        {startError && (
          <p role="alert" className={shell.alert}>
            {startError}
          </p>
        )}
        <div className={styles.footerActions}>
          <button
            type="button"
            className={shell.btn}
            onClick={onMergeByHand}
            disabled={running}
          >
            Merge by hand
          </button>
          <button
            type="button"
            className={shell.btn}
            onClick={onBack}
            disabled={running}
          >
            Cancel
          </button>
          <button
            type="button"
            className={`${shell.btn} ${shell.primary}`}
            disabled={!side || !command.trim() || running}
            onClick={() => void start()}
          >
            Run command
          </button>
        </div>
      </section>

      {(running || result) && (
        <section aria-label="Command output" className={styles.output}>
          {running && (
            <div className={`${styles.runBar} ${styles.runBusy}`} role="status">
              <span className={styles.spinner} aria-hidden="true" />
              <span>Running…</span>
              <button
                type="button"
                className={shell.btn}
                style={{ height: 30 }}
                onClick={() => current.current?.cancel()}
              >
                Cancel
              </button>
            </div>
          )}
          {result && ok && (
            <div className={`${styles.runBar} ${styles.runOk}`} role="status">
              {tick}
              <span>
                Exited 0 in {seconds(result.run.durationMs ?? 0)} ·{" "}
                <span className={shell.mono}>
                  {entry.display.split("/").pop()}
                </span>{" "}
                staged
              </span>
              <button
                type="button"
                className={`${shell.btn} ${shell.primary}`}
                style={{ height: 30 }}
                onClick={onDone}
              >
                Done
              </button>
            </div>
          )}
          {result && !ok && (
            <div className={`${styles.runBar} ${styles.runFail}`} role="status">
              {warnIcon}
              <span>
                {result.run.cancelled
                  ? "Cancelled"
                  : result.run.exitCode === null
                    ? "Stopped"
                    : `Exited ${result.run.exitCode}`}{" "}
                · not staged · the conflict is still listed
              </span>
              <button
                type="button"
                className={shell.btn}
                style={{ height: 30 }}
                onClick={() => void start()}
              >
                Run again
              </button>
            </div>
          )}
          <pre className={styles.term} aria-label="Output">
            <span>$ {ran}</span>
            {lines.map((l, i) => (
              <span
                key={i}
                className={l.stream === "Stderr" ? styles.termErr : undefined}
              >
                {"\n"}
                {l.text}
              </span>
            ))}
            {result && ok && <span className={styles.termOk}>{"\n"}Done</span>}
          </pre>
        </section>
      )}
    </PanelShell>
  );
}
