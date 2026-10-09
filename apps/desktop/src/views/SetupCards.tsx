import { useEffect, useState } from "react";
import type { CliSetupInfo } from "../ipc/bindings";
import type { CliSetupApi } from "../settings/cliApi";
import styles from "./SetupCards.module.css";

interface Props {
  api: CliSetupApi;
  version: number;
  onOpen: () => void;
}

/** Home › Set up: command-line tool and git mergetool status, linking to Settings. */
export function SetupCards({ api, version, onOpen }: Props) {
  const [info, setInfo] = useState<CliSetupInfo | null>(null);

  useEffect(() => {
    api
      .info()
      .then(setInfo)
      .catch(() => setInfo(null));
  }, [api, version]);

  if (!info) return null;
  const installed = info.installedAt !== null;
  return (
    <section className={styles.cards} aria-labelledby="setup-title">
      <h2 id="setup-title" className={styles.title}>
        Set up
      </h2>
      <div className={styles.card}>
        <div className={styles.head}>
          <span className={styles.name}>Command-line tool</span>
          <span className={installed ? styles.done : styles.todo}>
            {installed ? "Installed" : "Not installed"}
          </span>
        </div>
        <p className={styles.text}>
          Adds <code>mergeiq</code> to your PATH for <code>open</code>,{" "}
          <code>merge</code> and <code>resolve</code>.
        </p>
        <button type="button" className={styles.btn} onClick={onOpen}>
          {installed ? "Manage…" : "Install…"}
        </button>
      </div>
      <div className={styles.card}>
        <div className={styles.head}>
          <span className={styles.name}>git mergetool</span>
          <span
            className={info.mergetoolConfigured ? styles.done : styles.todo}
          >
            {info.mergetoolConfigured ? "Configured" : "Not configured"}
          </span>
        </div>
        <p className={styles.text}>
          Make <code>git mergetool</code> open conflicts in MergeIQ.
        </p>
        <button type="button" className={styles.btn} onClick={onOpen}>
          {info.mergetoolConfigured ? "Manage…" : "Configure…"}
        </button>
      </div>
    </section>
  );
}
