import { useEffect, useState } from "react";
import { lockfileName } from "../repo/describe";
import type {
  LockfileCommand,
  LockfileKind,
  LockfileSettingsApi,
} from "./lockfileApi";
import styles from "./Lockfiles.module.css";

interface Props {
  api: LockfileSettingsApi;
}

function Row({
  entry,
  api,
  onChanged,
}: {
  entry: LockfileCommand;
  api: LockfileSettingsApi;
  onChanged: (all: LockfileCommand[]) => void;
}) {
  const [value, setValue] = useState(entry.command);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const tool = lockfileName(entry.kind);
  const dirty = value.trim() !== entry.command;

  const apply = async (kind: LockfileKind, command: string | null) => {
    setBusy(true);
    setError(null);
    try {
      const all = await api.set(kind, command);
      onChanged(all);
      setValue(all.find((c) => c.kind === kind)?.command ?? "");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.row}>
      <label className={styles.field}>
        <span className={styles.name}>
          {tool}
          {entry.custom && <span className={styles.custom}>customised</span>}
        </span>
        <input
          type="text"
          value={value}
          spellCheck={false}
          aria-label={`${tool} command`}
          onChange={(e) => setValue(e.target.value)}
        />
      </label>
      <div className={styles.buttons}>
        <button
          type="button"
          disabled={busy || !dirty}
          onClick={() => void apply(entry.kind, value)}
        >
          Save
        </button>
        <button
          type="button"
          disabled={busy || (!entry.custom && !dirty)}
          onClick={() => void apply(entry.kind, null)}
        >
          Reset
        </button>
      </div>
      {entry.custom && (
        <p className={styles.default}>
          Default: <code>{entry.defaultCommand}</code>
        </p>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </div>
  );
}

/** Settings › Lockfiles: the command proposed when regenerating each kind of lockfile. */
export function Lockfiles({ api }: Props) {
  const [entries, setEntries] = useState<LockfileCommand[] | null>(null);
  useEffect(() => {
    void api.list().then(setEntries);
  }, [api]);
  return (
    <section className={styles.page} aria-label="Lockfiles">
      <h2 className={styles.title}>Lockfiles</h2>
      <p className={styles.help}>
        When a lockfile conflicts, MergeIQ can take one side and regenerate it
        with your package manager. These are the commands it proposes; you
        confirm the exact command and working directory every time, and it never
        runs through a shell.
      </p>
      {entries?.map((e) => (
        <Row
          // A saved change re-keys the row so its text follows the stored value.
          key={`${e.kind}:${e.command}:${e.custom}`}
          entry={e}
          api={api}
          onChanged={setEntries}
        />
      ))}
    </section>
  );
}
