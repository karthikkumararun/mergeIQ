import type {
  Confidence,
  ProviderKind,
  Strategy,
  Usage,
} from "../ipc/bindings";

/** `6.2k`, `410`, `1.3k`. */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  const k = n / 1000;
  return `${k >= 100 ? Math.round(k) : k.toFixed(1).replace(/\.0$/, "")}k`;
}

/** `3.1 s`, `0.8 s`, `120 ms`. */
export function latency(ms: number): string {
  return ms < 100 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

/** `$0.04`, `$0.003`, `<$0.001`. */
export function cost(dollars: number): string {
  if (dollars <= 0) return "$0.00";
  if (dollars < 0.001) return "<$0.001";
  if (dollars < 0.01) return `$${dollars.toFixed(3)}`;
  return `$${dollars.toFixed(2)}`;
}

/** Total input including cached tokens. */
export function totalInput(u: Usage): number {
  return u.inputTokens + u.cacheReadTokens + u.cacheCreationTokens;
}

/** `6.2k in (4.8k cached) · 410 out · 3.1 s` */
export function usageLine(model: string, u: Usage): string {
  const input = totalInput(u);
  const cached = u.cacheReadTokens;
  return [
    model,
    `${tokens(input)} in${cached > 0 ? ` (${tokens(cached)} cached)` : ""}`,
    `${tokens(u.outputTokens)} out`,
    latency(u.latencyMs),
  ].join(" · ");
}

/** `Session: 3 requests · 19.4k in · 1.3k out · est. $0.04` */
export function sessionLine(
  requests: number,
  totals: Usage,
  estimate: number | null,
): string {
  return [
    `Session: ${requests} ${requests === 1 ? "request" : "requests"}`,
    `${tokens(totalInput(totals))} in`,
    `${tokens(totals.outputTokens)} out`,
    estimate !== null ? `est. ${cost(estimate)}` : "no price set",
  ].join(" · ");
}

export const CONFIDENCE_LABEL: Record<Confidence, string> = {
  high: "High confidence",
  medium: "Medium confidence",
  low: "Low confidence",
};

export const STRATEGY_LABEL: Record<Strategy, string> = {
  left: "left",
  right: "right",
  both: "both",
  combined: "combined",
  new: "new",
};

/** How a provider is named in the UI. */
export const PROVIDER_NAME: Record<ProviderKind, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  "github-models": "GitHub Models",
  ollama: "Ollama (local)",
  mock: "Mock provider",
};
