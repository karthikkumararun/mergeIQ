import { commands, type IpcError, type PathToken } from "../ipc/bindings";
import { isIpcMock } from "../ipc/mock";
import type { Analysis, WhitespacePolicy } from "../ipc/bindings";
import { DEFAULT_SETTINGS, type MergeSettings } from "./model/types";

const MOCK_KEY = "mergeiq.mock.mergeEditorSettings";

function describeError(error: IpcError): string {
  switch (error.kind) {
    case "Settings":
      return error.message;
    case "NoRepo":
      return "no repository is open";
    case "Git": {
      const g = error.message;
      switch (g.kind) {
        case "CommandFailed":
          return `git ${g.command} failed: ${g.stderr}`;
        case "Unsupported":
          return g.what;
        case "Io":
          return g.message;
        default:
          return g.kind;
      }
    }
  }
}

/** Loads the persisted merge editor preferences (IPC, or localStorage when mocked). */
export async function loadMergeSettings(): Promise<MergeSettings> {
  if (isIpcMock) {
    try {
      const raw = window.localStorage.getItem(MOCK_KEY);
      return { ...DEFAULT_SETTINGS, ...(raw ? JSON.parse(raw) : {}) };
    } catch {
      return DEFAULT_SETTINGS;
    }
  }
  return { ...DEFAULT_SETTINGS, ...(await commands.getMergeEditorSettings()) };
}

/** Persists merge editor preferences. */
export async function saveMergeSettings(
  settings: MergeSettings,
): Promise<void> {
  if (isIpcMock) {
    try {
      window.localStorage.setItem(MOCK_KEY, JSON.stringify(settings));
    } catch {
      // storage unavailable: preferences simply do not persist
    }
    return;
  }
  const result = await commands.updateMergeEditorSettings(settings);
  if (result.status === "error") throw new Error(describeError(result.error));
}

/** `MergeEditor`'s `reanalyze` prop for a conflicted file in the open repository. */
export function reanalyzeViaIpc(
  path: PathToken,
): (policy: WhitespacePolicy) => Promise<Analysis> {
  return async (policy) => {
    const result = await commands.conflictAnalyze(path, policy);
    if (result.status === "error") throw new Error(describeError(result.error));
    return result.data;
  };
}
