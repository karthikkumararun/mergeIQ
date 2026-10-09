import { defaultKeymap, historyKeymap } from "@codemirror/commands";
import {
  Compartment,
  type EditorState,
  type Extension,
} from "@codemirror/state";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";
import type { Analysis } from "../../ipc/bindings";
import { loadLanguage, languageForPath } from "../lang";
import { createResultState } from "../model/state";
import { resultPaneDecorations, setCurrentChunk } from "./decorations";
import { paneHighlight, paneTheme } from "./theme";
import { useEditorView } from "./useEditorView";
import styles from "./Pane.module.css";

interface Props {
  analysis: Analysis;
  displayPath: string;
  autoApply: boolean;
  currentId: number | null;
  extensions: Extension;
  onView: (view: EditorView) => void;
  onState: (state: EditorState) => void;
}

/** The editable Result pane; owns the chunk session and undo history. */
export function ResultPane({
  analysis,
  displayPath,
  autoApply,
  currentId,
  extensions,
  onView,
  onState,
}: Props) {
  const host = useRef<HTMLDivElement>(null);
  const lang = useRef(new Compartment());
  const onStateRef = useRef(onState);
  useEffect(() => {
    onStateRef.current = onState;
  });

  const view = useEditorView(
    host,
    () =>
      createResultState(analysis, {
        autoApply,
        extensions: [
          lineNumbers(),
          resultPaneDecorations(analysis),
          paneTheme,
          paneHighlight,
          highlightSelectionMatches(),
          lang.current.of([]),
          keymap.of([...searchKeymap, ...historyKeymap, ...defaultKeymap]),
          EditorView.contentAttributes.of({ "aria-label": "Result" }),
          EditorView.updateListener.of((u) => {
            if (
              u.docChanged ||
              u.selectionSet ||
              u.transactions.some((t) => t.effects.length > 0)
            ) {
              onStateRef.current(u.state);
            }
          }),
          extensions,
        ],
      }),
    (v) => {
      onView(v);
      onStateRef.current(v.state);
    },
  );

  useEffect(() => {
    if (view) view.dispatch({ effects: setCurrentChunk.of(currentId) });
  }, [view, currentId]);

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

  return <div ref={host} className={styles.pane} data-pane="result" />;
}
