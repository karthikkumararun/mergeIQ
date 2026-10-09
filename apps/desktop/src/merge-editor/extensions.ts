import type {
  EditorState,
  Extension,
  TransactionSpec,
} from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { ReactNode } from "react";
import type { Analysis } from "../ipc/bindings";
import type { MergeDocument } from "./model/types";

/** What an extension can see and do. */
export interface MergeEditorContext {
  analysis: Analysis;
  /** The document being merged: path, contextual labels and commit context. */
  doc: MergeDocument;
  /** Current Result state (chunk session included). */
  state: EditorState;
  view: EditorView | null;
  /** Dispatches a transaction to the Result pane (undoable like any action). */
  dispatch: (spec: TransactionSpec) => void;
}

/**
 * Hook for `structural-merge` and `ai-assist` (and hosts) to add toolbar items, per-chunk
 * actions, Result-pane gutter markers and overlays. The default is no extensions.
 */
export interface MergeEditorExtension {
  toolbarItems?: (ctx: MergeEditorContext) => ReactNode;
  chunkActions?: (ctx: MergeEditorContext, chunkId: number) => ReactNode;
  /** CodeMirror extensions for the Result pane (read once, when the pane is created). */
  resultExtensions?: Extension;
  /** Rendered over the Result pane in its positioned wrapper (popovers, previews). */
  resultOverlay?: (ctx: MergeEditorContext) => ReactNode;
  /** A panel docked to the right of the panes (the AI assistant). */
  sidePanel?: (ctx: MergeEditorContext) => ReactNode;
}
