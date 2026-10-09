import { useEffect, useMemo } from "react";
import { useStore } from "zustand";
import type { MergeEditorContext } from "../../extensions";
import { applyReplacements } from "../../model/actions";
import { chunksOf } from "../../model/session";
import { setMarks, type Mark } from "./gutter";
import {
  applicableProposals,
  toReplacement,
  type StructuralStore,
} from "./store";
import styles from "./Structural.module.css";

interface Props {
  ctx: MergeEditorContext;
  store: StructuralStore;
}

/**
 * "Resolve structurally (N)" plus a status line. Also starts the background computation when
 * the editor opens (or is re-analysed) and keeps the Result gutter markers in sync.
 */
export function StructuralToolbar({ ctx, store }: Props) {
  const { analysis, doc, state, view, dispatch } = ctx;
  const phase = useStore(store, (s) => s.phase);
  const proposals = useStore(store, (s) => s.proposals);
  const dismissed = useStore(store, (s) => s.dismissed);
  const openIndex = useStore(store, (s) => s.openIndex);
  const elapsedMs = useStore(store, (s) => s.elapsedMs);

  useEffect(() => {
    void store.getState().compute(doc.displayPath, analysis);
  }, [store, doc.displayPath, analysis]);

  const applicable = useMemo(
    () => applicableProposals({ proposals, dismissed }, chunksOf(state)),
    [proposals, dismissed, state],
  );

  const marks: Mark[] = applicable.flatMap(({ index, proposal }) =>
    proposal.chunk_ids.map((chunkId) => ({
      chunkId,
      proposal: index,
      open: openIndex === index,
    })),
  );
  const marksKey = JSON.stringify(marks);
  useEffect(() => {
    view?.dispatch({ effects: setMarks.of(JSON.parse(marksKey) as Mark[]) });
  }, [view, marksKey]);

  const resolveAll = () => {
    const spec = applyReplacements(
      state,
      analysis,
      applicable.map(({ proposal }) => toReplacement(proposal)),
      "structural",
    );
    store.getState().close();
    if (spec) dispatch(spec);
  };

  return (
    <>
      {applicable.length > 0 ? (
        <button type="button" className={styles.bulk} onClick={resolveAll}>
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <rect x="3" y="3" width="7" height="7" rx="1" />
            <rect x="14" y="3" width="7" height="7" rx="1" />
            <rect x="8.5" y="14" width="7" height="7" rx="1" />
            <path d="M6.5 10v2h11v-2M12 12v2" />
          </svg>
          Resolve structurally ({applicable.length})
        </button>
      ) : null}
      <span
        className={styles.status}
        role="status"
        data-testid="structural-status"
      >
        {phase === "running"
          ? "Finding structural merges…"
          : phase === "ready" && applicable.length > 0 && elapsedMs !== null
            ? `Proposals ready · computed in ${elapsedMs} ms`
            : ""}
      </span>
    </>
  );
}
