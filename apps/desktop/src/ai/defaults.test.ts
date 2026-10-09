import { describe, expect, it } from "vitest";
import {
  defaultProviderSettings,
  full,
  globProblem,
  hasEffort,
  providerOf,
} from "./defaults";

describe("ai defaults", () => {
  it("match the engine's defaults", () => {
    expect(defaultProviderSettings("anthropic")).toEqual({
      baseUrl: "https://api.anthropic.com",
      model: "claude-opus-5",
      effort: "high",
    });
    expect(defaultProviderSettings("ollama").baseUrl).toBe(
      "http://localhost:11434",
    );
    expect(defaultProviderSettings("ollama").model).toBe("qwen2.5-coder");
    expect(defaultProviderSettings("github-models").baseUrl).toBe(
      "https://models.github.ai/inference",
    );
  });

  it("fills every optional field", () => {
    const s = full({});
    expect(s.provider).toBe("anthropic");
    expect(s.confirmed).toBe(false);
    expect(s.context).toEqual({ surroundingLines: 40, tokenBudget: 60_000 });
    expect(s.privacy.excludeGlobs).toEqual([]);
    expect(providerOf(s).model).toBe("claude-opus-5");
    expect(
      providerOf(
        full({
          providers: { openai: { baseUrl: "x", model: "m", effort: "low" } },
        }),
        "openai",
      ).model,
    ).toBe("m");
  });

  it("only some providers have an effort control", () => {
    expect(hasEffort("anthropic")).toBe(true);
    expect(hasEffort("ollama")).toBe(false);
  });

  it("flags patterns that cannot compile", () => {
    expect(globProblem("**/*.pem")).toBeNull();
    expect(globProblem("{a,b}/**")).toBeNull();
    expect(globProblem("  ")).toBe("Enter a pattern");
    expect(globProblem("a[")).toMatch(/Unbalanced/);
    expect(globProblem("a}")).toMatch(/Unbalanced/);
  });
});
