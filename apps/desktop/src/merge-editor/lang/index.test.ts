import { describe, expect, it } from "vitest";
import { languageForPath, languageLabel, loadLanguage } from "./index";

describe("Syntax highlighting", () => {
  it.each([
    ["Main.java", "java"],
    ["a/b/tool.py", "python"],
    ["stubs.pyi", "python"],
    ["Main.kt", "kotlin"],
    ["build.gradle.kts", "kotlin"],
    ["ci.yml", "yaml"],
    ["ci.yaml", "yaml"],
    ["package.json", "json"],
    ["tsconfig.jsonc", "json"],
    ["index.js", "javascript"],
    ["index.mjs", "javascript"],
    ["index.cjs", "javascript"],
    ["App.jsx", "javascript"],
    ["index.ts", "typescript"],
    ["index.mts", "typescript"],
    ["index.cts", "typescript"],
    ["App.tsx", "typescript"],
    ["main.go", "go"],
  ])("Language mapping for %s", (path, id) => {
    expect(languageForPath(path)).toBe(id);
  });

  it("Falls back to plain text", () => {
    expect(languageForPath("README")).toBeNull();
    expect(languageForPath("notes.txt")).toBeNull();
    expect(languageLabel("notes.txt")).toBe("Plain text");
  });

  it("Kotlin file", async () => {
    expect(languageLabel("Main.kt")).toBe("Kotlin");
    const ext = await loadLanguage("kotlin");
    expect(ext).toBeTruthy();
  });

  it("Every mapped language loads", async () => {
    for (const id of [
      "java",
      "python",
      "kotlin",
      "yaml",
      "json",
      "javascript",
      "typescript",
      "go",
    ] as const) {
      expect(await loadLanguage(id)).toBeTruthy();
    }
  });
});
