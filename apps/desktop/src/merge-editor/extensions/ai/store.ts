import { createStore, type StoreApi } from "zustand/vanilla";
import {
  failureOf,
  newRequestId,
  type AiApi,
  type AiEstimate,
  type AiExplained,
  type AiFailure,
  type AiPreview,
  type AiStatus,
  type AiSuggested,
  type AiTask,
  type ChunkSpans,
  type ContextInput,
  type RepoDecision,
} from "../../../ai/api";
import type { UsageSummary } from "../../../ipc/bindings";

/** Where the AI backend lives for this editor. */
export interface AiHost {
  api: AiApi;
  /** The open repository, or `null` for a standalone merge window. */
  repo: number | null;
  /** The key the opt-in decision is stored under (the repository root). */
  scope: string;
  /** What the opt-in prompt calls it. */
  scopeName: string;
}

export type Tab = "explain" | "suggestion";

/** Failures that mean "the request was not even attempted": the panel shows a card instead. */
export type GateCode =
  | "notConfigured"
  | "noticeRequired"
  | "repoUnasked"
  | "repoDeclined"
  | "excluded";

const GATES: readonly string[] = [
  "notConfigured",
  "noticeRequired",
  "repoUnasked",
  "repoDeclined",
  "excluded",
];

export function isGate(f: AiFailure): boolean {
  return GATES.includes(f.code);
}

export interface ExplainState {
  phase: "idle" | "streaming" | "done" | "cancelled" | "error";
  text: string;
  failure: AiFailure | null;
  requestId: string | null;
  result: AiExplained | null;
}

export interface SuggestState {
  phase: "idle" | "loading" | "ready" | "error";
  result: AiSuggested | null;
  failure: AiFailure | null;
  requestId: string | null;
}

export interface ChunkAi {
  explain: ExplainState;
  suggest: SuggestState;
}

export type Decision = "applied" | "dismissed" | "skipped";

export interface QueueState {
  /** Conflicts to suggest for, in document order. */
  ids: number[];
  /** Index into `ids` of the suggestion being reviewed. */
  review: number;
  decided: Record<number, Decision>;
}

export interface PreviewState {
  phase: "loading" | "ready" | "error";
  task: AiTask;
  data: AiPreview | null;
  failure: AiFailure | null;
}

export type Action = "explain" | "suggest";

/** The editor-side pieces the store needs but must not hold in state. */
export interface Env {
  path: string;
  /** Builds a request for a chunk from the editor's current state. */
  inputFor: (chunkId: number) => ContextInput | null;
  /** Spans for the estimate. */
  spansFor: (chunkId: number) => ChunkSpans | null;
}

export interface AiState {
  status: AiStatus | null;
  statusError: string | null;
  session: UsageSummary | null;
  estimate: AiEstimate | null;
  panel: { open: boolean; chunkId: number | null; tab: Tab };
  chunks: Record<number, ChunkAi>;
  /** Why the current action was not attempted, with the action to resume. */
  gate: { failure: AiFailure; chunkId: number; action: Action } | null;
  queue: QueueState | null;
  preview: PreviewState | null;

  bind: (env: Env) => void;
  refreshStatus: () => Promise<void>;
  refreshSession: () => Promise<void>;
  refreshEstimate: (ids: number[]) => Promise<void>;
  open: (chunkId: number, tab?: Tab) => void;
  close: () => void;
  setTab: (tab: Tab) => void;
  explain: (chunkId: number) => Promise<void>;
  suggest: (chunkId: number) => Promise<boolean>;
  cancel: (chunkId: number, which: "explain" | "suggest") => void;
  dismiss: (chunkId: number) => void;
  acceptNotice: () => Promise<void>;
  decideRepo: (decision: RepoDecision) => Promise<void>;
  openPreview: (chunkId: number, task: AiTask) => Promise<void>;
  closePreview: () => void;
  startQueue: (ids: number[]) => Promise<void>;
  cancelQueue: () => void;
  decide: (chunkId: number, decision: Decision) => void;
  select: (chunkId: number) => void;
}

export type AiStore = StoreApi<AiState>;

const IDLE_EXPLAIN: ExplainState = {
  phase: "idle",
  text: "",
  failure: null,
  requestId: null,
  result: null,
};
const IDLE_SUGGEST: SuggestState = {
  phase: "idle",
  result: null,
  failure: null,
  requestId: null,
};

export const CONCURRENCY = 3;

export function chunkAi(s: Pick<AiState, "chunks">, id: number): ChunkAi {
  return s.chunks[id] ?? { explain: IDLE_EXPLAIN, suggest: IDLE_SUGGEST };
}

