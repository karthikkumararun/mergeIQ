import { createStore, type StoreApi } from "zustand/vanilla";
import type {
  ConflictEntry,
  PathToken,
  RenameOutcome,
  RepoInfo,
} from "../ipc/bindings";
import type {
  AcceptSide,
  ControlOutcome,
  RepoApi,
  RepoStatus,
} from "./repoApi";

/** Most merge editors kept open at once; beyond this the least recently used clean tab goes. */
export const MAX_TABS = 10;

export type ResolutionMethod =
  | "Merged"
  | "Accepted Left"
  | "Accepted Right"
  | "Deleted"
  | "Kept modified"
  | "Auto-merged"
  | "Regenerated"
  | "Renamed";

export type TabNotice = "resolved-outside" | "changed-outside" | null;

export interface Tab {
  path: PathToken;
  display: string;
  dirty: boolean;
  notice: TabNotice;
  /** Stage object ids when the tab was opened, to notice external changes. */
  signature: string;
  /** Bumped to remount the editor (Reload). */
  epoch: number;
  lastUsed: number;
}

export interface ResolvedItem {
  path: PathToken;
  display: string;
  method: ResolutionMethod;
}

export type ViewMode = "flat" | "folders";

export interface RepoState {
  info: RepoInfo | null;
  status: RepoStatus | null;
  loaded: boolean;
  /** A problem refreshing or acting, shown to the user. */
  error: string | null;
  filter: string;
  view: ViewMode;
  selection: PathToken[];
  tabs: Tab[];
  activeTab: PathToken | null;
  resolved: ResolvedItem[];
  /** Output of the last continue/abort/skip. */
  gitOutput: string;
  /** Failure text of the last continue/abort/skip, verbatim. */
  opError: string | null;
  opBusy: boolean;
  autoAdvance: boolean;
}

export const signatureOf = (entry: ConflictEntry): string =>
  entry.stages.map((s) => `${s.stage}:${s.oid}`).join(",");

/** Folds a fresh status into the state: keeps selection and tabs, flags external changes. */
export function applyStatus(state: RepoState, status: RepoStatus): RepoState {
  const byPath = new Map(status.conflicts.map((c) => [c.path, c]));
  const ours = new Set(state.resolved.map((r) => r.path));
  return {
    ...state,
    status,
    loaded: true,
    selection: state.selection.filter((p) => byPath.has(p)),
    // A file that is a conflict again (reopened, or restored outside) is no longer resolved.
    resolved: state.resolved.filter((r) => !byPath.has(r.path)),
    tabs: state.tabs.map((tab) => {
      const entry = byPath.get(tab.path);
      if (!entry) {
        // Resolved by us means we are about to close the tab; anything else is external.
        return ours.has(tab.path) || tab.notice === "resolved-outside"
          ? tab
          : { ...tab, notice: "resolved-outside" as const };
      }
      return signatureOf(entry) !== tab.signature &&
        tab.notice !== "resolved-outside"
        ? { ...tab, notice: "changed-outside" as const }
        : tab;
    }),
  };
}

/** Files after the visible filter, in list order (sorted by path). */
export function sortedConflicts(status: RepoStatus | null): ConflictEntry[] {
  return [...(status?.conflicts ?? [])].sort((a, b) =>
    a.display < b.display ? -1 : a.display > b.display ? 1 : 0,
  );
}

/** The next conflict after `path` in list order (wrapping), excluding `path` itself. */
export function nextUnresolved(
  list: ConflictEntry[],
  path: PathToken,
): ConflictEntry | null {
  const rest = list.filter((c) => c.path !== path);
  if (rest.length === 0) return null;
  const at = list.findIndex((c) => c.path === path);
  if (at < 0) return rest[0];
  return list.slice(at + 1).find((c) => c.path !== path) ?? rest[0];
}

export type RepoStore = StoreApi<RepoState> & RepoActions;

