import { describe, expect, it } from "vitest";
// @ts-expect-error node builtins are untyped here (no @types/node); vitest runs in Node
import { readFileSync } from "node:fs";

// Vite's CSS pipeline would turn a `?raw` import into an empty module under vitest.
const css: string = readFileSync("src/theme/tokens.css", "utf8");

function themeTokens(theme: "dark" | "light"): Record<string, string> {
  const block = css.match(
    new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([\\s\\S]*?)\\n\\}`),
  );
  if (!block) throw new Error(`theme ${theme} not found`);
  const out: Record<string, string> = {};
  for (const m of block[1].matchAll(/--([a-z0-9-]+):\s*(#[0-9a-fA-F]{6})/g))
    out[m[1]] = m[2];
  return out;
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe.each(["dark", "light"] as const)(
  "Chunk highlighting (%s theme)",
  (theme) => {
    const t = themeTokens(theme);

    it.each(["ins", "mod", "con", "res"])(
      "%s: code text stays readable on the highlight",
      (kind) => {
        expect(contrast(t.code, t[`${kind}-bg`])).toBeGreaterThanOrEqual(4.5);
        expect(contrast(t.code, t[`${kind}-em`])).toBeGreaterThanOrEqual(4.5);
      },
    );

    it.each(["ins", "mod", "con", "res"])(
      "%s: gutter mark is at least 3:1 against the pane background",
      (kind) => {
        expect(contrast(t[`${kind}-fg`], t.bg)).toBeGreaterThanOrEqual(3);
        expect(
          contrast(t[`${kind}-fg`], t[`${kind}-bg`]),
        ).toBeGreaterThanOrEqual(3);
      },
    );
  },
);
