import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  commands,
  events,
  type AiEstimate,
  type AiExplained,
  type AiFailure,
  type AiPreview,
  type AiSettings,
  type AiStatus,
  type AiSuggested,
  type AiTask,
  type ChunkSpans,
  type ContextInput,
  type KeyInfo,
  type ProviderKind,
  type ProviderSettings,
  type RepoDecision,
  type SessionUsage,
  type TestReport,
} from "../ipc/bindings";

export type {
  AiEstimate,
  AiExplained,
  AiFailure,
  AiPreview,
  AiSettings,
  AiStatus,
  AiSuggested,
  AiTask,
  ChunkSpans,
  ContextInput,
  KeyInfo,
  ProviderKind,
  ProviderSettings,
  RepoDecision,
  SessionUsage,
  TestReport,
};

/** Consent key for windows that are not part of a repository (command-line merges). */
export const STANDALONE_SCOPE = "standalone";

/** The wording the UI uses for every failure. */
export function failureMessage(f: AiFailure): string {
  switch (f.code) {
    case "notConfigured":
      return "AI is not set up yet.";
    case "noticeRequired":
      return "Accept the data-sharing notice to use AI.";
    case "repoUnasked":
      return "AI is not enabled for this repository yet.";
    case "repoDeclined":
      return "AI is turned off for this repository.";
    case "excluded":
      return "File excluded from AI by your settings";
    case "auth":
      return `The provider rejected the credentials: ${f.message}`;
    case "rateLimited":
      return f.retryAfterSecs !== null
        ? `The provider is rate limiting requests. Try again in ${f.retryAfterSecs} s.`
        : "The provider is rate limiting requests. Try again shortly.";
    case "refused":
      return f.category
        ? `The model declined this request (${f.category})`
        : "The model declined this request";
    case "truncated":
      return "The response was cut off at the output limit.";
    case "schema":
      return `The model's answer was not in the expected format: ${f.message}`;
    case "network":
      return `Could not reach the provider: ${f.message}`;
    case "http":
      return `${f.message} (HTTP ${f.status})`;
    case "cancelled":
      return "Cancelled";
    case "internal":
      return f.message;
  }
}

/** A failed AI call, carrying the structured failure. */
export class AiRequestError extends Error {
  constructor(readonly failure: AiFailure) {
    super(failureMessage(failure));
    this.name = "AiRequestError";
  }
}

/** The failure behind any thrown value. */
export function failureOf(e: unknown): AiFailure {
  if (e instanceof AiRequestError) return e.failure;
  return {
    code: "internal",
    message: e instanceof Error ? e.message : String(e),
  };
}

/** The backend calls behind every AI surface, so tests can swap them. */
export interface AiApi {
  getSettings(): Promise<AiSettings>;
  updateSettings(settings: AiSettings): Promise<AiSettings>;
  /** Records (or with `null`, clears) the decision for a repository. */
  setRepoDecision(
    scope: string,
    decision: RepoDecision | null,
  ): Promise<AiSettings>;
  keyInfo(provider: ProviderKind): Promise<KeyInfo>;
  setKey(provider: ProviderKind, key: string): Promise<KeyInfo>;
  deleteKey(provider: ProviderKind): Promise<KeyInfo>;
  testConnection(
    provider: ProviderKind,
    settings: ProviderSettings,
    key: string | null,
  ): Promise<TestReport>;
  usage(): Promise<SessionUsage>;
  resetUsage(): Promise<void>;
  status(repo: number | null, path: string): Promise<AiStatus>;
  preview(
    repo: number | null,
    input: ContextInput,
    task: AiTask,
  ): Promise<AiPreview>;
  estimate(
    repo: number | null,
    input: ContextInput,
    chunks: ChunkSpans[],
  ): Promise<AiEstimate>;
  explain(
    repo: number | null,
    requestId: string,
    input: ContextInput,
    onDelta: (text: string) => void,
  ): Promise<AiExplained>;
  suggest(
    repo: number | null,
    requestId: string,
    input: ContextInput,
  ): Promise<AiSuggested>;
  cancel(requestId: string): Promise<void>;
  /** Shows Settings in the home window. */
  openSettings(section: string): Promise<void>;
}

async function call<T>(
  p: Promise<{ status: "ok"; data: T } | { status: "error"; error: AiFailure }>,
): Promise<T> {
  const r = await p;
  if (r.status === "error") throw new AiRequestError(r.error);
  return r.data;
}

export const ipcAiApi: AiApi = {
  getSettings: () => commands.aiGetSettings(),
  updateSettings: (s) => call(commands.aiUpdateSettings(s)),
  setRepoDecision: (scope, d) => call(commands.aiSetRepoDecision(scope, d)),
  keyInfo: (p) => call(commands.aiKeyInfo(p)),
  setKey: (p, key) => call(commands.aiSetKey(p, key)),
  deleteKey: (p) => call(commands.aiDeleteKey(p)),
  testConnection: (p, s, key) => call(commands.aiTestConnection(p, s, key)),
  usage: () => commands.aiUsage(),
  resetUsage: () => commands.aiResetUsage().then(() => undefined),
  status: (repo, path) => call(commands.aiStatus(repo, path)),
  preview: (repo, input, task) => call(commands.aiPreview(repo, input, task)),
  estimate: (repo, input, chunks) =>
    call(commands.aiEstimate(repo, input, chunks)),
  explain: async (repo, requestId, input, onDelta) => {
    const stop = await events.aiDelta(getCurrentWindow()).listen((e) => {
      if (e.payload.requestId === requestId) onDelta(e.payload.text);
    });
    try {
      return await call(commands.aiExplain(repo, requestId, input));
    } finally {
      stop();
    }
  },
  suggest: (repo, requestId, input) =>
    call(commands.aiSuggest(repo, requestId, input)),
  cancel: async (requestId) => {
    await commands.aiCancel(requestId);
  },
  openSettings: async (section) => {
    await call(commands.aiOpenSettings(section));
  },
};

let counter = 0;
/** A request id unique within this window. */
export function newRequestId(): string {
  counter += 1;
  return `${Date.now().toString(36)}-${counter}`;
}
