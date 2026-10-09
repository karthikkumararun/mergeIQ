import { history } from "@codemirror/commands";
import { EditorState, type Extension } from "@codemirror/state";
import type { Analysis } from "../../ipc/bindings";
import { applyNonConflicting } from "./actions";
import { chunkSession, resultDocText } from "./session";

/**
 * Builds the Result pane's `EditorState`: base text plus the chunk session. With
 * `autoApply` the non-conflicting chunks are applied as one undoable step.
 */
export function createResultState(
  analysis: Analysis,
  opts: { autoApply?: boolean; extensions?: Extension } = {},
): EditorState {
  const state = EditorState.create({
    doc: resultDocText(analysis),
    extensions: [history(), chunkSession(analysis), opts.extensions ?? []],
  });
  if (!opts.autoApply) return state;
  const spec = applyNonConflicting(state, analysis, "all");
  return spec ? state.update(spec).state : state;
}
