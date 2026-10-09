import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";
import type { Analysis } from "../../ipc/bindings";
import { loadLanguage, languageForPath } from "../lang";
import { documentText, sideLines } from "../model/text";
import type { ChunkState } from "../model/types";
import { setPaneChunks, sidePaneDecorations } from "./decorations";
import { sidePaneChunks } from "./paneChunks";
import { paneHighlight, paneTheme } from "./theme";
import { readOnly, useEditorView } from "./useEditorView";
import styles from "./Pane.module.css";

interface Props {
  role: "left" | "right" | "base";
  analysis: Analysis;
  displayPath: string;
  chunkStates: readonly ChunkState[];
  currentId: number | null;
  /** Extra extensions (folds, scroll listeners). */
  extensions: Extension;
  onView: (role: "left" | "right" | "base", view: EditorView) => void;
  label: string;
}

/** A read-only pane showing one side (or base) with chunk highlighting. */
export function SidePane({
  role,
  analysis,
  displayPath,
  chunkStates,
  currentId,
  extensions,
  onView,
  label,
}: Props) {
  const host = useRef<HTMLDivElement>(null);
  const lang = useRef(new Compartment());
  const text =
    role === "left"
      ? analysis.ours
      : role === "right"
        ? analysis.theirs
        : analysis.base;
  const view = useEditorView(
    host,
    () =>
      EditorState.create({
        doc: documentText(sideLines(text)),
        extensions: [
          readOnly(),
          lineNumbers(),
          sidePaneDecorations({ marks: role !== "base" }),
          paneTheme,
          paneHighlight,
          lang.current.of([]),
          extensions,
          EditorView.contentAttributes.of({
            "aria-label": label,
            "aria-readonly": "true",
          }),
        ],
      }),
    (v) => onView(role, v),
  );

  useEffect(() => {
    if (!view) return;
    view.dispatch({
      effects: setPaneChunks.of(
        sidePaneChunks(analysis, role, chunkStates, currentId),
      ),
    });
  }, [view, analysis, role, chunkStates, currentId]);

  useEffect(() => {
    const id = languageForPath(displayPath);
    if (!view || !id) return;
    let cancelled = false;
    void loadLanguage(id).then((ext) => {
      if (!cancelled) view.dispatch({ effects: lang.current.reconfigure(ext) });
    });
    return () => {
      cancelled = true;
    };
  }, [view, displayPath]);

  return <div ref={host} className={styles.pane} data-pane={role} />;
}
