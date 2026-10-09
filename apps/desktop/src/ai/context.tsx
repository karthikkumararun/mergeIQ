import { createContext, useContext } from "react";
import type { AiHost } from "../merge-editor/extensions/ai/store";
import { isIpcMock } from "../ipc/mock";
import { ipcAiApi, STANDALONE_SCOPE, type AiApi } from "./api";
import { createMockAiApi } from "./mockApi";

let fallbackApi: AiApi | null = null;

/** The backend used when no host provides one: the real IPC, or an in-memory one in mock runs. */
function defaultApi(): AiApi {
  fallbackApi ??= isIpcMock ? createMockAiApi() : ipcAiApi;
  return fallbackApi;
}

/** A standalone window (command-line merge): one shared consent scope. */
export function standaloneHost(api: AiApi = defaultApi()): AiHost {
  return {
    api,
    repo: null,
    scope: STANDALONE_SCOPE,
    scopeName: "standalone merges",
  };
}

export const AiHostContext = createContext<AiHost | null>(null);

/** The AI backend and consent scope for the merge editors below. */
export function useAiHost(): AiHost {
  const provided = useContext(AiHostContext);
  return provided ?? defaultStandalone();
}

let cachedStandalone: AiHost | null = null;
function defaultStandalone(): AiHost {
  cachedStandalone ??= standaloneHost();
  return cachedStandalone;
}
