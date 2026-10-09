import type { ConflictLoad, RenameOutcome } from "../ipc/bindings";
import { NonTextPanel } from "../repo/NonTextPanel";
import type { AcceptSide, RepoApi } from "../repo/repoApi";
import type { ResolutionMethod } from "../repo/repoStore";
import { BinaryPanel } from "./BinaryPanel";
import { GoSumPanel } from "./GoSumPanel";
import { useAsync } from "./hooks";
import { LockfilePanel } from "./LockfilePanel";
import { ModifyDeletePanel } from "./ModifyDeletePanel";
import { LfsPanel, OversizedPanel, SymlinkPanel } from "./PickSidePanel";
import { RenamePanel } from "./RenamePanel";
import shell from "./Special.module.css";
import { SubmodulePanel } from "./SubmodulePanel";

export interface SpecialConflictPanelProps {
  load: ConflictLoad;
  api: RepoApi;
  repoRoot: string;
  /** ← Conflicts: closes this tab. */
  onBack: () => void;
  onResolved: (method: ResolutionMethod) => Promise<void>;
  onAccept: (side: AcceptSide) => void;
  onDelete: () => void;
  /** A lockfile run staged the file: record and refresh without closing the tab. */
  onStaged: () => Promise<void>;
  onRenameChosen: (outcome: RenameOutcome) => Promise<void>;
  /** Opens the merge editor for a lockfile instead of its panel. */
  onMergeByHand: () => void;
}

/** Conflict types whose entries can be one half of a rename/rename conflict. */
const MAYBE_RENAME = new Set([
  "BothDeleted",
  "AddedByUs",
  "AddedByThem",
  "DeletedByUs",
  "DeletedByThem",
]);

/**
 * Chooses the panel for a conflict the text editor cannot show: modify/delete, rename,
 * binary/image, symlink, submodule, LFS pointer, oversized, go.sum and other lockfiles.
 * Anything else (e.g. a text file added on one side) keeps the minimal accept/delete panel.
 */
export function SpecialConflictPanel(props: SpecialConflictPanelProps) {
  const { load, api, onBack, onResolved } = props;
  const { entry } = load;
  const probe = MAYBE_RENAME.has(entry.conflictType);
  const details = useAsync(
    () => (probe ? api.details(entry.path) : Promise.resolve(null)),
    `${entry.path}:${probe}`,
  );
  if (probe && details.state === "loading")
    return <p className={shell.loading}>Loading {entry.display}…</p>;

  const pair = details.state === "ready" ? details.value?.renamePair : null;
  if (pair) {
    return (
      <RenamePanel
        load={load}
        api={api}
        pair={pair}
        onBack={onBack}
        onChosen={props.onRenameChosen}
      />
    );
  }

  const common = { load, api, onBack, onResolved };
  if (
    entry.conflictType === "DeletedByUs" ||
    entry.conflictType === "DeletedByThem"
  ) {
    return <ModifyDeletePanel {...common} onDelete={props.onDelete} />;
  }
  const c = entry.class;
  switch (c.class) {
    case "Binary":
      return <BinaryPanel {...common} />;
    case "Symlink":
      return <SymlinkPanel {...common} />;
    case "Submodule":
      return <SubmodulePanel {...common} />;
    case "LfsPointer":
      return <LfsPanel {...common} />;
    case "Oversized":
      return <OversizedPanel {...common} />;
    case "Lockfile":
      return c.kind === "GoSum" ? (
        <GoSumPanel {...common} onMergeByHand={props.onMergeByHand} />
      ) : (
        <LockfilePanel
          {...common}
          kind={c.kind}
          repoRoot={props.repoRoot}
          onStaged={props.onStaged}
          onDone={onBack}
          onMergeByHand={props.onMergeByHand}
        />
      );
    case "Text":
      return (
        <NonTextPanel
          load={load}
          onAccept={props.onAccept}
          onDelete={props.onDelete}
        />
      );
  }
}
