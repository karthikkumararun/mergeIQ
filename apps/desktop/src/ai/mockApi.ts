import type {
  AiSettings,
  KeyInfo,
  ProviderKind,
  ProviderSettings,
  SessionUsage,
  Usage,
} from "../ipc/bindings";
import {
  AiRequestError,
  STANDALONE_SCOPE,
  type AiApi,
  type AiEstimate,
  type AiFailure,
  type AiStatus,
  type AiSuggested,
  type ContextInput,
} from "./api";

/** How the in-memory backend behaves (Playwright scenarios and unit tests). */
export interface MockAiOptions {
  /** A provider is chosen and has a key. Default true. */
  configured?: boolean;
  /** The data-sharing notice was accepted. Default true. */
  notice?: boolean;
  /** The decision for the current scope. Default "allowed". */
  repo?: "allowed" | "unasked" | "declined";
  provider?: ProviderKind;
  model?: string;
  /** Milliseconds between streamed words, and before a suggestion returns. Default 0. */
  delayMs?: number;
  /** Fail suggestions (and explanations) with this. */
  fail?: AiFailure;
  /** Mark suggestions with the syntax warning. */
  syntaxWarning?: boolean;
  /** Confidence of suggestions. Default "high". */
  confidence?: "high" | "medium" | "low";
  /** The key that "Test connection" rejects. Default "bad". */
  badKey?: string;
  /** Other repositories with a stored decision (Settings › Privacy). */
  repos?: Record<string, "Allowed" | "Declined">;
  /** A key is stored for the provider. Defaults to `configured`. */
  hasKey?: boolean;
  /** The consent key the requests are gated on. Default: the standalone scope. */
  scope?: string;
}

const DEFAULT_GLOBS = ["**/.env*", "**/*secret*", "**/*.pem", "**/*.key"];

