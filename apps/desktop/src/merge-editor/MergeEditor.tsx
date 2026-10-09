import { redo, undo } from "@codemirror/commands";
import { openSearchPanel } from "@codemirror/search";
import type { EditorState, TransactionSpec } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Analysis } from "../ipc/bindings";
import { ConfirmDialog } from "./dialogs/ConfirmDialog";
import { SaveDialog } from "./dialogs/SaveDialog";
import type { MergeEditorExtension } from "./extensions";
import { Connector } from "./gutters/Connector";
import { SideHeader } from "./header/SideHeader";
import { formatShortcut, matchShortcut, type ShortcutId } from "./keymap";
import {
  acceptWholeSide,
  applyNonConflicting,
  applySide,
  canResolveSimple,
  countNonConflicting,
  counter,
  counterLabel,
  hasManualEdits,
  ignoreChunk,
  ignoreSide,
  resolveSimple,
  revertChunk,
} from "./model/actions";
import { chunkAt, pickChunk, type NavKind } from "./model/navigation";
import { buildSaveResult } from "./model/serialize";
import { chunksOf, isResolved } from "./model/session";
import { sideLines } from "./model/text";
import {
  DEFAULT_SETTINGS,
  type MergeDocument,
  type MergeSettings,
  type SaveMode,
  type SaveResult,
  type Side,
  type WhitespacePolicy,
} from "./model/types";
import { languageLabel } from "./lang";
import { foldEffects, foldPlaceholder, unfoldAllEffects } from "./panes/folds";
import { sidePaneChunks } from "./panes/paneChunks";
import { ResizeHandle } from "./panes/ResizeHandle";
import { ResultPane } from "./panes/ResultPane";
import { SidePane } from "./panes/SidePane";
import { resultAnchors, sideAnchors, type PaneId } from "./sync/anchors";
import { ScrollSync } from "./sync/scrollSync";
import { Toolbar } from "./toolbar/Toolbar";
import styles from "./MergeEditor.module.css";

export interface MergeEditorProps {
  doc: MergeDocument;
  onSave: (result: SaveResult) => Promise<void>;
  onCancel: () => void;
  settings?: Partial<MergeSettings>;
  onSettingsChange?: (settings: MergeSettings) => void;
  /** Re-runs the engine for a whitespace policy; the selector is disabled without it. */
  reanalyze?: (policy: WhitespacePolicy) => Promise<Analysis>;
  extensions?: MergeEditorExtension[];
  /** Called when the Result differs from (or returns to) its initial content. */
  onDirtyChange?: (dirty: boolean) => void;
  /** Increment to open the Apply flow (e.g. from a host's "save first" prompt). */
  saveRequest?: number;
}

type Dialog =
  | { kind: "save" }
  | { kind: "cancel" }
  | { kind: "accept"; side: Side }
  | { kind: "whitespace"; policy: WhitespacePolicy }
  | null;

type Views = Partial<Record<PaneId, EditorView>>;

const ENCODING_LABELS = {
  Utf8: "UTF-8",
  Utf16Le: "UTF-16 LE",
  Utf16Be: "UTF-16 BE",
} as const;
const EOL_LABELS = { Lf: "LF", Crlf: "CRLF", Cr: "CR", None: "LF" } as const;

/** The three-pane merge editor. Host-agnostic: save/cancel are callbacks. */
export function MergeEditor(props: MergeEditorProps) {
  const [session, setSession] = useState({
    doc: props.doc,
    analysis: props.doc.analysis,
    epoch: 0,
  });
  // A new document (or a re-analysis) remounts the panes from scratch.
  if (session.doc !== props.doc) {
    setSession({
      doc: props.doc,
      analysis: props.doc.analysis,
      epoch: session.epoch + 1,
    });
  }
  // Settings live here so they survive the remount a re-analysis causes.
  const [settings, setSettings] = useState<MergeSettings>({
    ...DEFAULT_SETTINGS,
    ...props.settings,
  });
  const updateSettings = (patch: Partial<MergeSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    props.onSettingsChange?.(next);
  };
  return (
    <Inner
      key={session.epoch}
      {...props}
      settings={settings}
      updateSettings={updateSettings}
      analysis={session.analysis}
      onReanalysed={(analysis) =>
        setSession((s) => ({ ...s, analysis, epoch: s.epoch + 1 }))
      }
    />
  );
}

