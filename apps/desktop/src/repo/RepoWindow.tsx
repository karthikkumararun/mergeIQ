import { useCallback, useEffect, useMemo, useState } from "react";
import { useStore } from "zustand";
import { loadMergeSettings, saveMergeSettings } from "../merge-editor/hosts";
import {
  DEFAULT_SETTINGS,
  type MergeSettings,
} from "../merge-editor/model/types";
import { ConflictsPanel } from "./ConflictsPanel";
import { EditorTabs } from "./EditorTabs";
import { OperationBanner } from "./OperationBanner";
import { RepoConfirm, UnsavedDialog } from "./dialogs";
import { operationName } from "./describe";
import type { AcceptSide, RepoApi } from "./repoApi";
import { createRepoStore, sortedConflicts, type RepoStore } from "./repoStore";
import styles from "./RepoWindow.module.css";

const WIDTH_KEY = "mergeiq.repo.panelWidth";
const MIN_WIDTH = 280;
const MAX_WIDTH = 520;
const DEFAULT_WIDTH = 340;

function storedWidth(): number {
  try {
    const raw = Number(window.localStorage.getItem(WIDTH_KEY));
    if (raw >= MIN_WIDTH && raw <= MAX_WIDTH) return raw;
  } catch {
    // storage unavailable: use the default
  }
  return DEFAULT_WIDTH;
}

type Pending =
  | { kind: "accept"; paths: string[]; side: AcceptSide }
  | { kind: "abort" }
  | { kind: "skip" }
  | { kind: "reopen"; path: string; display: string }
  | { kind: "delete"; path: string; display: string };

interface Unsaved {
  files: string[];
  action: string;
  proceed: () => void;
}

interface Props {
  api: RepoApi;
  /** Supplied by tests; otherwise a store is created for `api`. */
  store?: RepoStore;
}

