import { useEffect, useState } from "react";
import { commands } from "../ipc/bindings";
import { useTheme } from "../theme/useTheme";
import styles from "../views/Settings.module.css";

/** Placeholder for `mergeiq open <dir>` (`/repo/<id>`) until the repository browser lands. */
export function RepoRequestView({ id }: { id: number }) {
  useTheme();
  const [dir, setDir] = useState<string | null>(null);

  useEffect(() => {
    void commands.repoRequestLoad(id).then((result) => {
      if (result.status === "ok") setDir(result.data.dir);
    });
  }, [id]);

  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <h1 className={styles.title}>Repository</h1>
        <button
          type="button"
          className={styles.closeButton}
          onClick={() => void commands.requestClose(id, "repo")}
        >
          Close
        </button>
      </header>
      <section className={styles.section}>
        <p style={{ margin: 0, fontFamily: "var(--font-mono)" }}>{dir}</p>
        <p style={{ margin: 0, color: "var(--muted)" }}>
          The repository browser is not available yet.
        </p>
      </section>
    </div>
  );
}