interface InnerProps extends Omit<MergeEditorProps, "settings"> {
  settings: MergeSettings;
  updateSettings: (patch: Partial<MergeSettings>) => void;
  analysis: Analysis;
  onReanalysed: (a: Analysis) => void;
}

function Inner({
  doc,
  analysis,
  onSave,
  onCancel,
  settings,
  updateSettings,
  reanalyze,
  extensions = [],
  onReanalysed,
  onDirtyChange,
  saveRequest = 0,
}: InnerProps) {
  const [views, setViews] = useState<Views>({});
  const viewsRef = useRef<Views>({});
  const [resultState, setResultState] = useState<EditorState | null>(null);
  const [version, setVersion] = useState(0);
  const [currentId, setCurrentId] = useState<number | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [hint, setHint] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [weights, setWeights] = useState({
    left: 1,
    base: 1,
    result: 1,
    right: 1,
  });
  const initialDoc = useRef<string | null>(null);
  const panesRef = useRef<HTMLDivElement>(null);
  const hintTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const labels = doc.labels;
  const leftName = labels.left.refName ?? labels.left.role;
  const rightName = labels.right.refName ?? labels.right.role;
  const fileName = doc.displayPath.split(/[\\/]/).pop() ?? doc.displayPath;

  const sync = useMemo(() => {
    const total = {
      left: sideLines(analysis.ours).length,
      right: sideLines(analysis.theirs).length,
      base: sideLines(analysis.base).length,
    };
    const fixed = {
      left: sideAnchors(analysis, "left", total.left),
      right: sideAnchors(analysis, "right", total.right),
      base: sideAnchors(analysis, "base", total.base),
    };
    return new ScrollSync(() => {
      const rv = viewsRef.current.result;
      return {
        ...fixed,
        ...(rv
          ? { result: resultAnchors(chunksOf(rv.state), rv.state.doc) }
          : {}),
      };
    });
  }, [analysis]);

  useEffect(() => () => sync.destroy(), [sync]);
  useEffect(() => {
    sync.enabled = settings.syncScroll;
  }, [sync, settings.syncScroll]);

  const registerView = useCallback(
    (id: PaneId, view: EditorView) => {
      viewsRef.current = { ...viewsRef.current, [id]: view };
      sync.attach(id, view);
      setViews(viewsRef.current);
    },
    [sync],
  );

  // Dropping the base pane removes its view from sync.
  useEffect(() => {
    if (!settings.showBase) {
      sync.detach("base");
      const { base: _base, ...rest } = viewsRef.current;
      void _base;
      viewsRef.current = rest;
      setViews(rest);
    }
  }, [settings.showBase, sync]);

  const dirtyCallback = useRef(onDirtyChange);
  dirtyCallback.current = onDirtyChange;
  const lastDirty = useRef(false);
  const onResultState = useCallback((state: EditorState) => {
    if (initialDoc.current === null) initialDoc.current = state.doc.toString();
    const dirty = state.doc.toString() !== initialDoc.current;
    if (dirty !== lastDirty.current) {
      lastDirty.current = dirty;
      dirtyCallback.current?.(dirty);
    }
    setResultState(state);
    setVersion((v) => v + 1);
  }, []);

  const resultView = views.result ?? null;
  const chunkStates = useMemo(
    () => (resultState ? chunksOf(resultState) : []),
    [resultState],
  );
  const count = useMemo(
    () => (resultState ? counter(resultState) : { changes: 0, conflicts: 0 }),
    [resultState],
  );

  const dispatchSpec = useCallback((spec: TransactionSpec | null) => {
    const v = viewsRef.current.result;
    if (spec && v) v.dispatch(spec);
  }, []);

  const act = useCallback(
    (fn: (state: EditorState, a: Analysis) => TransactionSpec | null) => {
      const v = viewsRef.current.result;
      if (v) dispatchSpec(fn(v.state, analysis));
    },
    [analysis, dispatchSpec],
  );

  const flash = useCallback((message: string) => {
    setHint(message);
    clearTimeout(hintTimer.current);
    hintTimer.current = setTimeout(() => setHint(""), 2500);
  }, []);
  useEffect(() => () => clearTimeout(hintTimer.current), []);

  const navigate = useCallback(
    (dir: 1 | -1, kind: NavKind) => {
      const v = viewsRef.current.result;
      if (!v) return;
      const target = pickChunk(
        chunksOf(v.state),
        v.state.selection.main.head,
        dir,
        kind,
        currentId,
      );
      if (!target) {
        flash(kind === "conflict" ? "No unresolved conflicts" : "No changes");
        return;
      }
      const { chunk, wrapped } = target;
      setCurrentId(chunk.id);
      v.dispatch({
        selection: { anchor: chunk.from },
        effects: EditorView.scrollIntoView(chunk.from, { y: "center" }),
      });
      if (wrapped) {
        flash(
          dir === 1 ? `Wrapped to first ${kind}` : `Wrapped to last ${kind}`,
        );
      }
      requestAnimationFrame(() =>
        requestAnimationFrame(() => sync.alignFrom("result")),
      );
    },
    [currentId, flash, sync],
  );

  const targetChunkId = useCallback((): number | null => {
    const v = viewsRef.current.result;
    if (!v) return null;
    const at = chunkAt(chunksOf(v.state), v.state.selection.main.head);
    if (at) return at.id;
    return currentId;
  }, [currentId]);

  const startSave = useCallback(() => {
    const v = viewsRef.current.result;
    if (!v) return;
    if (counter(v.state).changes === 0) void doSave("resolved");
    else setDialog({ kind: "save" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const lastSaveRequest = useRef(saveRequest);
  useEffect(() => {
    if (saveRequest !== lastSaveRequest.current) {
      lastSaveRequest.current = saveRequest;
      startSave();
    }
  }, [saveRequest, startSave]);

  async function doSave(mode: SaveMode) {
    const v = viewsRef.current.result;
    if (!v) return;
    setDialog(null);
    setSaving(true);
    setError(null);
    try {
      await onSave(buildSaveResult(v.state, analysis, labels, mode));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  const isDirty = () => {
    const v = viewsRef.current.result;
    return (
      !!v &&
      initialDoc.current !== null &&
      v.state.doc.toString() !== initialDoc.current
    );
  };

  const handleShortcut = useCallback(
    (id: ShortcutId) => {
      switch (id) {
        case "nextChange":
          return navigate(1, "change");
        case "prevChange":
          return navigate(-1, "change");
        case "nextConflict":
          return navigate(1, "conflict");
        case "prevConflict":
          return navigate(-1, "conflict");
        case "applyLeft":
        case "applyRight": {
          const cid = targetChunkId();
          if (cid !== null)
            act((s, a) =>
              applySide(s, a, cid, id === "applyLeft" ? "left" : "right"),
            );
          return;
        }
        case "ignore": {
          const cid = targetChunkId();
          if (cid !== null) act((s, a) => ignoreChunk(s, a, cid));
          return;
        }
        case "resolveSimple":
          return act((s, a) => resolveSimple(s, a));
        case "applyNonConflicting":
          return act((s, a) => applyNonConflicting(s, a, "all"));
        case "save":
          return startSave();
        case "find": {
          const v = viewsRef.current.result;
          if (v) openSearchPanel(v);
          return;
        }
      }
    },
    [act, navigate, startSave, targetChunkId],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (dialog) return;
      const id = matchShortcut(e);
      if (id) {
        e.preventDefault();
        e.stopPropagation();
        handleShortcut(id);
        return;
      }
      // Undo/redo from outside the Result pane still act on the Result.
      const v = viewsRef.current.result;
      const target = e.target as Node | null;
      if (
        v &&
        target &&
        !v.dom.contains(target) &&
        (e.metaKey || e.ctrlKey) &&
        !e.altKey &&
        e.code === "KeyZ"
      ) {
        e.preventDefault();
        (e.shiftKey ? redo : undo)(v);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [dialog, handleShortcut]);

  const viewKeys = Object.keys(views).sort().join(",");

  // Marks the first frame after every pane exists (read by the performance e2e test).
  const painted = useRef(false);
  useEffect(() => {
    if (painted.current || !views.left || !views.right || !views.result) return;
    painted.current = true;
    requestAnimationFrame(() => performance.mark("mergeiq:first-paint"));
  }, [views]);

  // Collapse unchanged: fold the same regions in every pane, or unfold them all.
  useEffect(() => {
    const entries = Object.entries(views) as [PaneId, EditorView][];
    if (entries.length === 0) return;
    const rv = views.result;
    if (!rv) return;
    const anchors = {
      left: sideAnchors(analysis, "left", sideLines(analysis.ours).length),
      right: sideAnchors(analysis, "right", sideLines(analysis.theirs).length),
      base: sideAnchors(analysis, "base", sideLines(analysis.base).length),
      result: resultAnchors(chunksOf(rv.state), rv.state.doc),
    };
    for (const [id, view] of entries) {
      const effects = settings.collapseUnchanged
        ? [...unfoldAllEffects(view), ...foldEffects(view.state, anchors[id])]
        : unfoldAllEffects(view);
      if (effects.length > 0) view.dispatch({ effects });
    }
    // Folds are laid out once per toggle/pane set; later edits shift them via mapping.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.collapseUnchanged, viewKeys, analysis]);

  const sideExtensions = useMemo(() => [foldPlaceholder], []);

  const leftChunks = useMemo(
    () => sidePaneChunks(analysis, "left", chunkStates, currentId),
    [analysis, chunkStates, currentId],
  );
  const rightChunks = useMemo(
    () => sidePaneChunks(analysis, "right", chunkStates, currentId),
    [analysis, chunkStates, currentId],
  );

  const extCtx = useMemo(
    () =>
      resultState
        ? {
            analysis,
            state: resultState,
            view: resultView,
            dispatch: dispatchSpec,
          }
        : null,
    [analysis, resultState, resultView, dispatchSpec],
  );

  const resize =
    (a: keyof typeof weights, b: keyof typeof weights) => (dx: number) => {
      const W = panesRef.current?.clientWidth ?? 1;
      setWeights((w) => {
        const total =
          w.left + w.right + w.result + (settings.showBase ? w.base : 0);
        const dw = (dx / W) * total;
        const min = 0.25;
        const na = Math.max(min, w[a] + dw);
        const nb = Math.max(min, w[b] - dw);
        return { ...w, [a]: na, [b]: nb };
      });
    };

  const cursor = resultState
    ? (() => {
        const head = resultState.selection.main.head;
        const line = resultState.doc.lineAt(head);
        return `Ln ${line.number}, Col ${head - line.from + 1}`;
      })()
    : "";

  const pillWarn = count.changes > 0;
  const firstConflict = chunkStates.find(
    (c) => c.kind === "Conflict" && !isResolved(c),
  );
  const firstConflictLine =
    firstConflict && resultState
      ? resultState.doc.lineAt(firstConflict.from).number
      : null;
  const nonConflicting = resultState
    ? {
        all: countNonConflicting(resultState, "all"),
        left: countNonConflicting(resultState, "left"),
        right: countNonConflicting(resultState, "right"),
      }
    : { all: 0, left: 0, right: 0 };

  const sc = (...parts: string[]) => formatShortcut(parts);
  const w = weights;
  const totalWeight =
    w.left + w.result + w.right + (settings.showBase ? w.base : 0);
  const pct = (share: number) => (share / totalWeight) * 100;
  const leftShare = w.left + (settings.showBase ? w.base : 0);

  return (
    <div className={styles.root} data-testid="merge-editor">
      <header className={styles.titlebar}>
        <div className={styles.titleMain}>
          <span className={styles.fileName}>{fileName}</span>
          <span className={styles.merging}>
            Merging <span className={styles.mono}>{rightName}</span> into{" "}
            <span className={styles.mono}>{leftName}</span>
          </span>
        </div>
        <div className={styles.titleActions}>
          <span
            role="status"
            data-testid="counter"
            className={`${styles.pill} ${pillWarn ? "" : styles.pillDone}`}
          >
            {counterLabel(count)}
          </span>
          <button
            type="button"
            className={styles.btn}
            onClick={() =>
              isDirty() ? setDialog({ kind: "cancel" }) : onCancel()
            }
          >
            Cancel
          </button>
          <button
            type="button"
            className={`${styles.btn} ${styles.primary}`}
            disabled={saving}
            onClick={startSave}
          >
            Apply <span className={styles.kbdOnAccent}>{sc("Mod", "S")}</span>
          </button>
        </div>
      </header>

      <Toolbar
        settings={settings}
        counts={nonConflicting}
        canResolveSimple={
          resultState ? canResolveSimple(resultState, analysis) : false
        }
        leftName={leftName}
        rightName={rightName}
        canReanalyze={!!reanalyze}
        hint={hint}
        extra={
          extCtx
            ? extensions.map((x, i) => (
                <span key={i}>{x.toolbarItems?.(extCtx)}</span>
              ))
            : null
        }
        onApplyNonConflicting={(scope) =>
          act((s, a) => applyNonConflicting(s, a, scope))
        }
        onResolveSimple={() => act((s, a) => resolveSimple(s, a))}
        onAcceptSide={(side) => {
          const v = viewsRef.current.result;
          if (v && hasManualEdits(v.state)) setDialog({ kind: "accept", side });
          else act((s, a) => acceptWholeSide(s, a, side));
        }}
        onNavigate={navigate}
        onToggle={(key) => updateSettings({ [key]: !settings[key] })}
        onWhitespace={(policy) => {
          const resolved = chunkStates.some(isResolved);
          if (resolved) setDialog({ kind: "whitespace", policy });
          else void rerun(policy);
        }}
      />

      {error ? (
        <div role="alert" className={styles.error}>
          Could not save: {error}
        </div>
      ) : null}

      <div className={styles.scroller}>
        <div
          className={styles.inner}
          style={{ minWidth: settings.showBase ? 1320 : 1040 }}
        >
          <div className={styles.headers}>
            <div className={styles.cell} style={{ flex: `${w.left} 1 0` }}>
              <SideHeader
                side="left"
                label={labels.left}
                commits={doc.context?.ours ?? []}
              />
            </div>
            {settings.showBase ? (
              <div
                className={`${styles.cell} ${styles.baseHeader}`}
                style={{ flex: `${w.base} 1 0` }}
              >
                <span className={styles.line}>
                  <span className={styles.baseName}>Base</span>
                  <span className={styles.chip}>read-only</span>
                </span>
                <span className={styles.sub}>
                  {labels.base?.shortSha ?? "merge base"}
                </span>
              </div>
            ) : null}
            <div className={styles.spacer} />
            <div
              className={`${styles.cell} ${styles.resultHeader}`}
              style={{ flex: `${w.result} 1 0` }}
            >
              <span className={styles.line}>
                <span className={styles.baseName}>Result</span>
                <span className={`${styles.chip} ${styles.chipEditable}`}>
                  editable
                </span>
              </span>
              <span className={`${styles.sub} ${styles.mono}`}>
                {doc.displayPath}
              </span>
            </div>
            <div className={styles.spacer} />
            <div className={styles.cell} style={{ flex: `${w.right} 1 0` }}>
              <SideHeader
                side="right"
                label={labels.right}
                commits={doc.context?.theirs ?? []}
              />
            </div>
          </div>

          <div ref={panesRef} className={styles.panes}>
            <div className={styles.pane} style={{ flex: `${w.left} 1 0` }}>
              <SidePane
                role="left"
                label={`Left: ${leftName}`}
                analysis={analysis}
                displayPath={doc.displayPath}
                chunkStates={chunkStates}
                currentId={currentId}
                extensions={sideExtensions}
                onView={registerView}
              />
            </div>
            {settings.showBase ? (
              <div className={styles.pane} style={{ flex: `${w.base} 1 0` }}>
                <SidePane
                  role="base"
                  label="Base"
                  analysis={analysis}
                  displayPath={doc.displayPath}
                  chunkStates={chunkStates}
                  currentId={currentId}
                  extensions={sideExtensions}
                  onView={registerView}
                />
              </div>
            ) : null}
            <div className={styles.gutterCell}>
              <Connector
                side="left"
                sideView={views.left ?? null}
                resultView={resultView}
                sideChunks={leftChunks}
                chunkStates={chunkStates}
                resultState={resultState}
                version={version}
                currentId={currentId}
                onApply={(id, side) => act((s, a) => applySide(s, a, id, side))}
                onIgnore={(id, side) =>
                  act((s, a) => ignoreOne(s, a, id, side))
                }
                onRevert={(id) => act((s, a) => revertChunk(s, a, id))}
                extra={
                  extCtx
                    ? (id) =>
                        extensions.map((x, i) => (
                          <span key={i}>{x.chunkActions?.(extCtx, id)}</span>
                        ))
                    : undefined
                }
              />
              <ResizeHandle
                edge="right"
                label="Resize left panes"
                value={pct(leftShare)}
                onResize={resize(settings.showBase ? "base" : "left", "result")}
              />
            </div>
            <div className={styles.pane} style={{ flex: `${w.result} 1 0` }}>
              <ResultPane
                analysis={analysis}
                displayPath={doc.displayPath}
                autoApply={settings.autoApplyNonConflicting}
                currentId={currentId}
                extensions={sideExtensions}
                onView={(v) => registerView("result", v)}
                onState={onResultState}
              />
            </div>
            <div className={styles.gutterCell}>
              <ResizeHandle
                edge="left"
                label="Resize right pane"
                value={pct(leftShare + w.result)}
                onResize={resize("result", "right")}
              />
              <Connector
                side="right"
                sideView={views.right ?? null}
                resultView={resultView}
                sideChunks={rightChunks}
                chunkStates={chunkStates}
                resultState={resultState}
                version={version}
                currentId={currentId}
                onApply={(id, side) => act((s, a) => applySide(s, a, id, side))}
                onIgnore={(id, side) =>
                  act((s, a) => ignoreOne(s, a, id, side))
                }
                onRevert={(id) => act((s, a) => revertChunk(s, a, id))}
              />
            </div>
            <div className={styles.pane} style={{ flex: `${w.right} 1 0` }}>
              <SidePane
                role="right"
                label={`Right: ${rightName}`}
                analysis={analysis}
                displayPath={doc.displayPath}
                chunkStates={chunkStates}
                currentId={currentId}
                extensions={sideExtensions}
                onView={registerView}
              />
            </div>
          </div>
        </div>
      </div>

      <footer className={styles.footer}>
        <span className={styles.footLeft}>
          <span>{languageLabel(doc.displayPath)}</span>
          <span>
            {ENCODING_LABELS[analysis.encoding.encoding]}
            {analysis.encoding.bom ? " with BOM" : ""}
          </span>
          <span>{EOL_LABELS[analysis.dominant_eol]}</span>
          <span data-testid="cursor">{cursor}</span>
        </span>
        <span className={styles.footRight}>
          <span>
            <kbd className={styles.kbd}>F7</kbd> next conflict
          </span>
          <span>
            <kbd className={styles.kbd}>{sc("Mod", "Alt", "Left")}</kbd> apply
            left
          </span>
          <span>
            <kbd className={styles.kbd}>{sc("Mod", "Alt", "Right")}</kbd> apply
            right
          </span>
          <span>
            <kbd className={styles.kbd}>{sc("Mod", "Alt", "Backspace")}</kbd>{" "}
            ignore
          </span>
        </span>
      </footer>

      {dialog?.kind === "save" ? (
        <SaveDialog
          fileName={fileName}
          conflicts={count.conflicts}
          changes={count.changes}
          firstConflictLine={firstConflictLine}
          onChoose={(choice) => {
            if (choice === "continue") {
              setDialog(null);
              navigate(1, "conflict");
            } else void doSave(choice);
          }}
        />
      ) : null}
      {dialog?.kind === "cancel" ? (
        <ConfirmDialog
          title="Discard your changes?"
          message="The result differs from what the editor opened with. Closing now loses those edits."
          confirmLabel="Discard changes"
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            setDialog(null);
            onCancel();
          }}
        />
      ) : null}
      {dialog?.kind === "accept" ? (
        <ConfirmDialog
          title={`Replace the result with ${dialog.side === "left" ? leftName : rightName}?`}
          message="Your manual edits in the result will be replaced. You can undo this."
          confirmLabel={`Accept ${dialog.side === "left" ? "Left" : "Right"}`}
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            const side = dialog.side;
            setDialog(null);
            act((s, a) => acceptWholeSide(s, a, side));
          }}
        />
      ) : null}
      {dialog?.kind === "whitespace" ? (
        <ConfirmDialog
          title="Reset your progress?"
          message="Changing the whitespace policy recomputes all chunks and discards what you have resolved so far."
          confirmLabel="Recompute"
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            const policy = dialog.policy;
            setDialog(null);
            void rerun(policy);
          }}
        />
      ) : null}
    </div>
  );

  async function rerun(policy: WhitespacePolicy) {
    if (!reanalyze) return;
    try {
      const next = await reanalyze(policy);
      updateSettings({ whitespacePolicy: policy });
      onReanalysed(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }
}

/** Ignores a single side of a chunk (the gutter × button). */
function ignoreOne(
  state: EditorState,
  analysis: Analysis,
  id: number,
  side: Side,
): TransactionSpec | null {
  return ignoreSide(state, analysis, id, side);
}
