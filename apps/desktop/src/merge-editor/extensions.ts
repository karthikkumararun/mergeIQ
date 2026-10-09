import type { EditorState, TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { ReactNode } from "react";
import type { Analysis } from "../ipc/bindings";

/** What an extension can see and do. */
export interface MergeEditorContext {
  analysis: Analysis;
  /** Current Result state (chunk session included). */
  state: EditorState;
  view: EditorView | null;
  /** Dispatches a transaction to the Result pane (undoable like any action). */
  dispatch: (spec: TransactionSpec) => void;
}

/**
 * Hook for later changes (`structural-merge`, `ai-assist`) to add toolbar items and
 * per-chunk actions. The default is no extensions.
 */
export interface MergeEditorExtension {
  toolbarItems?: (ctx: MergeEditorContext) => ReactNode;
  chunkActions?: (ctx: MergeEditorContext, chunkId: number) => ReactNode;
}
