import type { EditorView } from "@codemirror/view";
import { useRef, type ReactNode } from "react";
import type { ChunkState, Side } from "../model/types";
import { canActOn, canRevert, isAppend } from "../model/actions";
import { isResolved } from "../model/session";
import type { PaneChunk } from "../panes/paneChunks";
import { GUTTER_WIDTH, bandPath, rangeSpan } from "./geometry";
import { useViewTick } from "./useViewTick";
import styles from "./Connector.module.css";
import type { EditorState } from "@codemirror/state";

interface Props {
  side: Side;
  sideView: EditorView | null;
  resultView: EditorView | null;
  /** Descriptors for the side pane (changed chunks only). */
  sideChunks: readonly PaneChunk[];
  chunkStates: readonly ChunkState[];
  resultState: EditorState | null;
  /** Bumped by the host on every Result update so geometry is recomputed after edits. */
  version: number;
  currentId: number | null;
  onApply: (id: number, side: Side) => void;
  onIgnore: (id: number, side: Side) => void;
  onRevert: (id: number) => void;
  /** Extra per-chunk controls contributed by extensions. */
  extra?: (id: number) => ReactNode;
}

const bandVar = (t: string) => (t === "del" ? "res" : t);

/** One 48px gutter: Bézier bands between a side pane and the Result, plus action buttons. */
export function Connector({
  side,
  sideView,
  resultView,
  sideChunks,
  chunkStates,
  resultState,
  currentId,
  onApply,
  onIgnore,
  onRevert,
  version,
  extra,
}: Props) {
  const box = useRef<HTMLDivElement>(null);
  useViewTick([sideView, resultView]);
  void version;

  const height = sideView?.scrollDOM.clientHeight ?? 0;
  const sideX = side === "left" ? 0 : GUTTER_WIDTH;
  const resultX = side === "left" ? GUTTER_WIDTH : 0;
  const stateById = new Map(chunkStates.map((c) => [c.id, c]));
  const sideLabel = side === "left" ? "left" : "right";
  const glyphs =
    side === "left"
      ? { apply: ">>", append: "+>" }
      : { apply: "<<", append: "<+" };

  const items =
    sideView && resultView && resultState
      ? sideChunks.flatMap((pc) => {
          const cs = stateById.get(pc.id);
          if (!cs) return [];
          const s = rangeSpan(sideView, pc.from, pc.to);
          const r = rangeSpan(resultView, cs.from, cs.to);
          if (
            Math.max(s.bottom, r.bottom) < -80 ||
            Math.min(s.top, r.top) > height + 80
          )
            return [];
          return [{ pc, cs, s, r }];
        })
      : [];

  return (
    <div
      ref={box}
      className={styles.gutter}
      data-gutter={sideLabel}
      style={{ width: GUTTER_WIDTH }}
    >
      <svg
        className={styles.svg}
        width={GUTTER_WIDTH}
        height={height}
        aria-hidden="true"
      >
        {items.map(({ pc, s, r }) => {
          const t = bandVar(pc.type);
          const d = bandPath(s, r, sideX, resultX);
          return (
            <g key={pc.id} data-band={pc.id} data-type={pc.type}>
              <path d={d.fill} fill={`var(--${t}-band)`} />
              <path
                d={d.top}
                className={styles.edge}
                stroke={`var(--${t}-fg)`}
              />
              <path
                d={d.bottom}
                className={styles.edge}
                stroke={`var(--${t}-fg)`}
              />
            </g>
          );
        })}
      </svg>
      {items.map(({ pc, cs, s }) => {
        const status = side === "left" ? cs.leftStatus : cs.rightStatus;
        const active = resultState ? canActOn(resultState, pc.id, side) : false;
        const append = resultState ? isAppend(resultState, pc.id, side) : false;
        const showRevert =
          resultState && canRevert(cs) && revertHome(cs) === side;
        const done =
          status === "applied" ||
          status === "ignored" ||
          (isResolved(cs) && status === "pending");
        const top = s.top + 1;
        const verb = append ? "Append" : "Apply";
        return (
          <div
            key={pc.id}
            className={styles.actions}
            data-chunk={pc.id}
            data-current={currentId === pc.id || undefined}
            style={{ top, [side === "left" ? "left" : "right"]: 0 }}
          >
            {active ? (
              <>
                <button
                  type="button"
                  className={styles.gb}
                  aria-label={`${verb} ${sideLabel} change to result (Mod+Alt+${side === "left" ? "Left" : "Right"})`}
                  data-action={append ? "append" : "apply"}
                  onClick={() => onApply(pc.id, side)}
                >
                  {append ? glyphs.append : glyphs.apply}
                </button>
                <button
                  type="button"
                  className={styles.gb}
                  aria-label={`Ignore ${sideLabel} change`}
                  data-action="ignore"
                  onClick={() => onIgnore(pc.id, side)}
                >
                  ×
                </button>
              </>
            ) : done ? (
              <span
                className={styles.done}
                role="img"
                aria-label={status === "ignored" ? "Ignored" : "Applied"}
                data-status={status}
              >
                {status === "ignored" ? "∅" : "✓"}
              </span>
            ) : null}
            {showRevert ? (
              <button
                type="button"
                className={`${styles.gb} ${styles.revert}`}
                aria-label="Revert chunk to base"
                title="Revert"
                data-action="revert"
                onClick={() => onRevert(pc.id)}
              >
                ↺
              </button>
            ) : null}
            {extra?.(pc.id)}
          </div>
        );
      })}
    </div>
  );
}

/** Revert lives in the gutter of the first side that changed the chunk. */
function revertHome(cs: ChunkState): Side {
  return cs.kind === "TheirsOnly" ? "right" : "left";
}
