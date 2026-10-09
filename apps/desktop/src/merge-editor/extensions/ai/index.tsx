import type { MergeEditorExtension } from "../../extensions";
import { aiGutter } from "./gutter";
import { AiPanel } from "./Panel";
import { createAiStore, type AiHost, type AiStore } from "./store";
import { AiToolbar } from "./Toolbar";

/** The extension together with its store (tests drive the store directly). */
export function createAiParts(host: AiHost): {
  extension: MergeEditorExtension;
  store: AiStore;
} {
  const store = createAiStore(host);
  return {
    store,
    extension: {
      toolbarItems: (ctx) => <AiToolbar ctx={ctx} host={host} store={store} />,
      resultExtensions: aiGutter((chunkId) => {
        store.getState().select(chunkId);
        void store.getState().refreshStatus();
      }),
      sidePanel: (ctx) => <AiPanel ctx={ctx} host={host} store={store} />,
    },
  };
}

/**
 * AI assistance (`ai-assist`): per-conflict Explain and Suggest in a side panel, a ✦ marker in
 * the Result gutter, and a toolbar action that suggests every remaining conflict. One instance
 * per merge editor.
 */
export function createAiExtension(host: AiHost): MergeEditorExtension {
  return createAiParts(host).extension;
}
