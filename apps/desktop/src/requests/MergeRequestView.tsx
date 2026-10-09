import { useEffect, useState } from "react";
import { commands, type WhitespacePolicy } from "../ipc/bindings";
import { fail, saveAndClose, toMergeDocument } from "./mergeRequest";
import { MergeEditor } from "../merge-editor/MergeEditor";
import {
  describeError,
  loadMergeSettings,
  saveMergeSettings,
} from "../merge-editor/hosts";
import type { MergeDocument, MergeSettings } from "../merge-editor/model/types";
import { useTheme } from "../theme/useTheme";

/**
 * The merge editor for a window opened by `mergeiq merge` / `mergeiq resolve`
 * (`/merge/<id>`). Apply saves through the request (the backend records the exit code
 * for the waiting CLI process) and then closes the window; Cancel just closes it.
 */
export function MergeRequestView({ id }: { id: number }) {
  useTheme();
  const [doc, setDoc] = useState<MergeDocument | null>(null);
  const [settings, setSettings] = useState<MergeSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([commands.mergeRequestLoad(id), loadMergeSettings()]).then(
      ([result, stored]) => {
        if (result.status === "error") {
          setError(describeError(result.error));
          return;
        }
        setDoc(toMergeDocument(result.data));
        setSettings(stored);
      },
    );
  }, [id]);

  if (error) {
    return (
      <p role="alert" style={{ padding: 24, color: "var(--text)" }}>
        {error}
      </p>
    );
  }
  if (!doc || !settings) return null;

  const reanalyze = async (policy: WhitespacePolicy) => {
    const result = await commands.mergeRequestAnalyze(id, policy);
    if (result.status === "error") fail(result.error);
    return result.data;
  };

  return (
    <div style={{ height: "100vh" }}>
      <MergeEditor
        doc={doc}
        settings={settings}
        onSettingsChange={(next) => void saveMergeSettings(next)}
        onSave={(result) => saveAndClose(id, result)}
        onCancel={() => void commands.requestClose(id)}
        reanalyze={reanalyze}
      />
    </div>
  );
}
