import { describe, expect, it } from "vitest";
import {
  cost,
  latency,
  sessionLine,
  tokens,
  totalInput,
  usageLine,
} from "./format";

const usage = (over: Partial<Parameters<typeof usageLine>[1]> = {}) => ({
  inputTokens: 1400,
  outputTokens: 410,
  cacheReadTokens: 4800,
  cacheCreationTokens: 0,
  latencyMs: 3100,
  ...over,
});

describe("format", () => {
  it("abbreviates token counts", () => {
    expect(tokens(0)).toBe("0");
    expect(tokens(999)).toBe("999");
    expect(tokens(1000)).toBe("1k");
    expect(tokens(1300)).toBe("1.3k");
    expect(tokens(19400)).toBe("19.4k");
    expect(tokens(120_000)).toBe("120k");
  });

  it("formats latency and cost", () => {
    expect(latency(80)).toBe("80 ms");
    expect(latency(800)).toBe("0.8 s");
    expect(latency(3100)).toBe("3.1 s");
    expect(cost(0)).toBe("$0.00");
    expect(cost(0.0004)).toBe("<$0.001");
    expect(cost(0.0042)).toBe("$0.004");
    expect(cost(0.04)).toBe("$0.04");
  });

  it("counts cached tokens as input", () => {
    expect(totalInput(usage())).toBe(6200);
    expect(totalInput(usage({ cacheCreationTokens: 100 }))).toBe(6300);
  });

  it("writes the per-request and session lines like the board", () => {
    expect(usageLine("claude-opus-5", usage())).toBe(
      "claude-opus-5 · 6.2k in (4.8k cached) · 410 out · 3.1 s",
    );
    expect(
      usageLine("m", usage({ cacheReadTokens: 0, inputTokens: 900 })),
    ).toBe("m · 900 in · 410 out · 3.1 s");
    expect(sessionLine(3, usage(), 0.04)).toBe(
      "Session: 3 requests · 6.2k in · 410 out · est. $0.04",
    );
    expect(sessionLine(1, usage(), null)).toContain("no price set");
    expect(sessionLine(1, usage(), null)).toContain("1 request ·");
  });
});
