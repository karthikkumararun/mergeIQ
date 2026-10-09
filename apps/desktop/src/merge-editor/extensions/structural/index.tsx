import type { MergeEditorExtension } from "../../extensions";
import type { ResolveStructural } from "./api";
import { structuralGutter } from "./gutter";
import { StructuralPopover } from "./Popover";
import { createStructuralStore, type StructuralStore } from "./store";
import { StructuralToolbar } from "./Toolbar";

/** The extension together with the store that backs it (tests drive the store directly). */
export function createStructuralParts(resolve?: ResolveStructural): {
  extension: MergeEditorExtension;
  store: StructuralStore;
} {
  const store = createStructuralStore(resolve);
  return {
    store,
    extension: {
      toolbarItems: (ctx) => <StructuralToolbar ctx={ctx} store={store} />,
      resultExtensions: structuralGutter((i) => store.getState().open(i)),
      resultOverlay: (ctx) => <StructuralPopover ctx={ctx} store={store} />,
    },
  };
}

/**
 * Syntax-aware conflict resolution (`structural-merge`): a gutter indicator and preview per
 * proposal, and a toolbar action that applies all of them as one undoable step. One instance
 * per merge editor (it owns the proposals computed for that editor's analysis).
 */
export function createStructuralExtension(
  resolve?: ResolveStructural,
): MergeEditorExtension {
  return createStructuralParts(resolve).extension;
}
