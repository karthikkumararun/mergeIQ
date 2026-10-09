import { useEffect, useMemo, useRef } from "react";
import { useStore } from "zustand";
import { tokens } from "../../../ai/format";
import type { MergeEditorContext } from "../../extensions";
import { setAiActive, setAiMarks } from "./gutter";
import { buildInput, unresolvedConflicts } from "./input";
import styles from "./Ai.module.css";
import { chunkAi, type AiHost, type AiStore } from "./store";

interface Props {
  ctx: MergeEditorContext;
  host: AiHost;
  store: AiStore;
}

function Sparkles() {
  return (
    <svg
      width="15"
      height="15"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z" />
      <path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z" />
    </svg>
  );
}

/**
 * "AI: suggest remaining (N)" with the estimated input tokens, or a "Set up AI" link when no
 * provider is configured. Also binds the store to the editor, loads the status, and keeps the
 * Result gutter markers and the active-conflict outline in sync.
 */
export function AiToolbar({ ctx, host, store }: Props) {
  const { doc, state, view } = ctx;
  const status = useStore(store, (s) => s.status);
  const estimate = useStore(store, (s) => s.estimate);
  const queue = useStore(store, (s) => s.queue);
  const chunks = useStore(store, (s) => s.chunks);
  const panel = useStore(store, (s) => s.panel);

  // Latest editor context, read by the store when a request is built.
  const latest = useRef(ctx);
  useEffect(() => {
    latest.current = ctx;
  });
  useEffect(() => {
    const read = () => latest.current;
    store.getState().bind({
      path: doc.displayPath,
      inputFor: (id) => {
        const c = read();
        return buildInput(c.doc, c.analysis, c.state, id);
      },
      spansFor: (id) => {
        const c = read();
        return buildInput(c.doc, c.analysis, c.state, id)?.chunk ?? null;
      },
    });
    void store.getState().refreshStatus();
    void store.getState().refreshSession();
  }, [store, doc.displayPath]);

  const ids = useMemo(() => unresolvedConflicts(state), [state]);
  const idsKey = ids.join(",");
  const blocked = status?.blocked?.code ?? null;
  const hidden = status === null || blocked === "notConfigured";

  // The estimate follows the set of unresolved conflicts.
  useEffect(() => {
    if (hidden || blocked === "excluded" || queue) return;
    const timer = setTimeout(
      () => void store.getState().refreshEstimate(ids),
      250,
    );
    return () => clearTimeout(timer);
    // `ids` is derived from `idsKey`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [store, idsKey, hidden, blocked, queue]);

  const marks = hidden ? [] : ids;
  const marksKey = marks.join(",");
  useEffect(() => {
    view?.dispatch({
      effects: setAiMarks.of(marksKey ? marksKey.split(",").map(Number) : []),
    });
  }, [view, marksKey]);

  const activeId = panel.open ? panel.chunkId : null;
  useEffect(() => {
    view?.dispatch({ effects: setAiActive.of(activeId) });
  }, [view, activeId]);

  // How many of the run's suggestions have arrived, and how many were decided.
  const waiting = queue
    ? queue.ids.filter(
        (id) =>
          !(id in queue.decided) &&
          chunkAi({ chunks }, id).suggest.phase === "loading",
      ).length
    : 0;
  const arrived = queue ? queue.ids.length - waiting : 0;
  const decided = queue ? Object.keys(queue.decided).length : 0;

  if (status === null) return null;
  if (blocked === "notConfigured")
    return (
      <button
        type="button"
        className={styles.link}
        onClick={() => void host.api.openSettings("ai")}
      >
        Set up AI
      </button>
    );

  // Nothing to suggest for: no button (the conflicts are all resolved, or there never were any).
  if (ids.length === 0 && !queue) return null;

  const disabled = blocked === "excluded" || !!queue;
  return (
    <>
      <span className={styles.sep} aria-hidden="true" />
      <button
        type="button"
        className={styles.aiBtn}
        disabled={disabled}
        title={
          blocked === "excluded"
            ? "File excluded from AI by your settings"
            : undefined
        }
        onClick={() => void store.getState().startQueue(ids)}
      >
        <Sparkles />
        {queue
          ? waiting > 0
            ? `AI: suggesting… ${arrived} of ${queue.ids.length}`
            : `AI: reviewing ${decided} of ${queue.ids.length} done`
          : `AI: suggest remaining (${ids.length})`}
      </button>
      {estimate && ids.length > 0 && !queue ? (
        <span className={styles.hint} data-testid="ai-estimate">
          ≈ {tokens(estimate.totalTokens)} input tokens
        </span>
      ) : null}
      {blocked === "excluded" ? (
        <span className={styles.hint}>
          File excluded from AI by your settings
        </span>
      ) : null}
    </>
  );
}
