import type { ConflictLoad } from "../ipc/bindings";
import type { MergeDocument } from "../merge-editor/model/types";

/** The editor's document for a conflicted file loaded from the repository. */
export function toDocument(load: ConflictLoad): MergeDocument & {
  analysis: NonNullable<ConflictLoad["analysis"]>;
} {
  return {
    pathToken: load.entry.path,
    displayPath: load.entry.display,
    analysis: load.analysis!,
    labels: { left: load.labels.ours, right: load.labels.theirs },
    context: load.context,
  };
}