/** A repository window: operation banner, conflicts panel, editor tabs. */
export function RepoWindow({ api, store: injected }: Props) {
  const store = useMemo(
    () => injected ?? createRepoStore(api),
    [api, injected],
  );
  const state = useStore(store);
  const [settings, setSettings] = useState<MergeSettings>(DEFAULT_SETTINGS);
  const [width, setWidth] = useState(storedWidth);
  const [pending, setPending] = useState<Pending | null>(null);
  const [unsaved, setUnsaved] = useState<Unsaved | null>(null);
  const [saveRequests, setSaveRequests] = useState<Record<string, number>>({});

  useEffect(() => {
    void store.start();
    void loadMergeSettings().then((loaded) => {
      setSettings(loaded);
      store.setAutoAdvance(loaded.autoAdvanceAfterSave);
    });
  }, [store]);

  // Live refresh: git-adapter's change events re-read the repository state.
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    void api
      .onChanged(() => void store.refresh())
      .then((fn) => {
        if (cancelled) fn();
        else off = fn;
      });
    return () => {
      cancelled = true;
      off?.();
    };
  }, [api, store]);

  const dirtyFiles = useCallback(
    () =>
      store
        .getState()
        .tabs.filter((t) => t.dirty)
        .map((t) => t.display),
    [store],
  );

  // Closing the window with unsaved editor changes asks first.
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    void api
      .onCloseRequested(() => {
        const files = dirtyFiles();
        if (files.length === 0) return true;
        setUnsaved({
          files,
          action: "close this window",
          proceed: () => void api.closeWindow(),
        });
        return false;
      })
      .then((fn) => {
        if (cancelled) fn();
        else off = fn;
      });
    return () => {
      cancelled = true;
      off?.();
    };
  }, [api, dirtyFiles]);

  /** Runs `run`, after Save/Discard/Cancel if any editor has unsaved changes. */
  const guard = (action: string, run: () => void) => {
    const files = dirtyFiles();
    if (files.length === 0) run();
    else setUnsaved({ files, action, proceed: run });
  };

  const closeTab = (path: string) => {
    const tab = store.getState().tabs.find((t) => t.path === path);
    if (tab?.dirty) {
      setUnsaved({
        files: [tab.display],
        action: "close this tab",
        proceed: () => store.closeTab(path),
      });
    } else {
      store.closeTab(path);
    }
  };

  const requestAccept = (paths: string[], side: AcceptSide) => {
    if (paths.length > 1) setPending({ kind: "accept", paths, side });
    else void store.acceptMany(paths, side);
  };

  const onSettingsChange = (next: MergeSettings) => {
    setSettings(next);
    store.setAutoAdvance(next.autoAdvanceAfterSave);
    void saveMergeSettings(next);
  };

  const resize = (delta: number) =>
    setWidth((w) => Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, w + delta)));

  useEffect(() => {
    try {
      window.localStorage.setItem(WIDTH_KEY, String(width));
    } catch {
      // storage unavailable: the width just isn't remembered
    }
  }, [width]);

  const { status, info } = state;
  const conflicts = sortedConflicts(status);
  const displayOf = (paths: string[]) =>
    paths.map((p) => conflicts.find((c) => c.path === p)?.display ?? p);
  const leftName = status
    ? (status.labels.ours.refName ?? status.labels.ours.role)
    : "";
  const rightName = status
    ? (status.labels.theirs.refName ?? status.labels.theirs.role)
    : "";
  const opName = status ? operationName(status.operation) : "";

  const confirm = () => {
    if (!pending) return null;
    const done = () => setPending(null);
    switch (pending.kind) {
      case "accept": {
        const label = pending.side === "Ours" ? "Left" : "Right";
        const name = pending.side === "Ours" ? leftName : rightName;
        return (
          <RepoConfirm
            title={`Accept ${label} for ${pending.paths.length} files?`}
            message={`Each file is replaced with the ${label.toLowerCase()} version (${name}) and staged.`}
            files={displayOf(pending.paths)}
            confirmLabel={`Accept ${label}`}
            onConfirm={() => {
              done();
              void store.acceptMany(pending.paths, pending.side);
            }}
            onCancel={done}
          />
        );
      }
      case "abort":
        return (
          <RepoConfirm
            title={`Abort the ${opName}?`}
            message={`git ${opName} --abort restores the state from before the ${opName}. Resolutions made so far are lost.`}
            confirmLabel={`Abort ${opName}`}
            danger
            onConfirm={() => {
              done();
              void store.runOperation("abort");
            }}
            onCancel={done}
          />
        );
      case "skip":
        return (
          <RepoConfirm
            title="Skip this commit?"
            message="The commit being replayed is dropped from the rebase, together with the resolutions made for it."
            confirmLabel="Skip commit"
            danger
            onConfirm={() => {
              done();
              void store.runOperation("skip");
            }}
            onCancel={done}
          />
        );
      case "reopen":
        return (
          <RepoConfirm
            title="Reopen this conflict?"
            message={`${pending.display} returns to the conflict list with its original versions. Your resolution is discarded.`}
            confirmLabel="Reopen conflict"
            onConfirm={() => {
              done();
              void store.reopen(pending.path);
            }}
            onCancel={done}
          />
        );
      case "delete":
        return (
          <RepoConfirm
            title="Delete this file?"
            message={`${pending.display} is removed from the working tree and the index.`}
            confirmLabel="Delete file"
            danger
            onConfirm={() => {
              done();
              void store.deleteFile(pending.path);
            }}
            onCancel={done}
          />
        );
    }
  };

  if (!status) {
    return (
      <div className={styles.shell}>
        {state.error ? (
          <p role="alert" className={styles.fatal}>
            {state.error}
          </p>
        ) : (
          <p className={styles.fatal}>Loading…</p>
        )}
      </div>
    );
  }

  const first = conflicts[0] ?? null;
  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--accent)"
          strokeWidth="2.2"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M5 3v5c0 4 7 4 7 8v5" />
          <path d="M19 3v5c0 4-7 4-7 8" />
        </svg>
        <span className={styles.name}>{info?.name}</span>
        <span className={styles.path}>{info?.path ?? status.root}</span>
      </header>

      <OperationBanner
        status={status}
        busy={state.opBusy}
        gitOutput={state.gitOutput}
        opError={state.opError}
        onContinue={() =>
          guard("continue", () => void store.runOperation("continue"))
        }
        onAbort={() => guard("abort", () => setPending({ kind: "abort" }))}
        onSkip={() =>
          guard("skip this commit", () => setPending({ kind: "skip" }))
        }
      />

      {state.error && (
        <div role="alert" className={styles.error}>
          <span className={styles.errorText}>{state.error}</span>
          <button type="button" onClick={store.dismissError}>
            Dismiss
          </button>
        </div>
      )}

      <div className={styles.split}>
        <div className={styles.side} style={{ width }}>
          <ConflictsPanel
            status={status}
            filter={state.filter}
            view={state.view}
            selection={state.selection}
            activePath={state.activeTab}
            resolved={state.resolved}
            onFilter={store.setFilter}
            onView={store.setView}
            onToggle={store.toggleSelected}
            onSelectAll={store.setSelection}
            onClearSelection={store.clearSelection}
            onOpen={store.openFile}
            onAccept={requestAccept}
            onReopen={(path) =>
              setPending({
                kind: "reopen",
                path,
                display:
                  state.resolved.find((r) => r.path === path)?.display ?? path,
              })
            }
          />
        </div>
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize conflicts panel"
          aria-valuenow={width}
          aria-valuemin={MIN_WIDTH}
          aria-valuemax={MAX_WIDTH}
          tabIndex={0}
          className={styles.handle}
          onPointerDown={(e) => e.currentTarget.setPointerCapture(e.pointerId)}
          onPointerMove={(e) => {
            if (e.buttons === 1) resize(e.movementX);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowLeft") {
              e.preventDefault();
              resize(-24);
            } else if (e.key === "ArrowRight") {
              e.preventDefault();
              resize(24);
            }
          }}
        />
        <main className={styles.main}>
          <EditorTabs
            tabs={state.tabs}
            activeTab={state.activeTab}
            api={api}
            settings={settings}
            saveRequests={saveRequests}
            firstUnresolved={
              first ? { path: first.path, display: first.display } : null
            }
            allResolved={status.conflicts.length === 0}
            noOperation={status.operation.kind === "None"}
            branch={status.branch}
            onSettingsChange={onSettingsChange}
            onFocus={store.focusTab}
            onClose={closeTab}
            onOpen={store.openFile}
            onDirty={store.setDirty}
            onResolved={store.fileResolved}
            onRefresh={store.refresh}
            onReload={store.reloadTab}
            onAccept={(path, side) => void store.acceptFile(path, side)}
            onDelete={(path) =>
              setPending({
                kind: "delete",
                path,
                display: displayOf([path])[0],
              })
            }
          />
        </main>
      </div>

      {confirm()}
      {unsaved && (
        <UnsavedDialog
          files={unsaved.files}
          action={unsaved.action}
          onCancel={() => setUnsaved(null)}
          onDiscard={() => {
            const { proceed } = unsaved;
            setUnsaved(null);
            // Discarded edits no longer count as unsaved.
            for (const t of store.getState().tabs)
              store.setDirty(t.path, false);
            proceed();
          }}
          onSave={() => {
            // "Save…" brings up the editor's own Apply flow for the first unsaved file.
            const tab = store.getState().tabs.find((t) => t.dirty);
            setUnsaved(null);
            if (tab) {
              store.focusTab(tab.path);
              setSaveRequests((r) => ({
                ...r,
                [tab.path]: (r[tab.path] ?? 0) + 1,
              }));
            }
          }}
        />
      )}
    </div>
  );
}
