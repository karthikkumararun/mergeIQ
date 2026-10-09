import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { useEffect, useRef, useState, type RefObject } from "react";

/**
 * Creates one `EditorView` in `host` for the lifetime of the component.
 * `create` runs once; the returned view is exposed through state so dependents re-render.
 */
export function useEditorView(
  host: RefObject<HTMLElement | null>,
  create: () => EditorState,
  onView?: (view: EditorView) => void,
): EditorView | null {
  const [view, setView] = useState<EditorView | null>(null);
  const createRef = useRef(create);
  const onViewRef = useRef(onView);
  useEffect(() => {
    const parent = host.current;
    if (!parent) return;
    const v = new EditorView({ state: createRef.current(), parent });
    setView(v);
    onViewRef.current?.(v);
    return () => {
      v.destroy();
      setView(null);
    };
  }, [host]);
  return view;
}

export function readOnly(): Extension {
  return [EditorState.readOnly.of(true), EditorView.editable.of(false)];
}