function globToRegExp(glob: string): RegExp {
  let out = "";
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i];
    if (c === "*" && glob[i + 1] === "*") {
      i++;
      if (glob[i + 1] === "/") {
        i++;
        out += "(?:.*/)?";
      } else out += ".*";
    } else if (c === "*") out += "[^/]*";
    else if (c === "?") out += "[^/]";
    else out += c.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${out}$`, "i");
}

/** Whether `path` matches any exclusion glob. */
export function isExcluded(path: string, globs: string[]): boolean {
  const p = path.replace(/\\/g, "/");
  return globs.some((g) => globToRegExp(g).test(p));
}

const EXPLANATION =
  "main changed this region one way and feature/checkout-v2 changed it another. " +
  "Both edits touch the same lines, so they clash. A correct merge keeps both changes.";

const ZERO: Usage = {
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
  latencyMs: 0,
};

function lines(text: string): string[] {
  const parts = text.split("\n");
  if (parts[parts.length - 1] === "") parts.pop();
  return parts;
}

/** The mock's suggestion: the left lines followed by the right lines. */
export function mockResolution(input: ContextInput): string {
  const slice = (text: string, s: { start: number; end: number }) =>
    lines(text)
      .slice(s.start, s.end)
      .map((l) => `${l}\n`)
      .join("");
  return (
    slice(input.left.text, input.chunk.left) +
    slice(input.right.text, input.chunk.right)
  );
}

export interface MockAiApi extends AiApi {
  /** Recorded calls, for assertions. */
  calls: {
    explain: number;
    suggest: number;
    /** Requests that passed the gates and would have reached the provider. */
    sent: number;
    cancel: string[];
    preview: number;
    openSettings: string[];
    inputs: ContextInput[];
  };
  /** The settings as the backend holds them. */
  readonly settings: AiSettings;
}

/** An in-memory AI backend. */
export function createMockAiApi(options: MockAiOptions = {}): MockAiApi {
  const scopeKey = options.scope ?? STANDALONE_SCOPE;
  const o = {
    configured: true,
    notice: true,
    repo: "allowed" as const,
    provider: "anthropic" as ProviderKind,
    delayMs: 0,
    confidence: "high" as const,
    ...options,
  };
  let settings: AiSettings = {
    provider: o.provider,
    confirmed: o.configured,
    noticeAccepted: o.notice,
    providers: {},
    privacy: {
      excludeGlobs: [...DEFAULT_GLOBS],
      repos: {
        ...(o.repo === "unasked"
          ? {}
          : {
              [scopeKey]: o.repo === "allowed" ? "Allowed" : "Declined",
            }),
        ...options.repos,
      },
    },
    context: { surroundingLines: 40, tokenBudget: 60_000 },
    prices: {
      entries: {
        "claude-opus-5": {
          input: 5,
          output: 25,
          cacheRead: 0.5,
          cacheWrite: 6.25,
        },
      },
    },
  };
  const keys = new Map<ProviderKind, string>();
  if (options.hasKey ?? o.configured) keys.set(o.provider, "sk-ant-mock-a9F2");
  const records: SessionUsage["records"] = [];
  const cancelled = new Set<string>();
  const calls: MockAiApi["calls"] = {
    explain: 0,
    suggest: 0,
    sent: 0,
    cancel: [],
    preview: 0,
    openSettings: [],
    inputs: [],
  };

  const providerSettings = (kind: ProviderKind): ProviderSettings => ({
    baseUrl:
      kind === "ollama"
        ? "http://localhost:11434"
        : kind === "openai"
          ? "https://api.openai.com"
          : kind === "github-models"
            ? "https://models.github.ai/inference"
            : "https://api.anthropic.com",
    model:
      o.model && kind === o.provider
        ? o.model
        : kind === "ollama"
          ? "qwen2.5-coder"
          : kind === "openai"
            ? "gpt-4.1"
            : kind === "github-models"
              ? "openai/gpt-4.1"
              : "claude-opus-5",
    effort: "high",
    ...settings.providers?.[kind],
  });
  const active = () => providerSettings(settings.provider ?? "anthropic");
  const globs = () => settings.privacy?.excludeGlobs ?? DEFAULT_GLOBS;
  const decision = () => settings.privacy?.repos?.[scopeKey];
  const keyPresent = () => keys.has(settings.provider ?? "anthropic");

  const blocked = (path: string): AiFailure | null => {
    const kind = settings.provider ?? "anthropic";
    const configured =
      !!settings.confirmed && (kind === "ollama" || keyPresent());
    if (!configured) return { code: "notConfigured" };
    if (!settings.noticeAccepted) return { code: "noticeRequired" };
    if (!decision()) return { code: "repoUnasked" };
    if (decision() === "Declined") return { code: "repoDeclined" };
    if (isExcluded(path, globs())) return { code: "excluded" };
    return null;
  };

  const wait = (ms: number) =>
    ms > 0 ? new Promise<void>((r) => setTimeout(r, ms)) : Promise.resolve();

  const record = (usage: Usage) => {
    const model = active().model;
    records.push({
      provider: settings.provider ?? "anthropic",
      model,
      usage,
      cost: usage.outputTokens * 0.00002 + usage.inputTokens * 0.000005,
    });
    const totals = records.reduce<Usage>(
      (acc, r) => ({
        inputTokens: acc.inputTokens + r.usage.inputTokens,
        outputTokens: acc.outputTokens + r.usage.outputTokens,
        cacheReadTokens: acc.cacheReadTokens + r.usage.cacheReadTokens,
        cacheCreationTokens:
          acc.cacheCreationTokens + r.usage.cacheCreationTokens,
        latencyMs: acc.latencyMs + r.usage.latencyMs,
      }),
      ZERO,
    );
    const cost = records.reduce((a, r) => a + (r.cost ?? 0), 0);
    return {
      record: records[records.length - 1],
      session: { requests: records.length, totals, cost },
    };
  };

  const usageFor = (): Usage => ({
    inputTokens: 1400,
    outputTokens: 410,
    cacheReadTokens: records.length > 0 ? 4800 : 0,
    cacheCreationTokens: records.length > 0 ? 0 : 4800,
    latencyMs: 3100,
  });

  const guard = (path: string) => {
    const b = blocked(path);
    if (b) throw new AiRequestError(b);
  };

  const keyInfo = (p: ProviderKind): KeyInfo => {
    const key = keys.get(p);
    return {
      present: key !== undefined,
      last4: key ? key.slice(-4) : null,
      storeName: "macOS Keychain",
    };
  };

  const api: MockAiApi = {
    calls,
    get settings() {
      return settings;
    },
    getSettings: () => Promise.resolve(settings),
    updateSettings: (s) => {
      settings = s;
      return Promise.resolve(settings);
    },
    setRepoDecision: (scope, d) => {
      const repos = { ...settings.privacy?.repos };
      if (d === null) delete repos[scope];
      else repos[scope] = d;
      settings = {
        ...settings,
        privacy: { ...settings.privacy, repos },
      };
      return Promise.resolve(settings);
    },
    keyInfo: (p) => Promise.resolve(keyInfo(p)),
    setKey: (p, key) => {
      keys.set(p, key);
      return Promise.resolve(keyInfo(p));
    },
    deleteKey: (p) => {
      keys.delete(p);
      return Promise.resolve(keyInfo(p));
    },
    testConnection: async (p, s, key) => {
      await wait(o.delayMs);
      const used = key ?? keys.get(p) ?? null;
      if (p !== "ollama" && used === null)
        throw new AiRequestError({ code: "notConfigured" });
      if (used !== null && used === (o.badKey ?? "bad"))
        throw new AiRequestError({
          code: "auth",
          message: "invalid x-api-key",
        });
      return { model: s.model, latencyMs: 800 };
    },
    usage: () =>
      Promise.resolve({
        requests: records.length,
        totals: records.reduce<Usage>(
          (acc, r) => ({
            inputTokens: acc.inputTokens + r.usage.inputTokens,
            outputTokens: acc.outputTokens + r.usage.outputTokens,
            cacheReadTokens: acc.cacheReadTokens + r.usage.cacheReadTokens,
            cacheCreationTokens:
              acc.cacheCreationTokens + r.usage.cacheCreationTokens,
            latencyMs: acc.latencyMs + r.usage.latencyMs,
          }),
          ZERO,
        ),
        cost: records.length
          ? records.reduce((a, r) => a + (r.cost ?? 0), 0)
          : null,
        records: [...records],
      }),
    resetUsage: () => {
      records.length = 0;
      return Promise.resolve();
    },
    status: (_repo, path): Promise<AiStatus> =>
      Promise.resolve({
        blocked: blocked(path),
        provider: settings.provider ?? "anthropic",
        model: active().model,
        effort: active().effort,
        local: (settings.provider ?? "anthropic") === "ollama",
        storeName: "macOS Keychain",
      }),
    preview: async (_repo, input, task) => {
      calls.preview += 1;
      if (isExcluded(input.path, globs()))
        throw new AiRequestError({ code: "excluded" });
      return {
        text:
          `You are the conflict-resolution assistant built into MergeIQ…\n\n` +
          `<file_context>\npath: ${input.path}\nLeft is "${input.left.label}". Right is "${input.right.label}".\n</file_context>\n\n` +
          `<conflict>\n<left label="${input.left.label}">\n${mockResolution(input)}</left>\n</conflict>\n\n` +
          `<task>\n${task === "explain" ? "Explain this conflict…" : "Resolve this conflict…"}\n</task>`,
        estimatedTokens: 6200,
        notes: [],
        overBudget: false,
        destination:
          (settings.provider ?? "anthropic") === "ollama"
            ? "localhost:11434"
            : "api.anthropic.com",
      };
    },
    estimate: (_repo, _input, chunks): Promise<AiEstimate> => {
      const perChunk = chunks.map(() => 6000);
      return Promise.resolve({
        perChunk,
        totalTokens: perChunk.reduce((a, b) => a + b, 0),
        overBudget: false,
        provider: settings.provider ?? "anthropic",
        model: active().model,
        inputCost: 0.09,
      });
    },
    explain: async (_repo, requestId, input, onDelta) => {
      calls.explain += 1;
      calls.inputs.push(input);
      guard(input.path);
      calls.sent += 1;
      if (o.fail) throw new AiRequestError(o.fail);
      const words = EXPLANATION.split(/(?<= )/);
      let text = "";
      for (const w of words) {
        await wait(o.delayMs);
        if (cancelled.has(requestId))
          throw new AiRequestError({ code: "cancelled" });
        text += w;
        onDelta(w);
      }
      const { record: rec, session } = record(usageFor());
      return { text, record: rec, session, notes: [] };
    },
    suggest: async (_repo, requestId, input): Promise<AiSuggested> => {
      calls.suggest += 1;
      calls.inputs.push(input);
      guard(input.path);
      calls.sent += 1;
      await wait(o.delayMs);
      if (cancelled.has(requestId))
        throw new AiRequestError({ code: "cancelled" });
      if (o.fail) throw new AiRequestError(o.fail);
      const { record: rec, session } = record(usageFor());
      return {
        checked: {
          suggestion: {
            resolution: mockResolution(input),
            explanation:
              "main and feature/checkout-v2 edited the same lines. This keeps both sides' lines, left first.",
            confidence: o.confidence,
            strategy: "both",
            risks: [
              "Callers on main may rely on the old behaviour and will need updating.",
              "Both changes now run together, which can shift results slightly.",
            ],
          },
          syntaxWarning: o.syntaxWarning ? "Suggestion may not compile" : null,
          notes: [],
        },
        record: rec,
        session,
        notes: [],
        estimatedTokens: 6200,
      };
    },
    cancel: (requestId) => {
      calls.cancel.push(requestId);
      cancelled.add(requestId);
      return Promise.resolve();
    },
    openSettings: (section) => {
      calls.openSettings.push(section);
      return Promise.resolve();
    },
  };
  return api;
}

/** `?ai=<scenario>` picks how the mock AI backend behaves. */
export function mockAiOptionsFromParams(
  params: URLSearchParams,
): MockAiOptions {
  const delay = Number(params.get("aiDelay") ?? 0) || 0;
  switch (params.get("ai") ?? "ready") {
    case "unconfigured":
      return { configured: false };
    case "notice":
      return { notice: false, delayMs: delay };
    case "unasked":
      return { repo: "unasked", delayMs: delay };
    case "declined":
      return { repo: "declined" };
    case "ollama-notice":
      return { provider: "ollama", model: "qwen2.5-coder", notice: false };
    case "ollama":
      return { provider: "ollama", model: "qwen2.5-coder", delayMs: delay };
    case "refused":
      return {
        fail: { code: "refused", category: "cyber", explanation: null },
        delayMs: delay,
      };
    case "truncated":
      return { fail: { code: "truncated" }, delayMs: delay };
    case "error":
      return {
        fail: { code: "network", message: "connection refused" },
        delayMs: delay,
      };
    case "syntax":
      return { syntaxWarning: true, confidence: "low", delayMs: delay };
    case "medium":
      return { confidence: "medium", delayMs: delay };
    default:
      return { delayMs: delay };
  }
}
