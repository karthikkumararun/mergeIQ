import { useEffect, useState } from "react";
import type { CliSetupInfo, InstallOutcome } from "../ipc/bindings";
import { ConfirmDialog } from "../merge-editor/dialogs/ConfirmDialog";
import type { CliSetupApi } from "./cliApi";
import styles from "./CommandLine.module.css";

const CHOOSE = "__choose__";

interface Props {
  api: CliSetupApi;
  /** Called after an install or configuration so other views can refresh their status. */
  onChanged?: () => void;
}

/** Settings › Command line: install the `mergeiq` command, register the git mergetool. */
export function CommandLine({ api, onChanged }: Props) {
  const [info, setInfo] = useState<CliSetupInfo | null>(null);
  const [choice, setChoice] = useState<string>("");
  const [custom, setCustom] = useState("");
  const [outcome, setOutcome] = useState<InstallOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [noBackup, setNoBackup] = useState(false);
  const [lines, setLines] = useState<string[]>([]);
  const [confirming, setConfirming] = useState(false);
  const [configured, setConfigured] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    void api.info().then((loaded) => {
      setInfo(loaded);
      setChoice(loaded.dirs[0]?.path ?? "");
      setConfigured(loaded.mergetoolConfigured);
    });
    void api.commands(true).then(setLines);
  }, [api]);

  if (!info) return null;
  const windows = info.platform === "windows";
  const dir = choice === CHOOSE ? custom.trim() : choice;
  const picked = info.dirs.find((d) => d.path === choice);
  const active = noBackup ? lines : lines.slice(0, -1);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const install = () =>
    run(async () => {
      setOutcome(await api.install(dir, picked?.needsAdmin ?? false));
      onChanged?.();
    });

  const addToPath = () =>
    run(async () => {
      await api.addToPath(dir);
      setOutcome((o) => (o ? { ...o, onPath: true } : o));
      onChanged?.();
    });

  const configure = () =>
    run(async () => {
      setConfirming(false);
      await api.configure(noBackup);
      setConfigured(true);
      onChanged?.();
    });

  const copy = () => {
    void navigator.clipboard?.writeText(active.join("\n")).then(() => {
      setCopied(true);
    });
  };

  return (
    <div className={styles.page}>
      <h1 className={styles.h1}>Command line</h1>

      <section className={styles.section} aria-labelledby="cli-install">
        <div>
          <h2 id="cli-install" className={styles.h2}>
            Install command-line tool
          </h2>
          <p className={styles.help}>
            {windows ? (
              <>
                Add the install folder to your user PATH to use{" "}
                <code className={styles.inline}>mergeiq</code> from a terminal.
              </>
            ) : (
              <>
                Links the bundled binary as{" "}
                <code className={styles.inline}>mergeiq</code> in a folder of
                your choice.
              </>
            )}
          </p>
        </div>
        <div className={styles.installRow}>
          <label className={styles.field}>
            {windows ? "Install folder" : "Install to"}
            <select
              className={styles.select}
              value={choice}
              onChange={(e) => {
                setChoice(e.target.value);
                setOutcome(null);
              }}
            >
              {info.dirs.map((d) => (
                <option key={d.path} value={d.path}>
                  {d.label}
                </option>
              ))}
              {!windows && <option value={CHOOSE}>Choose folder…</option>}
            </select>
          </label>
          <button
            type="button"
            className={`${styles.btn} ${styles.primary}`}
            disabled={busy || !dir}
            onClick={() => void install()}
          >
            {windows ? "Check PATH" : "Install"}
          </button>
        </div>
        {choice === CHOOSE && (
          <label className={styles.field}>
            Folder path
            <input
              className={styles.input}
              value={custom}
              placeholder="/path/to/bin"
              onChange={(e) => setCustom(e.target.value)}
            />
          </label>
        )}
        {!outcome && info.installedAt && !windows && (
          <p className={styles.ok} role="status">
            Installed at{" "}
            <code className={styles.inline}>{info.installedAt}</code>
          </p>
        )}
        {outcome && !windows && (
          <p className={styles.ok} role="status">
            Installed at{" "}
            <code className={styles.inline}>{outcome.linkPath}</code>
          </p>
        )}
        {outcome && !outcome.onPath && (
          <div className={styles.warn} role="status">
            <svg
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
              className={styles.warnIcon}
            >
              <path d="M12 3 2 21h20z" />
              <path d="M12 10v5" />
              <path d="M12 18v.5" />
            </svg>
            <div className={styles.warnBody}>
              {outcome.pathHint ? (
                <>
                  <span>
                    <code className={styles.inline}>{dir}</code> is not on your
                    PATH. Add this to{" "}
                    <code className={styles.inline}>
                      {outcome.pathHint.rcFile}
                    </code>
                    :
                  </span>
                  <code className={styles.hint}>{outcome.pathHint.line}</code>
                </>
              ) : (
                <>
                  <span>
                    <code className={styles.inline}>{dir}</code> is not on your
                    user PATH.
                  </span>
                  <button
                    type="button"
                    className={styles.btn}
                    disabled={busy}
                    onClick={() => void addToPath()}
                  >
                    Add to PATH
                  </button>
                </>
              )}
            </div>
          </div>
        )}
        {outcome?.onPath && (
          <p className={styles.ok} role="status">
            <code className={styles.inline}>{dir}</code> is on your PATH.
          </p>
        )}
      </section>

      <section className={styles.section} aria-labelledby="cli-git">
        <div>
          <h2 id="cli-git" className={styles.h2}>
            Configure as git mergetool
          </h2>
          <p className={styles.help}>
            These commands change your global git config. Nothing runs until you
            confirm.
          </p>
        </div>
        <pre className={styles.pre} aria-label="git config commands">
          {lines.map((line, i) => {
            const optional = i === lines.length - 1;
            return (
              <span
                key={line}
                className={optional && !noBackup ? styles.dim : undefined}
              >
                {line}
                {i < lines.length - 1 ? "\n" : ""}
              </span>
            );
          })}
        </pre>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={noBackup}
            onChange={(e) => setNoBackup(e.target.checked)}
          />
          <span>
            Also stop git from keeping{" "}
            <code className={styles.inline}>.orig</code> backup files
          </span>
        </label>
        <div className={styles.actions}>
          <button
            type="button"
            className={`${styles.btn} ${styles.primary}`}
            disabled={busy}
            onClick={() => setConfirming(true)}
          >
            Run {active.length} commands…
          </button>
          <button type="button" className={styles.btn} onClick={copy}>
            {copied ? "Copied" : "Copy commands"}
          </button>
        </div>
        {configured && (
          <p className={styles.ok} role="status">
            git is configured to use MergeIQ as its merge tool.
          </p>
        )}
      </section>

      {error && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}

      <section className={styles.section} aria-labelledby="cli-usage">
        <h2 id="cli-usage" className={styles.h2}>
          Usage
        </h2>
        <dl className={styles.usage}>
          <dt>
            <code>mergeiq open &lt;dir&gt;</code>
          </dt>
          <dd>Open a repository window</dd>
          <dt>
            <code>mergeiq resolve &lt;path&gt;</code>
          </dt>
          <dd>Resolve one conflicted file, or a file with conflict markers</dd>
          <dt>
            <code>mergeiq merge B L R M</code>
          </dt>
          <dd>
            Mergetool mode. Exits 0 when resolved, 1 when cancelled or saved
            with markers
          </dd>
        </dl>
      </section>

      {confirming && (
        <ConfirmDialog
          title={`Run ${active.length} git config commands?`}
          message="This changes your global git config so that git mergetool opens conflicts in MergeIQ."
          confirmLabel={`Run ${active.length} commands`}
          cancelLabel="Cancel"
          onConfirm={() => void configure()}
          onCancel={() => setConfirming(false)}
        />
      )}
    </div>
  );
}
