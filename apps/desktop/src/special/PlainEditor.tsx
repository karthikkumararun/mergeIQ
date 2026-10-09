import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";
import { languageForPath, loadLanguage } from "../merge-editor/lang";
import { paneHighlight, paneTheme } from "../merge-editor/panes/theme";
import { useEditorView } from "../merge-editor/panes/useEditorView";
import type { WorkingText } from "../repo/repoApi";
import { useAction } from "./hooks";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  display: string;
  initial: WorkingText;
  /** Writes `text` and marks the path resolved. */
  onSave: (text: string) => Promise<void>;
  onCancel: () => void;
}

/** A plain single-pane text editor ("Keep and edit"); saving marks the file resolved. */
export function PlainEditor({ display, initial, onSave, onCancel }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const lang = useRef(new Compartment());
  const { busy, error, run } = useAction();
  const crlf =
    initial.text.includes("\r\n") && !/(^|[^\r])\n/.test(initial.text);

  const view = useEditorView(host, () =>
    EditorState.create({
      doc: initial.text,
      extensions: [
        EditorState.lineSeparator.of(crlf ? "\r\n" : "\n"),
        lineNumbers(),
        history(),
        keymap.of([...historyKeymap, ...defaultKeymap]),
        paneTheme,
        paneHighlight,
        lang.current.of([]),
        EditorView.contentAttributes.of({ "aria-label": `Edit ${display}` }),
      ],
    }),
  );

  useEffect(() => {
    const id = languageForPath(display);
    if (!view || !id) return;
    let cancelled = false;
    void loadLanguage(id).then((ext) => {
      if (!cancelled) view.dispatch({ effects: lang.current.reconfigure(ext) });
    });
    return () => {
      cancelled = true;
    };
  }, [view, display]);

  const save = () => {
    if (!view) return;
    void run(() => onSave(view.state.sliceDoc()));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.altKey && e.code === "KeyS") {
        e.preventDefault();
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view]);

  return (
    <>
      <div className={styles.editorWrap}>
        <div ref={host} className={styles.editorHost} data-pane="plain" />
      </div>
      {error && (
        <p role="alert" className={shell.alert}>
          Could not save: {error}
        </p>
      )}
      <div className={styles.footerActions}>
        <button type="button" className={shell.btn} onClick={onCancel}>
          Cancel
        </button>
        <button
          type="button"
          className={`${shell.btn} ${shell.primary}`}
          disabled={busy || !view}
          onClick={save}
        >
          Save and mark resolved
        </button>
      </div>
    </>
  );
}
