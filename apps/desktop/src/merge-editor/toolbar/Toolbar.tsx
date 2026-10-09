import type { ReactNode } from "react";
import { formatShortcut } from "../keymap";
import type { MergeSettings, WhitespacePolicy } from "../model/types";
import { Menu } from "./Menu";
import styles from "./Toolbar.module.css";

export interface ToolbarProps {
  settings: MergeSettings;
  counts: { all: number; left: number; right: number };
  canResolveSimple: boolean;
  leftName: string;
  rightName: string;
  canReanalyze: boolean;
  onApplyNonConflicting: (scope: "all" | "left" | "right") => void;
  onResolveSimple: () => void;
  onAcceptSide: (side: "left" | "right") => void;
  onNavigate: (dir: 1 | -1, kind: "change" | "conflict") => void;
  onToggle: (key: "showBase" | "collapseUnchanged" | "syncScroll") => void;
  onWhitespace: (policy: WhitespacePolicy) => void;
  /** Extra toolbar content contributed by `MergeEditorExtension`s. */
  extra?: ReactNode;
  hint?: string;
}

const WHITESPACE_OPTIONS: { value: WhitespacePolicy; label: string }[] = [
  { value: "Exact", label: "Compare all" },
  { value: "TrimTrailing", label: "Ignore trailing" },
  { value: "IgnoreAmount", label: "Ignore amount" },
  { value: "IgnoreAll", label: "Ignore all" },
];

export function Toolbar(p: ToolbarProps) {
  const sc = (...parts: string[]) => formatShortcut(parts);
  return (
    <div role="toolbar" aria-label="Merge actions" className={styles.toolbar}>
      <Menu
        label={
          <>
            <span aria-hidden="true" className={styles.insIcon}>
              »
            </span>
            Apply non-conflicting
            <span aria-hidden="true">▾</span>
          </>
        }
        ariaLabel="Apply non-conflicting changes"
        disabled={p.counts.all === 0}
        items={[
          {
            id: "all",
            label: "All",
            hint: sc("Mod", "Alt", "A"),
            disabled: p.counts.all === 0,
            onSelect: () => p.onApplyNonConflicting("all"),
          },
          {
            id: "left",
            label: "Left only",
            disabled: p.counts.left === 0,
            onSelect: () => p.onApplyNonConflicting("left"),
          },
          {
            id: "right",
            label: "Right only",
            disabled: p.counts.right === 0,
            onSelect: () => p.onApplyNonConflicting("right"),
          },
        ]}
      />
      <button
        type="button"
        className={styles.btn}
        disabled={!p.canResolveSimple}
        title={`Resolve simple conflicts (${sc("Mod", "Alt", "M")})`}
        onClick={p.onResolveSimple}
      >
        <span aria-hidden="true">✦</span>
        Resolve simple
      </button>
      <span className={styles.sep} />
      <button
        type="button"
        className={styles.btn}
        title={`Replace the result with ${p.leftName}`}
        onClick={() => p.onAcceptSide("left")}
      >
        Accept Left
      </button>
      <button
        type="button"
        className={styles.btn}
        title={`Replace the result with ${p.rightName}`}
        onClick={() => p.onAcceptSide("right")}
      >
        Accept Right
      </button>
      <span className={styles.sep} />
      <button
        type="button"
        className={styles.ib}
        aria-label="Previous change (Alt+Up)"
        onClick={() => p.onNavigate(-1, "change")}
      >
        ⌃
      </button>
      <button
        type="button"
        className={styles.ib}
        aria-label="Next change (Alt+Down)"
        onClick={() => p.onNavigate(1, "change")}
      >
        ⌄
      </button>
      <button
        type="button"
        className={`${styles.btn} ${styles.conflictNav}`}
        aria-label="Next conflict (F7)"
        onClick={() => p.onNavigate(1, "conflict")}
      >
        Next conflict <span className={styles.kbd}>F7</span>
      </button>
      <Menu
        label="⋯"
        ariaLabel="More navigation"
        buttonClass={styles.ib}
        items={[
          {
            id: "prev-conflict",
            label: "Previous conflict",
            hint: "Shift+F7",
            onSelect: () => p.onNavigate(-1, "conflict"),
          },
        ]}
      />
      <span className={styles.sep} />
      <button
        type="button"
        className={styles.btn}
        aria-pressed={p.settings.showBase}
        onClick={() => p.onToggle("showBase")}
      >
        Show base
      </button>
      <button
        type="button"
        className={styles.btn}
        aria-pressed={p.settings.collapseUnchanged}
        onClick={() => p.onToggle("collapseUnchanged")}
      >
        Collapse unchanged
      </button>
      <button
        type="button"
        className={styles.btn}
        aria-pressed={p.settings.syncScroll}
        onClick={() => p.onToggle("syncScroll")}
      >
        Sync scroll
      </button>
      <span className={styles.sep} />
      <label className={styles.select}>
        Whitespace
        <select
          value={p.settings.whitespacePolicy}
          disabled={!p.canReanalyze}
          onChange={(e) => p.onWhitespace(e.target.value as WhitespacePolicy)}
        >
          {WHITESPACE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </label>
      {p.extra}
      <span className={styles.hint} role="status" aria-live="polite">
        {p.hint}
      </span>
    </div>
  );
}
