import { useEffect, useMemo, useState } from "react";
import type { ConflictLoad, RenameOutcome } from "../ipc/bindings";
import { MergeEditor } from "../merge-editor/MergeEditor";
import { renderSaveText } from "../merge-editor/model/serialize";
import type { MergeSettings, SaveResult } from "../merge-editor/model/types";
import { SpecialConflictPanel } from "../special/SpecialConflictPanel";
import { toDocument } from "./toDocument";
import { isLockfile, opensInEditor, splitPath } from "./describe";
import type { AcceptSide, RepoApi } from "./repoApi";
import type { ResolutionMethod, Tab } from "./repoStore";
import styles from "./EditorTabs.module.css";

interface Props {
  tabs: Tab[];
  activeTab: string | null;
  api: RepoApi;
  settings: MergeSettings;
  saveRequests: Record<string, number>;
  /** First unresolved file, offered when no tab is open. */
  firstUnresolved: { path: string; display: string } | null;
  allResolved: boolean;
  noOperation: boolean;
  branch: string | null;
  onSettingsChange: (settings: MergeSettings) => void;
  onFocus: (path: string) => void;
  onClose: (path: string) => void;
  onOpen: (path: string) => void;
  onDirty: (path: string, dirty: boolean) => void;
  /** Absolute path of the repository root (shown as a lockfile command's directory). */
  repoRoot: string;
  onResolved: (path: string, method: ResolutionMethod) => Promise<void>;
  /** Records `path` as resolved without closing its tab. */
  onLogResolved: (path: string, method: ResolutionMethod) => void;
  onRenameChosen: (path: string, outcome: RenameOutcome) => Promise<void>;
  onRefresh: () => Promise<void>;
  onReload: (path: string) => void;
  onAccept: (path: string, side: AcceptSide) => void;
  onDelete: (path: string) => void;
}

/** Tab strip plus one pane per open file; hidden panes stay mounted to keep editor state. */
export function EditorTabs(props: Props) {
  const { tabs, activeTab } = props;
  return (
    <div className={styles.wrap}>
      <div role="tablist" aria-label="Open files" className={styles.strip}>
        {tabs.map((tab) => {
          const active = tab.path === activeTab;
          const { file } = splitPath(tab.display);
          return (
            <div
              key={tab.path}
              className={`${styles.tab} ${active ? styles.current : ""}`}
            >
              <button
                type="button"
                role="tab"
                aria-selected={active}
                className={styles.tabButton}
                title={tab.display}
                onClick={() => props.onFocus(tab.path)}
              >
                <span className={styles.name}>{file}</span>
                {tab.dirty && (
                  <span
                    className={styles.dot}
                    role="img"
                    aria-label="Unsaved changes"
                  />
                )}
              </button>
              <button
                type="button"
                className={styles.close}
                aria-label={`Close ${file}`}
                onClick={() => props.onClose(tab.path)}
              >
                ×
              </button>
            </div>
          );
        })}
      </div>
      <div className={styles.body}>
        {tabs.map((tab) => (
          <div
            key={tab.path}
            role="tabpanel"
            aria-label={tab.display}
            className={`${styles.pane} ${tab.path === activeTab ? "" : styles.hidden}`}
            data-active={tab.path === activeTab}
          >
            <TabPane {...props} tab={tab} />
          </div>
        ))}
        {!activeTab && <EmptyState {...props} />}
      </div>
    </div>
  );
}

function EmptyState({
  firstUnresolved,
  allResolved,
  noOperation,
  branch,
  onOpen,
}: Props) {
  return (
    <div className={styles.empty}>
      {firstUnresolved ? (
        <>
          <h2>Select a file to resolve</h2>
          <p>
            Start with{" "}
            <button
              type="button"
              className={styles.link}
              onClick={() => onOpen(firstUnresolved.path)}
            >
              {splitPath(firstUnresolved.display).file}
            </button>
            .
          </p>
        </>
      ) : noOperation ? (
        <>
          <h2>No conflicts to resolve</h2>
          <p>{branch ? `On branch ${branch}.` : "No branch checked out."}</p>
        </>
      ) : (
        <>
          <h2>
            {allResolved ? "All conflicts resolved" : "Nothing to resolve"}
          </h2>
          <p>Use Continue above to finish the operation.</p>
        </>
      )}
    </div>
  );
}

