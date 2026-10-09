import { EditorView } from "@codemirror/view";
import { useEffect, useMemo, useRef } from "react";
import { useStore } from "zustand";
import type { Usage, UsageSummary } from "../../../ipc/bindings";
import {
  CONFIDENCE_LABEL,
  sessionLine,
  STRATEGY_LABEL,
  usageLine,
} from "../../../ai/format";
import type { MergeEditorContext } from "../../extensions";
import { applyReplacement } from "../../model/actions";
import { chunksOf, isResolved } from "../../model/session";
import { diffRows } from "../structural/diff";
import styles from "./Ai.module.css";
import { ErrorBox, GateCard } from "./Cards";
import { PreviewDialog } from "./PreviewDialog";
import { chunkAi, type AiHost, type AiStore, type Tab } from "./store";
import { linesOf } from "./lines";
import { DiffRowView, WithCode } from "./text";

interface Props {
  ctx: MergeEditorContext;
  host: AiHost;
  store: AiStore;
}

function Footer({ model, record, session }: UsageProps) {
  return (
    <div className={styles.usage} data-testid="ai-usage">
      <span>{usageLine(model, record)}</span>
      {session ? (
        <span>
          {sessionLine(session.requests, session.totals, session.cost)}
        </span>
      ) : null}
    </div>
  );
}

interface UsageProps {
  model: string;
  record: Usage;
  session: UsageSummary | null;
}

