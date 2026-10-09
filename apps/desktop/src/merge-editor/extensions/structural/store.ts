import { createStore, type StoreApi } from "zustand/vanilla";
import type { Analysis, Proposal } from "../../../ipc/bindings";
import type { Replacement } from "../../model/actions";
import { isResolved } from "../../model/session";
import type { ChunkState } from "../../model/types";
import { resolveStructural, type ResolveStructural } from "./api";

/** Where the background computation stands. */
export type Phase =
  "idle" | "running" | "ready" | "unsupported" | "timeout" | "error";

export interface StructuralState {
  phase: Phase;
  proposals: Proposal[];
  /** Backend time for the last computation. */
  elapsedMs: number | null;
  /** Indices (into `proposals`) the user dismissed. */
  dismissed: readonly number[];
  /** The proposal whose preview is open. */
  openIndex: number | null;
  compute: (path: string, analysis: Analysis) => Promise<void>;
  open: (index: number) => void;
  close: () => void;
  dismiss: (index: number) => void;
}

export type StructuralStore = StoreApi<StructuralState>;

/** One store per merge editor: proposals belong to the analysis they were computed for. */
export function createStructuralStore(
  resolve: ResolveStructural = resolveStructural,
): StructuralStore {
  let token = 0;
  return createStore<StructuralState>((set, get) => ({
    phase: "idle",
    proposals: [],
    elapsedMs: null,
    dismissed: [],
    openIndex: null,
    async compute(path, analysis) {
      const mine = ++token;
      set({
        phase: "running",
        proposals: [],
        elapsedMs: null,
        dismissed: [],
        openIndex: null,
      });
      try {
        const { outcome, elapsed_ms } = await resolve(path, analysis);
        if (mine !== token) return;
        if (outcome === "Unsupported") set({ phase: "unsupported" });
        else if (outcome === "TimedOut") set({ phase: "timeout" });
        else
          set({
            phase: "ready",
            proposals: outcome.Proposals,
            elapsedMs: elapsed_ms,
          });
      } catch {
        // Proposals are an optional convenience: a failure just means none are shown.
        if (mine === token) set({ phase: "error" });
      }
    },
    open: (index) => set({ openIndex: index }),
    close: () => set({ openIndex: null }),
    dismiss: (index) =>
      set({
        dismissed: [...get().dismissed, index],
        openIndex: get().openIndex === index ? null : get().openIndex,
      }),
  }));
}

export interface Applicable {
  index: number;
  proposal: Proposal;
}

/** Proposals that are still offered: not dismissed and every covered chunk unresolved. */
export function applicableProposals(
  s: Pick<StructuralState, "proposals" | "dismissed">,
  chunks: readonly ChunkState[],
): Applicable[] {
  const byId = new Map(chunks.map((c) => [c.id, c]));
  const out: Applicable[] = [];
  s.proposals.forEach((proposal, index) => {
    if (s.dismissed.includes(index)) return;
    const covered = proposal.chunk_ids.map((id) => byId.get(id));
    if (covered.length > 0 && covered.every((c) => c && !isResolved(c)))
      out.push({ index, proposal });
  });
  return out;
}

export function toReplacement(p: Proposal): Replacement {
  return {
    chunkIds: p.chunk_ids,
    baseRange: p.base_range,
    text: p.text,
  };
}