export interface RepoActions {
  api: RepoApi;
  start(): Promise<void>;
  refresh(): Promise<void>;
  setFilter(filter: string): void;
  setView(view: ViewMode): void;
  setAutoAdvance(on: boolean): void;
  toggleSelected(path: PathToken): void;
  clearSelection(): void;
  setSelection(paths: PathToken[]): void;
  openFile(path: PathToken): void;
  focusTab(path: PathToken): void;
  closeTab(path: PathToken): void;
  setDirty(path: PathToken, dirty: boolean): void;
  reloadTab(path: PathToken): void;
  /** Called after the editor saved `path` as resolved. */
  fileResolved(path: PathToken, method: ResolutionMethod): Promise<void>;
  /**
   * Records that `path` was resolved without closing its tab (a panel that still shows
   * results, e.g. a lockfile run's output), so the refresh that follows is not flagged as an
   * outside change.
   */
  logResolved(path: PathToken, method: ResolutionMethod): void;
  /**
   * After a rename/rename choice: the three involved paths are gone from the list. Opens the
   * chosen path in a tab when it now needs a text merge, otherwise logs `from` as resolved.
   */
  renameChosen(from: PathToken, outcome: RenameOutcome): Promise<void>;
  acceptSelected(side: AcceptSide): Promise<void>;
  acceptMany(paths: PathToken[], side: AcceptSide): Promise<void>;
  acceptFile(path: PathToken, side: AcceptSide): Promise<void>;
  deleteFile(path: PathToken): Promise<void>;
  reopen(path: PathToken): Promise<void>;
  runOperation(kind: "continue" | "abort" | "skip"): Promise<void>;
  dismissError(): void;
  anyDirty(): boolean;
}

const INITIAL: RepoState = {
  info: null,
  status: null,
  loaded: false,
  error: null,
  filter: "",
  view: "flat",
  selection: [],
  tabs: [],
  activeTab: null,
  resolved: [],
  gitOutput: "",
  opError: null,
  opBusy: false,
  autoAdvance: true,
};

const message = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

