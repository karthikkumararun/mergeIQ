import { commands, type IpcError, type MergeRequestDoc } from "../ipc/bindings";
import { describeError } from "../merge-editor/hosts";
import { renderSaveText } from "../merge-editor/model/serialize";
import type { MergeDocument, SaveResult } from "../merge-editor/model/types";

/** Builds the editor's document from the backend's request document. */
export function toMergeDocument(doc: MergeRequestDoc): MergeDocument {
  return {
    pathToken: "",
    displayPath: doc.displayPath,
    analysis: doc.analysis,
    labels: doc.labels,
    context: doc.context ?? undefined,
  };
}

export function fail(error: IpcError): never {
  throw new Error(describeError(error));
}

/** Apply: write the result (the backend records the exit code), then close the window. */
export async function saveAndClose(id: number, result: SaveResult) {
  const saved = await commands.mergeRequestSave(
    id,
    renderSaveText(result),
    result.encoding,
    result.mode,
  );
  if (saved.status === "error") fail(saved.error);
  await commands.requestClose(id);
}
