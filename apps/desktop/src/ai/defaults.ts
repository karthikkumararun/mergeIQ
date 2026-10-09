import type {
  AiSettings,
  Effort,
  ProviderKind,
  ProviderSettings,
} from "../ipc/bindings";

/** The providers offered in the UI, in display order. */
export const PROVIDERS: ProviderKind[] = [
  "anthropic",
  "openai",
  "github-models",
  "ollama",
];

const BASE_URL: Record<ProviderKind, string> = {
  anthropic: "https://api.anthropic.com",
  openai: "https://api.openai.com",
  "github-models": "https://models.github.ai/inference",
  ollama: "http://localhost:11434",
  mock: "mock://",
};

const MODEL: Record<ProviderKind, string> = {
  anthropic: "claude-opus-5",
  openai: "gpt-4.1",
  "github-models": "openai/gpt-4.1",
  ollama: "qwen2.5-coder",
  mock: "mock-1",
};

/** Model ids offered as suggestions (the field is free text). */
export const SUGGESTED_MODELS: Record<ProviderKind, string[]> = {
  anthropic: [
    "claude-opus-5",
    "claude-opus-5-5",
    "claude-sonnet-5",
    "claude-sonnet-5-5",
    "claude-haiku-4-5",
    "claude-haiku-5-5",
  ],
  openai: ["gpt-4.1", "gpt-4o", "gpt-5", "o4-mini"],
  "github-models": ["openai/gpt-4.1", "openai/gpt-4o", "openai/o4-mini"],
  ollama: ["qwen2.5-coder", "llama3.1", "deepseek-coder-v2"],
  mock: ["mock-1"],
};

export const EFFORTS: Effort[] = ["low", "medium", "high", "xhigh", "max"];

/** Providers whose requests have an effort control. */
export function hasEffort(kind: ProviderKind): boolean {
  return kind === "anthropic" || kind === "openai";
}

export function defaultProviderSettings(kind: ProviderKind): ProviderSettings {
  return { baseUrl: BASE_URL[kind], model: MODEL[kind], effort: "high" };
}

/** Defaults filled in for every optional field of the settings. */
export interface FullAiSettings {
  provider: ProviderKind;
  confirmed: boolean;
  noticeAccepted: boolean;
  providers: Record<string, ProviderSettings>;
  privacy: {
    excludeGlobs: string[];
    repos: Record<string, "Allowed" | "Declined">;
  };
  context: { surroundingLines: number; tokenBudget: number };
  prices: NonNullable<AiSettings["prices"]>;
}

export function full(s: AiSettings): FullAiSettings {
  return {
    provider: s.provider ?? "anthropic",
    confirmed: s.confirmed ?? false,
    noticeAccepted: s.noticeAccepted ?? false,
    providers: s.providers ?? {},
    privacy: {
      excludeGlobs: s.privacy?.excludeGlobs ?? [],
      repos: s.privacy?.repos ?? {},
    },
    context: {
      surroundingLines: s.context?.surroundingLines ?? 40,
      tokenBudget: s.context?.tokenBudget ?? 60_000,
    },
    prices: s.prices ?? { entries: {} },
  };
}

export function providerOf(
  s: FullAiSettings,
  kind: ProviderKind = s.provider,
): ProviderSettings {
  return { ...defaultProviderSettings(kind), ...s.providers[kind] };
}

/** A basic check that `glob` can be compiled (balanced brackets and braces, not empty). */
export function globProblem(glob: string): string | null {
  const g = glob.trim();
  if (!g) return "Enter a pattern";
  for (const [open, close] of [
    ["[", "]"],
    ["{", "}"],
  ]) {
    let depth = 0;
    for (const c of g) {
      if (c === open) depth += 1;
      if (c === close) depth -= 1;
      if (depth < 0) break;
    }
    if (depth !== 0) return `Unbalanced ${open}${close} in the pattern`;
  }
  return null;
}
