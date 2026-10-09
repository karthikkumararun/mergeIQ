import { useEffect, useMemo, useState } from "react";
import {
  mockMergeDocument,
  recordMockCancel,
  recordMockSave,
} from "../ipc/mock";
import { AiHostContext, standaloneHost } from "../ai/context";
import {
  createMockAiApi,
  mockAiOptionsFromParams,
  type MockAiApi,
} from "../ai/mockApi";
import { MergeEditor } from "./MergeEditor";
import type { MergeEditorExtension } from "./extensions";
import { loadMergeSettings, saveMergeSettings } from "./hosts";
import type { MergeSettings } from "./model/types";
import { fixture } from "./__fixtures__";

/** Exercises the extension hook (`?extension=1`): one toolbar button and one chunk button. */
const demoExtension: MergeEditorExtension = {
  toolbarItems: (ctx) => (
    <button
      type="button"
      onClick={() =>
        ctx.dispatch({
          changes: { from: 0, insert: "ext\n" },
          userEvent: "input.ext",
        })
      }
    >
      Ext toolbar
    </button>
  ),
  chunkActions: (_ctx, id) => (
    <button type="button" aria-label={`Ext chunk ${id}`}>
      E
    </button>
  ),
};

declare global {
  interface Window {
    /** The mock AI backend of the dev editor, for Playwright assertions. */
    __mergeiqAi?: MockAiApi;
  }
}

/**
 * Dev-only host at `/dev/merge?fixture=<name>[&autoApply=1][&showBase=1][&collapse=1]`.
 * It runs the editor against engine-exported fixtures with the IPC mocked; saves are
 * recorded on `window.__mergeiqSaves` for Playwright. Persisted preferences (mock:
 * localStorage) apply unless a URL parameter overrides them.
 */
export function DevMerge() {
  const params = useMemo(() => new URLSearchParams(window.location.search), []);
  const name = params.get("fixture") ?? "mixed-changes";
  const doc = useMemo(() => {
    const base = mockMergeDocument(name);
    const path = params.get("path");
    return path ? { ...base, displayPath: path } : base;
  }, [name, params]);
  const aiApi = useMemo(
    () => createMockAiApi(mockAiOptionsFromParams(params)),
    [params],
  );
  const aiHost = useMemo(() => standaloneHost(aiApi), [aiApi]);
  useEffect(() => {
    window.__mergeiqAi = aiApi;
  }, [aiApi]);
  const [settings, setSettings] = useState<MergeSettings | null>(null);

  useEffect(() => {
    void loadMergeSettings().then((stored) => {
      const override = (key: string, field: keyof MergeSettings) =>
        params.has(key) ? { [field]: params.get(key) === "1" } : {};
      setSettings({
        ...stored,
        ...override("autoApply", "autoApplyNonConflicting"),
        ...override("showBase", "showBase"),
        ...override("collapse", "collapseUnchanged"),
      });
    });
  }, [params]);

  const reanalyzeName = params.get("reanalyze");
  if (!settings) return null;
  performance.mark("mergeiq:analysis-ready");
  return (
    <div style={{ height: "100vh" }}>
      <AiHostContext.Provider value={aiHost}>
        <MergeEditor
          doc={doc}
          settings={settings}
          onSettingsChange={(next) => void saveMergeSettings(next)}
          onSave={
            params.get("failSave") === "1"
              ? () => Promise.reject(new Error("disk full"))
              : recordMockSave
          }
          onCancel={recordMockCancel}
          extensions={params.has("extension") ? [demoExtension] : undefined}
          reanalyze={
            reanalyzeName
              ? () => Promise.resolve(fixture(reanalyzeName))
              : undefined
          }
        />
      </AiHostContext.Provider>
    </div>
  );
}