/** The AI assistant: Explain and Suggestion for one conflict at a time. */
export function AiPanel({ ctx, host, store }: Props) {
  const panel = useStore(store, (s) => s.panel);
  const chunks = useStore(store, (s) => s.chunks);
  const gate = useStore(store, (s) => s.gate);
  const queue = useStore(store, (s) => s.queue);
  const status = useStore(store, (s) => s.status);
  const session = useStore(store, (s) => s.session);
  const preview = useStore(store, (s) => s.preview);
  const ref = useRef<HTMLElement>(null);

  const id = panel.open ? panel.chunkId : null;
  // Focus the panel when it opens or moves to another conflict, not on every update.
  useEffect(() => {
    if (id !== null) ref.current?.focus();
  }, [id]);

  const chunk = useMemo(
    () =>
      id === null
        ? null
        : (chunksOf(ctx.state).find((c) => c.id === id) ?? null),
    [ctx.state, id],
  );
  if (id === null) return null;

  const ai = chunkAi({ chunks }, id);
  const s = store.getState();
  const resolved = !chunk || isResolved(chunk);
  const model = status?.model ?? "";
  const thisGate = gate && gate.chunkId === id ? gate.failure : null;

  const startLine = chunk ? ctx.state.doc.lineAt(chunk.from).number : 0;
  const endLine = chunk
    ? Math.max(
        startLine,
        ctx.state.doc.lineAt(Math.max(chunk.from, chunk.to - 1)).number,
      )
    : 0;
  const where =
    endLine > startLine ? `lines ${startLine}–${endLine}` : `line ${startLine}`;

  const tab: Tab = panel.tab;
  const tabs: [Tab, string][] = [
    ["explain", "Explain"],
    ["suggestion", "Suggestion"],
  ];

  const apply = () => {
    const result = ai.suggest.result;
    const analysisChunk = ctx.analysis.chunks.find((c) => c.id === id);
    if (!result || !analysisChunk) return;
    const spec = applyReplacement(
      ctx.state,
      ctx.analysis,
      {
        chunkIds: [id],
        baseRange: analysisChunk.base,
        text: result.checked.suggestion.resolution,
      },
      "ai",
    );
    if (!spec) {
      s.decide(id, "skipped");
      return;
    }
    ctx.dispatch(spec);
    if (queue) s.decide(id, "applied");
    else {
      s.dismiss(id);
      s.close();
    }
  };

  const dismiss = () => {
    s.dismiss(id);
    if (queue) s.decide(id, "dismissed");
  };

  const edit = () => {
    if (chunk) {
      ctx.dispatch({
        selection: { anchor: chunk.from },
        effects: EditorView.scrollIntoView(chunk.from, { y: "center" }),
      });
      ctx.view?.focus();
    }
    s.decide(id, "skipped");
  };

  const previewLink = (task: "explain" | "suggest") => (
    <button
      type="button"
      className={styles.link}
      onClick={() => void s.openPreview(id, task)}
    >
      Preview request
    </button>
  );

  const explainBody = () => {
    const e = ai.explain;
    if (e.phase === "idle")
      return (
        <>
          <div className={styles.body}>
            <p className={styles.empty}>
              Explain what each side changed and why, where they clash, and what
              a correct merge has to keep. Nothing is changed in the file.
            </p>
          </div>
          <div className={styles.footer}>
            <button
              type="button"
              className={`${styles.btn} ${styles.primary}`}
              onClick={() => void s.explain(id)}
            >
              Explain this conflict
            </button>
            <span className={styles.grow} />
            {previewLink("explain")}
          </div>
        </>
      );
    return (
      <>
        <div className={styles.body}>
          {e.phase === "streaming" ? (
            <span className={styles.streaming} role="status">
              Explaining…
            </span>
          ) : null}
          {e.text ? (
            <p
              className={styles.prose}
              aria-live="polite"
              data-testid="ai-explanation"
            >
              <WithCode text={e.text} />
            </p>
          ) : null}
          {e.phase === "cancelled" ? (
            <span className={styles.hint} role="status">
              Cancelled.
            </span>
          ) : null}
          {e.phase === "error" && e.failure ? (
            <ErrorBox failure={e.failure} />
          ) : null}
        </div>
        <div className={styles.footer}>
          {e.phase === "streaming" ? (
            <button
              type="button"
              className={styles.btn}
              onClick={() => s.cancel(id, "explain")}
            >
              Cancel
            </button>
          ) : (
            <>
              <button
                type="button"
                className={styles.btn}
                onClick={() => void s.explain(id)}
              >
                {e.phase === "done" ? "Explain again" : "Try again"}
              </button>
              <button
                type="button"
                className={`${styles.btn} ${styles.ai}`}
                onClick={() => {
                  s.setTab("suggestion");
                  if (ai.suggest.phase === "idle") void s.suggest(id);
                }}
              >
                Suggest a resolution
              </button>
            </>
          )}
          <span className={styles.grow} />
          {previewLink("explain")}
        </div>
        {e.result ? (
          <Footer
            model={e.result.record.model}
            record={e.result.record.usage}
            session={session}
          />
        ) : null}
      </>
    );
  };

  const suggestBody = () => {
    const sg = ai.suggest;
    if (sg.phase === "idle")
      return (
        <>
          <div className={styles.body}>
            <p className={styles.empty}>
              Ask for a resolution of this conflict. You see a diff against the
              current result, a confidence level and the risks before anything
              changes.
            </p>
          </div>
          <div className={styles.footer}>
            <button
              type="button"
              className={`${styles.btn} ${styles.primary}`}
              onClick={() => void s.suggest(id)}
            >
              Suggest a resolution
            </button>
            <span className={styles.grow} />
            {previewLink("suggest")}
          </div>
        </>
      );
    if (sg.phase === "loading")
      return (
        <>
          <div className={styles.body}>
            <span className={styles.streaming} role="status">
              {queue
                ? "Waiting for the suggestion for this conflict…"
                : `Asking ${model}…`}
            </span>
          </div>
          <div className={styles.footer}>
            <button
              type="button"
              className={styles.btn}
              onClick={() =>
                queue ? s.cancelQueue() : s.cancel(id, "suggest")
              }
            >
              {queue ? "Cancel all" : "Cancel"}
            </button>
          </div>
        </>
      );
    if (sg.phase === "error" && sg.failure)
      return (
        <>
          <div className={styles.body}>
            <ErrorBox failure={sg.failure} />
          </div>
          <div className={styles.footer}>
            <button
              type="button"
              className={styles.btn}
              onClick={() => void s.suggest(id)}
            >
              Try again
            </button>
            {queue ? (
              <button
                type="button"
                className={styles.btn}
                onClick={() => s.decide(id, "skipped")}
              >
                Skip
              </button>
            ) : null}
          </div>
        </>
      );
    const result = sg.result;
    if (!result) return null;
    const { suggestion, syntaxWarning, notes } = result.checked;
    const current = chunk
      ? ctx.state.doc.sliceString(chunk.from, chunk.to)
      : "";
    const rows = diffRows(linesOf(current), linesOf(suggestion.resolution));
    return (
      <>
        <div className={styles.body}>
          <div className={styles.chips}>
            <span
              className={`${styles.conf} ${
                suggestion.confidence === "high"
                  ? styles.confHigh
                  : suggestion.confidence === "medium"
                    ? styles.confMedium
                    : styles.confLow
              }`}
            >
              {CONFIDENCE_LABEL[suggestion.confidence]}
            </span>
            <span className={styles.strategy}>
              Strategy: {STRATEGY_LABEL[suggestion.strategy]}
            </span>
            <span className={styles.where}>{where}</span>
          </div>
          <p className={styles.prose} data-testid="ai-suggestion-explanation">
            <WithCode text={suggestion.explanation} />
          </p>
          <div className={styles.diffBox}>
            <div className={styles.diffHead}>Current result → suggestion</div>
            <div
              className={styles.diff}
              aria-label="Changes the suggestion makes"
            >
              {rows.map((row, i) => (
                <DiffRowView key={i} row={row} />
              ))}
            </div>
          </div>
          {syntaxWarning ? (
            <div role="alert" className={styles.alert}>
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
                <path d="M12 3 2 21h20z" />
                <path d="M12 10v5" />
                <path d="M12 18v.5" />
              </svg>
              <span>
                <strong>{syntaxWarning}.</strong> The file has more syntax
                errors with this applied than any of base, left or right.
              </span>
            </div>
          ) : null}
          {notes.map((n) => (
            <div key={n} role="status" className={styles.alert}>
              <span>{n}</span>
            </div>
          ))}
          {suggestion.risks.length > 0 ? (
            <div>
              <h3 className={styles.risksTitle}>Risks</h3>
              <ul className={styles.risks}>
                {suggestion.risks.map((r) => (
                  <li key={r}>
                    <WithCode text={r} />
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {result.notes.length > 0 ? (
            <span className={styles.hint}>{result.notes.join(" · ")}</span>
          ) : null}
        </div>
        <div className={styles.footer}>
          <button
            type="button"
            className={`${styles.btn} ${styles.primary}`}
            onClick={apply}
            disabled={resolved}
          >
            Apply
          </button>
          <button type="button" className={styles.btn} onClick={dismiss}>
            Dismiss
          </button>
          {queue ? (
            <button type="button" className={styles.btn} onClick={edit}>
              Edit
            </button>
          ) : null}
          <button
            type="button"
            className={styles.btn}
            onClick={() => void s.suggest(id)}
          >
            Regenerate
          </button>
          <span className={styles.grow} />
          {previewLink("suggest")}
        </div>
        <Footer
          model={result.record.model}
          record={result.record.usage}
          session={session}
        />
      </>
    );
  };

  return (
    <aside
      ref={ref}
      className={styles.panel}
      aria-label="AI assistant"
      tabIndex={-1}
      data-testid="ai-panel"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !preview) {
          e.stopPropagation();
          s.close();
        }
      }}
    >
      <div className={styles.tabs} role="tablist" aria-label="AI assistant">
        {tabs.map(([t, label]) => (
          <button
            key={t}
            type="button"
            role="tab"
            id={`ai-tab-${t}`}
            aria-selected={tab === t}
            aria-controls="ai-tabpanel"
            className={styles.tab}
            onClick={() => s.setTab(t)}
          >
            {label}
          </button>
        ))}
        <span className={styles.tabSpacer} />
        {queue ? (
          <span className={styles.counter} data-testid="ai-queue-counter">
            {queue.review + 1} of {queue.ids.length}
          </span>
        ) : null}
        <button
          type="button"
          className={styles.iconBtn}
          aria-label="Close AI assistant"
          onClick={() => {
            if (queue) s.cancelQueue();
            s.close();
          }}
        >
          ×
        </button>
      </div>
      <div
        id="ai-tabpanel"
        role="tabpanel"
        aria-labelledby={`ai-tab-${tab}`}
        style={{
          display: "flex",
          flexDirection: "column",
          flex: 1,
          minHeight: 0,
        }}
      >
        {resolved && !queue ? (
          <div className={styles.body}>
            <p className={styles.empty} role="status">
              This conflict is already resolved.
            </p>
          </div>
        ) : thisGate ? (
          <div className={styles.body}>
            <GateCard
              failure={thisGate}
              status={status}
              host={host}
              store={store}
            />
          </div>
        ) : tab === "explain" ? (
          explainBody()
        ) : (
          suggestBody()
        )}
      </div>
      {preview ? (
        <PreviewDialog preview={preview} onClose={s.closePreview} />
      ) : null}
    </aside>
  );
}
