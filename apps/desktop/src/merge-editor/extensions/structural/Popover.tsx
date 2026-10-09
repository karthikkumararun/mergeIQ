import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { useStore } from "zustand";
import type { MergeEditorContext } from "../../extensions";
import { rangeSpan } from "../../gutters/geometry";
import { useViewTick } from "../../gutters/useViewTick";
import { applyReplacement, replacedText } from "../../model/actions";
import { chunksOf } from "../../model/session";
import { normalizeEol } from "../../model/text";
import { diffRows, type DiffRow } from "./diff";
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

/** Splits `text` on backticks into plain text and `<code>` runs. */
function withCode(text: string): ReactNode[] {
  return text
    .split(/`([^`]+)`/)
    .map((part, i) => (i % 2 === 1 ? <code key={i}>{part}</code> : part));
}

function lines(text: string): string[] {
  const t = normalizeEol(text);
  const parts = t.split("\n");
  if (parts[parts.length - 1] === "") parts.pop();
  return parts;
}

function Row({ row }: { row: DiffRow }) {
  const [from, to] = row.emphasis ?? [0, 0];
  const body =
    row.emphasis && to > from ? (
      <>
        {row.text.slice(0, from)}
        <span className={styles.em}>{row.text.slice(from, to)}</span>
        {row.text.slice(to)}
      </>
    ) : (
      row.text
    );
  return (
    <div
      className={`${styles.row} ${row.kind === "del" ? styles.del : row.kind === "add" ? styles.add : ""}`}
    >
      <span className={styles.mark} aria-hidden="true">
        {row.kind === "del" ? "−" : row.kind === "add" ? "+" : ""}
      </span>
      <span>
        {row.kind === "del" ? (
          <span className={styles.sr}>Removed: </span>
        ) : null}
        {row.kind === "add" ? <span className={styles.sr}>Added: </span> : null}
        {body}
      </span>
    </div>
  );
}

/** The preview of one structural proposal, anchored to its first chunk. */
export function StructuralPopover({ ctx, store }: Props) {
  const { analysis, doc, state, view, dispatch } = ctx;
  const openIndex = useStore(store, (s) => s.openIndex);
  const proposals = useStore(store, (s) => s.proposals);
  const dismissed = useStore(store, (s) => s.dismissed);
  const ref = useRef<HTMLDivElement>(null);
  useViewTick([view]);

  const offered = useMemo(
    () =>
      openIndex === null
        ? null
        : (applicableProposals({ proposals, dismissed }, chunksOf(state)).find(
            (a) => a.index === openIndex,
          ) ?? null),
    [openIndex, proposals, dismissed, state],
  );

  const current = useMemo(
    () =>
      offered
        ? replacedText(state, analysis, toReplacement(offered.proposal))
        : null,
    [offered, state, analysis],
  );
  const rows = useMemo(
    () =>
      offered && current !== null
        ? diffRows(lines(current), lines(offered.proposal.text))
        : [],
    [offered, current],
  );

  // Focus the preview when it opens, not on every later state change (that would steal focus
  // from whatever the user tabbed to inside it).
  const isOpen = offered !== null;
  useEffect(() => {
    if (isOpen) ref.current?.focus();
  }, [isOpen, openIndex]);

  // A proposal that stopped being offered (applied elsewhere, dismissed, re-analysed) closes.
  useEffect(() => {
    if (openIndex !== null && !offered) store.getState().close();
  }, [openIndex, offered, store]);

  if (!offered || current === null) return null;
  const { proposal, index } = offered;
  const first = chunksOf(state).find((c) => c.id === proposal.chunk_ids[0]);
  const top = Math.max(
    8,
    view && first ? rangeSpan(view, first.from, first.from).top : 8,
  );
  const fileName = doc.displayPath.split(/[\\/]/).pop() ?? doc.displayPath;
  const covered = analysis.chunks.filter((c) =>
    proposal.chunk_ids.includes(c.id),
  );
  const conflicts = covered.filter((c) => c.kind === "Conflict").length;
  const others = covered.length - conflicts;

  const apply = () => {
    const spec = applyReplacement(
      state,
      analysis,
      toReplacement(proposal),
      "structural",
    );
    store.getState().close();
    if (spec) dispatch(spec);
  };

  return (
    <div className={styles.overlay}>
      <div
        ref={ref}
        className={styles.popover}
        role="dialog"
        aria-labelledby="structural-title"
        tabIndex={-1}
        style={{ top, maxHeight: `calc(100% - ${top}px - 8px)` }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            store.getState().close();
          }
        }}
      >
        <div className={styles.head}>
          <h2 id="structural-title" className={styles.title}>
            Structural proposal ·{" "}
            <span className={styles.mono}>
              {proposal.container || fileName}
            </span>
          </h2>
          <span className={styles.explain}>
            {withCode(proposal.explanation)}
          </span>
        </div>
        <div className={styles.diff} aria-label="Changes the proposal makes">
          {rows.map((row, i) => (
            <Row key={i} row={row} />
          ))}
        </div>
        <div className={styles.facts}>
          <span>
            Covers {conflicts} {conflicts === 1 ? "conflict" : "conflicts"}
            {others > 0
              ? ` and ${others} other ${others === 1 ? "change" : "changes"}`
              : ""}
          </span>
          <span>Validated: parses without errors</span>
        </div>
        <div className={styles.actions}>
          <button
            type="button"
            className={styles.btn}
            onClick={() => store.getState().dismiss(index)}
          >
            Dismiss
          </button>
          <button
            type="button"
            className={`${styles.btn} ${styles.primary}`}
            onClick={apply}
          >
            Apply proposal
          </button>
        </div>
      </div>
    </div>
  );
}
