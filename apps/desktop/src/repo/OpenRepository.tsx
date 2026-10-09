import { useCallback, useEffect, useState } from "react";
import { NotARepoError, type HomeApi, type RecentRepoDto } from "./homeApi";
import { openedLabel } from "./openedLabel";
import styles from "./OpenRepository.module.css";

interface Problem {
  title: string;
  path: string;
  detail: string;
}

/** Home body: open a repository by picker, drop or recent entry; recent repositories. */
export function OpenRepository({ api }: { api: HomeApi }) {
  const [recents, setRecents] = useState<RecentRepoDto[]>([]);
  const [problem, setProblem] = useState<Problem | null>(null);

  const reload = useCallback(() => {
    void api.recents().then(setRecents);
  }, [api]);

  const open = useCallback(
    async (path: string) => {
      setProblem(null);
      try {
        await api.openRepo(path);
        reload();
      } catch (error) {
        setProblem(
          error instanceof NotARepoError
            ? {
                title: "Not a git repository.",
                path,
                detail: "was not added to recents.",
              }
            : {
                title: "Could not open the repository.",
                path,
                detail: error instanceof Error ? error.message : String(error),
              },
        );
      }
    },
    [api, reload],
  );

  const pick = useCallback(async () => {
    const path = await api.pickFolder();
    if (path) await open(path);
  }, [api, open]);

  useEffect(reload, [reload]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void api
      .onDrop((paths) => {
        if (paths[0]) void open(paths[0]);
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [api, open]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void pick();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pick]);

  const modifier = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";

  return (
    <section className={styles.section} aria-labelledby="open-title">
      <div className={styles.intro}>
        <h1 id="open-title" className={styles.h1}>
          Open a repository
        </h1>
        <p className={styles.lead}>
          MergeIQ shows the operation in progress and every conflicted file,
          then lets you resolve them one by one.
        </p>
      </div>

      <div>
        <button
          type="button"
          className={`${styles.btn} ${styles.primary}`}
          onClick={() => void pick()}
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
          </svg>
          Open repository…
          <span className={styles.kbd}>{modifier}O</span>
        </button>
      </div>

      <div className={styles.drop} data-testid="drop-zone">
        <span>Drop a repository folder anywhere on this window</span>
        <span className={styles.hint}>or run mergeiq open ~/code/app</span>
      </div>

      {problem && (
        <div role="alert" className={styles.error}>
          <span className={styles.errorText}>
            <strong>{problem.title}</strong>{" "}
            <span className={styles.path}>{problem.path}</span> {problem.detail}
          </span>
          <button
            type="button"
            className={styles.ib}
            aria-label="Dismiss"
            onClick={() => setProblem(null)}
          >
            ×
          </button>
        </div>
      )}

      <div className={styles.recents}>
        <div className={styles.recentsHead}>
          <h2 className={styles.h2}>Recent repositories</h2>
          <span className={styles.note}>Most recent first</span>
        </div>
        {recents.length === 0 && (
          <p className={styles.empty}>Repositories you open show up here.</p>
        )}
        <ul className={styles.list}>
          {recents.map((r) => (
            <li key={r.path} className={styles.item}>
              {r.exists ? (
                <button
                  type="button"
                  className={styles.row}
                  onClick={() => void open(r.path)}
                >
                  <RecentText r={r} />
                  <span className={styles.when}>{openedLabel(r.openedAt)}</span>
                </button>
              ) : (
                <div
                  className={`${styles.row} ${styles.missing}`}
                  aria-disabled="true"
                >
                  <RecentText r={r} missing />
                  <span className={styles.when}>{openedLabel(r.openedAt)}</span>
                </div>
              )}
              {r.exists ? (
                <button
                  type="button"
                  className={styles.ib}
                  aria-label={`Remove ${r.name} from recents`}
                  onClick={() => void api.removeRecent(r.path).then(setRecents)}
                >
                  ×
                </button>
              ) : (
                <button
                  type="button"
                  className={`${styles.btn} ${styles.small}`}
                  aria-label={`Remove ${r.name} from recents`}
                  onClick={() => void api.removeRecent(r.path).then(setRecents)}
                >
                  Remove
                </button>
              )}
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function RecentText({ r, missing }: { r: RecentRepoDto; missing?: boolean }) {
  return (
    <span className={styles.text}>
      <span className={styles.name}>{r.name}</span>
      <span className={styles.rowPath}>
        {r.path}
        {missing ? " · folder not found" : ""}
      </span>
    </span>
  );
}