type Loaded =
  | { state: "loading" }
  | { state: "error"; message: string }
  | { state: "ready"; load: ConflictLoad };

function TabPane(props: Props & { tab: Tab }) {
  const { tab, api } = props;
  const [loaded, setLoaded] = useState<Loaded>({ state: "loading" });
  // A lockfile shows its regenerate panel first; "Merge by hand" switches to the editor.
  const [byHand, setByHand] = useState(false);

  useEffect(() => {
    let live = true;
    void api
      .loadConflict(tab.path)
      .then((load) => live && setLoaded({ state: "ready", load }))
      .catch(
        (e: unknown) =>
          live &&
          setLoaded({
            state: "error",
            message: e instanceof Error ? e.message : String(e),
          }),
      );
    return () => {
      live = false;
    };
    // The editor reloads when `epoch` changes (Reload).
  }, [api, tab.path, tab.epoch]);

  // A new document object remounts the editor and discards its edits, so keep it stable.
  const doc = useMemo(
    () =>
      loaded.state === "ready" && loaded.load.analysis
        ? toDocument(loaded.load)
        : null,
    [loaded],
  );

  const onSave = async (result: SaveResult) => {
    const stage = result.mode !== "markers";
    await api.save(tab.path, renderSaveText(result), result.encoding, stage);
    if (stage) await props.onResolved(tab.path, "Merged");
    else await props.onRefresh();
  };

  return (
    <div className={styles.paneInner}>
      {tab.notice && (
        <div role="status" className={styles.notice}>
          <span>
            <strong>{splitPath(tab.display).file}</strong>{" "}
            {tab.notice === "resolved-outside"
              ? "was resolved outside MergeIQ"
              : "was changed outside MergeIQ"}
          </span>
          <span className={styles.noticeActions}>
            <button type="button" onClick={() => props.onClose(tab.path)}>
              Close tab
            </button>
            <button type="button" onClick={() => props.onReload(tab.path)}>
              Reload
            </button>
          </span>
        </div>
      )}
      <div className={styles.content}>
        {loaded.state === "loading" && (
          <p className={styles.message}>Loading {tab.display}…</p>
        )}
        {loaded.state === "error" && (
          <p className={styles.message} role="alert">
            {loaded.message}
          </p>
        )}
        {loaded.state === "ready" &&
          (doc &&
          opensInEditor(loaded.load.entry) &&
          (!isLockfile(loaded.load.entry) || byHand) ? (
            <MergeEditor
              key={`${tab.path}:${tab.epoch}`}
              doc={doc}
              settings={props.settings}
              onSettingsChange={props.onSettingsChange}
              onSave={onSave}
              onCancel={() => props.onClose(tab.path)}
              onDirtyChange={(dirty) => props.onDirty(tab.path, dirty)}
              saveRequest={props.saveRequests[tab.path] ?? 0}
              reanalyze={(policy) => api.analyze(tab.path, policy)}
            />
          ) : (
            <SpecialConflictPanel
              key={`${tab.path}:${tab.epoch}`}
              load={loaded.load}
              api={api}
              repoRoot={props.repoRoot}
              onBack={() => props.onClose(tab.path)}
              onResolved={(method) => props.onResolved(tab.path, method)}
              onAccept={(side) => props.onAccept(tab.path, side)}
              onDelete={() => props.onDelete(tab.path)}
              onStaged={async () => {
                props.onLogResolved(tab.path, "Regenerated");
                await props.onRefresh();
              }}
              onRenameChosen={(outcome) =>
                props.onRenameChosen(tab.path, outcome)
              }
              onMergeByHand={() => setByHand(true)}
            />
          ))}
      </div>
    </div>
  );
}