/** One store per merge editor. */
export function createAiStore(host: AiHost): AiStore {
  let env: Env | null = null;
  // Requests whose results are no longer wanted (cancelled or superseded).
  const live = new Set<string>();

  return createStore<AiState>((set, get) => {
    const patchExplain = (id: number, patch: Partial<ExplainState>) =>
      set((s) => {
        const c = chunkAi(s, id);
        return {
          chunks: {
            ...s.chunks,
            [id]: { ...c, explain: { ...c.explain, ...patch } },
          },
        };
      });
    const patchSuggest = (id: number, patch: Partial<SuggestState>) =>
      set((s) => {
        const c = chunkAi(s, id);
        return {
          chunks: {
            ...s.chunks,
            [id]: { ...c, suggest: { ...c.suggest, ...patch } },
          },
        };
      });

    const withGate = (
      failure: AiFailure,
      chunkId: number,
      action: Action,
    ): void => {
      set({ gate: { failure, chunkId, action } });
      // The status shows which gate it was; keep it current.
      void get().refreshStatus();
    };

    const resume = (): void => {
      const gate = get().gate;
      if (!gate) return;
      set({ gate: null });
      if (gate.action === "explain") void get().explain(gate.chunkId);
      else void get().suggest(gate.chunkId);
    };

    return {
      status: null,
      statusError: null,
      session: null,
      estimate: null,
      panel: { open: false, chunkId: null, tab: "suggestion" },
      chunks: {},
      gate: null,
      queue: null,
      preview: null,

      bind(e) {
        env = e;
      },

      async refreshStatus() {
        if (!env) return;
        try {
          const status = await host.api.status(host.repo, env.path);
          set({ status, statusError: null });
        } catch (e) {
          set({ statusError: failureOf(e).code });
        }
      },

      async refreshSession() {
        try {
          const usage = await host.api.usage();
          set({
            session: {
              requests: usage.requests,
              totals: usage.totals,
              cost: usage.cost,
            },
          });
        } catch {
          // The footer just stays empty.
        }
      },

      async refreshEstimate(ids) {
        if (!env || ids.length === 0) {
          set({ estimate: null });
          return;
        }
        const first = env.inputFor(ids[0]);
        const spans = ids
          .map((id) => env?.spansFor(id))
          .filter((s): s is ChunkSpans => !!s);
        if (!first || spans.length === 0) return;
        try {
          const estimate = await host.api.estimate(host.repo, first, spans);
          set({ estimate });
        } catch {
          set({ estimate: null });
        }
      },

      open(chunkId, tab) {
        set((s) => ({
          panel: { open: true, chunkId, tab: tab ?? s.panel.tab },
          gate: s.gate && s.gate.chunkId === chunkId ? s.gate : null,
        }));
      },

      close() {
        set((s) => ({ panel: { ...s.panel, open: false }, preview: null }));
      },

      setTab(tab) {
        set((s) => ({ panel: { ...s.panel, tab } }));
      },

      async explain(chunkId) {
        const input = env?.inputFor(chunkId);
        if (!input) return;
        const requestId = newRequestId();
        live.add(requestId);
        set({ gate: null });
        patchExplain(chunkId, {
          phase: "streaming",
          text: "",
          failure: null,
          requestId,
          result: null,
        });
        try {
          const done = await host.api.explain(
            host.repo,
            requestId,
            input,
            (text) => {
              if (live.has(requestId))
                patchExplain(chunkId, {
                  text: chunkAi(get(), chunkId).explain.text + text,
                });
            },
          );
          if (!live.has(requestId)) return;
          patchExplain(chunkId, {
            phase: "done",
            text: done.text,
            result: done,
          });
          set({ session: done.session });
        } catch (e) {
          if (!live.has(requestId)) return;
          const failure = failureOf(e);
          if (failure.code === "cancelled") {
            patchExplain(chunkId, { phase: "cancelled" });
          } else if (isGate(failure)) {
            patchExplain(chunkId, { ...IDLE_EXPLAIN });
            withGate(failure, chunkId, "explain");
          } else {
            patchExplain(chunkId, { phase: "error", failure });
          }
        } finally {
          live.delete(requestId);
        }
      },

      async suggest(chunkId) {
        const input = env?.inputFor(chunkId);
        if (!input) return false;
        const requestId = newRequestId();
        live.add(requestId);
        set({ gate: null });
        patchSuggest(chunkId, {
          phase: "loading",
          result: null,
          failure: null,
          requestId,
        });
        try {
          const result = await host.api.suggest(host.repo, requestId, input);
          if (!live.has(requestId)) return false;
          patchSuggest(chunkId, { phase: "ready", result });
          set({ session: result.session });
          return true;
        } catch (e) {
          if (!live.has(requestId)) return false;
          const failure = failureOf(e);
          if (failure.code === "cancelled") {
            patchSuggest(chunkId, { ...IDLE_SUGGEST });
          } else if (isGate(failure)) {
            patchSuggest(chunkId, { ...IDLE_SUGGEST });
            withGate(failure, chunkId, "suggest");
          } else {
            patchSuggest(chunkId, { phase: "error", failure });
          }
          return false;
        } finally {
          live.delete(requestId);
        }
      },

      cancel(chunkId, which) {
        const c = chunkAi(get(), chunkId);
        const id =
          which === "explain" ? c.explain.requestId : c.suggest.requestId;
        if (!id || !live.has(id)) return;
        // Forget the request first so late results are ignored, then stop the backend.
        live.delete(id);
        void host.api.cancel(id);
        if (which === "explain") patchExplain(chunkId, { phase: "cancelled" });
        else patchSuggest(chunkId, { ...IDLE_SUGGEST });
      },

      dismiss(chunkId) {
        patchSuggest(chunkId, { ...IDLE_SUGGEST });
      },

      async acceptNotice() {
        const settings = await host.api.getSettings();
        await host.api.updateSettings({ ...settings, noticeAccepted: true });
        await get().refreshStatus();
        resume();
      },

      async decideRepo(decision) {
        await host.api.setRepoDecision(host.scope, decision);
        await get().refreshStatus();
        if (decision === "Allowed") resume();
        else {
          const gate = get().gate;
          if (gate)
            set({ gate: { ...gate, failure: { code: "repoDeclined" } } });
        }
      },

      async openPreview(chunkId, task) {
        const input = env?.inputFor(chunkId);
        if (!input) return;
        set({ preview: { phase: "loading", task, data: null, failure: null } });
        try {
          const data = await host.api.preview(host.repo, input, task);
          set({ preview: { phase: "ready", task, data, failure: null } });
        } catch (e) {
          set({
            preview: {
              phase: "error",
              task,
              data: null,
              failure: failureOf(e),
            },
          });
        }
      },

      closePreview() {
        set({ preview: null });
      },

      async startQueue(ids) {
        if (ids.length === 0 || get().queue) return;
        set({
          queue: { ids, review: 0, decided: {} },
          panel: { open: true, chunkId: ids[0], tab: "suggestion" },
          gate: null,
        });
        let next = 0;
        let stopped = false;
        const worker = async () => {
          while (!stopped) {
            const q = get().queue;
            if (!q) return;
            const index = next++;
            if (index >= ids.length) return;
            await get().suggest(ids[index]);
            // A gate (or a vanished queue) ends the whole run.
            if (get().gate || !get().queue) stopped = true;
          }
        };
        await Promise.all(
          Array.from({ length: Math.min(CONCURRENCY, ids.length) }, worker),
        );
        if (get().gate) set({ queue: null });
      },

      cancelQueue() {
        const q = get().queue;
        if (!q) return;
        for (const id of q.ids) {
          const c = chunkAi(get(), id);
          if (c.suggest.phase === "loading") get().cancel(id, "suggest");
        }
        set({ queue: null });
      },

      decide(chunkId, decision) {
        const q = get().queue;
        if (!q) {
          if (decision !== "applied") get().dismiss(chunkId);
          return;
        }
        const decided = { ...q.decided, [chunkId]: decision };
        const undecided = q.ids.filter((id) => !(id in decided));
        if (undecided.length === 0) {
          set({
            queue: null,
            panel: { open: false, chunkId: null, tab: "suggestion" },
          });
          return;
        }
        // Next in document order after the one just decided, else the first left.
        const after = q.ids.findIndex((id) => id === chunkId);
        const nextId =
          q.ids.slice(after + 1).find((id) => !(id in decided)) ?? undecided[0];
        set({
          queue: { ...q, decided, review: q.ids.indexOf(nextId) },
          panel: { open: true, chunkId: nextId, tab: "suggestion" },
        });
      },

      select(chunkId) {
        const q = get().queue;
        const index = q ? q.ids.indexOf(chunkId) : -1;
        set((s) => ({
          panel: { open: true, chunkId, tab: s.panel.tab },
          queue: q && index >= 0 ? { ...q, review: index } : q,
        }));
      },
    };
  });
}