/** One store per repository window. */
export function createRepoStore(
  api: RepoApi,
  initial: Partial<RepoState> = {},
): RepoStore {
  const store = createStore<RepoState>(() => ({ ...INITIAL, ...initial }));
  const { getState: get, setState: set } = store;
  let clock = 0;

  const conflicts = () => get().status?.conflicts ?? [];
  const entryOf = (path: PathToken) => conflicts().find((c) => c.path === path);
  const log = (items: ResolvedItem[]) =>
    set((s) => ({
      resolved: [
        ...s.resolved.filter((r) => !items.some((i) => i.path === r.path)),
        ...items,
      ],
    }));
  const resolvedItem = (
    path: PathToken,
    method: ResolutionMethod,
  ): ResolvedItem => ({
    path,
    display: entryOf(path)?.display ?? path,
    method,
  });
  const dropTabs = (paths: PathToken[]) =>
    set((s) => {
      const tabs = s.tabs.filter((t) => !paths.includes(t.path));
      const active =
        s.activeTab && paths.includes(s.activeTab)
          ? (tabs[tabs.length - 1]?.path ?? null)
          : s.activeTab;
      return { tabs, activeTab: active };
    });

  const actions: RepoActions = {
    api,

    async start() {
      try {
        set({ info: await api.info() });
      } catch (e) {
        set({ error: message(e) });
      }
      await actions.refresh();
    },

    async refresh() {
      try {
        const status = await api.status();
        set((s) => applyStatus(s, status));
      } catch (e) {
        set({ error: message(e), loaded: true });
      }
    },

    setFilter: (filter) => set({ filter }),
    setView: (view) => set({ view }),
    setAutoAdvance: (autoAdvance) => set({ autoAdvance }),
    toggleSelected: (path) =>
      set((s) => ({
        selection: s.selection.includes(path)
          ? s.selection.filter((p) => p !== path)
          : [...s.selection, path],
      })),
    clearSelection: () => set({ selection: [] }),
    setSelection: (selection) => set({ selection }),

    openFile(path) {
      const state = get();
      const existing = state.tabs.find((t) => t.path === path);
      if (existing) {
        actions.focusTab(path);
        return;
      }
      const entry = entryOf(path);
      if (!entry) return;
      let tabs = state.tabs;
      if (tabs.length >= MAX_TABS) {
        // Least recently used clean tab goes; unsaved work is never dropped.
        const victim = [...tabs]
          .filter((t) => !t.dirty)
          .sort((a, b) => a.lastUsed - b.lastUsed)[0];
        if (!victim) {
          set({
            error:
              "Too many tabs have unsaved changes. Save or close one first.",
          });
          return;
        }
        tabs = tabs.filter((t) => t.path !== victim.path);
      }
      set({
        tabs: [
          ...tabs,
          {
            path,
            display: entry.display,
            dirty: false,
            notice: null,
            signature: signatureOf(entry),
            epoch: 0,
            lastUsed: ++clock,
          },
        ],
        activeTab: path,
      });
    },

    focusTab(path) {
      set((s) => ({
        activeTab: path,
        tabs: s.tabs.map((t) =>
          t.path === path ? { ...t, lastUsed: ++clock } : t,
        ),
      }));
    },

    closeTab: (path) => dropTabs([path]),

    setDirty: (path, dirty) =>
      set((s) => ({
        tabs: s.tabs.map((t) => (t.path === path ? { ...t, dirty } : t)),
      })),

    reloadTab(path) {
      const entry = entryOf(path);
      set((s) => ({
        tabs: s.tabs.map((t) =>
          t.path === path
            ? {
                ...t,
                dirty: false,
                notice: null,
                epoch: t.epoch + 1,
                signature: entry ? signatureOf(entry) : t.signature,
              }
            : t,
        ),
      }));
    },

    async fileResolved(path, method) {
      const before = sortedConflicts(get().status);
      const wasActive = get().activeTab === path;
      log([resolvedItem(path, method)]);
      await actions.refresh();
      const state = get();
      const next = state.autoAdvance
        ? nextUnresolved(
            before.filter((c) =>
              state.status?.conflicts.some((x) => x.path === c.path),
            ),
            path,
          )
        : null;
      // Stay on the saved file's slot: replace its tab with the next file (or close it).
      if (wasActive && next) {
        const open = get().tabs.find((t) => t.path === next.path);
        dropTabs([path]);
        if (open) actions.focusTab(next.path);
        else actions.openFile(next.path);
      } else {
        dropTabs([path]);
      }
    },

    logResolved(path, method) {
      log([resolvedItem(path, method)]);
    },

    async renameChosen(from, outcome) {
      if (!outcome.needsMerge) log([resolvedItem(from, "Renamed")]);
      dropTabs([from]);
      await actions.refresh();
      if (outcome.needsMerge) actions.openFile(outcome.chosen);
    },

    async acceptMany(paths, side) {
      if (paths.length === 0) return;
      const method: ResolutionMethod =
        side === "Ours" ? "Accepted Left" : "Accepted Right";
      try {
        const items = paths.map((p) => resolvedItem(p, method));
        const result = await api.acceptMany(paths, side);
        const done = new Set(result.done);
        log(items.filter((i) => done.has(i.path)));
        dropTabs(result.done);
        set((s) => ({ selection: s.selection.filter((p) => !done.has(p)) }));
        if (result.failed.length > 0) {
          set({
            error: result.failed
              .map((f) => `${entryOf(f.path)?.display ?? f.path}: ${f.message}`)
              .join("\n"),
          });
        }
      } catch (e) {
        set({ error: message(e) });
      }
      await actions.refresh();
    },

    async acceptSelected(side) {
      await actions.acceptMany(get().selection, side);
    },

    async acceptFile(path, side) {
      await actions.acceptMany([path], side);
    },

    async deleteFile(path) {
      try {
        const item = resolvedItem(path, "Deleted");
        await api.deleteFile(path);
        log([item]);
        dropTabs([path]);
        await actions.refresh();
      } catch (e) {
        set({ error: message(e) });
      }
    },

    async reopen(path) {
      try {
        await api.restore(path);
        set((s) => ({ resolved: s.resolved.filter((r) => r.path !== path) }));
        await actions.refresh();
      } catch (e) {
        set({ error: message(e) });
      }
    },

    async runOperation(kind) {
      set({ opBusy: true, opError: null, gitOutput: "" });
      try {
        const run: Promise<ControlOutcome> =
          kind === "continue"
            ? api.continueOperation()
            : kind === "abort"
              ? api.abortOperation()
              : api.skipOperation();
        const outcome = await run;
        set({ gitOutput: outcome.message });
      } catch (e) {
        set({ opError: message(e) });
      }
      set({ opBusy: false });
      await actions.refresh();
    },

    dismissError: () => set({ error: null }),
    anyDirty: () => get().tabs.some((t) => t.dirty),
  };

  return Object.assign(store, actions);
}
