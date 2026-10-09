import { StreamLanguage, type LanguageSupport } from "@codemirror/language";
import type { Extension } from "@codemirror/state";

export type LanguageId =
  | "java"
  | "python"
  | "kotlin"
  | "yaml"
  | "json"
  | "javascript"
  | "typescript"
  | "go";

const EXTENSIONS: Record<string, LanguageId> = {
  java: "java",
  py: "python",
  pyi: "python",
  kt: "kotlin",
  kts: "kotlin",
  yml: "yaml",
  yaml: "yaml",
  json: "json",
  jsonc: "json",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascript",
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "typescript",
  go: "go",
};

export const LANGUAGE_LABELS: Record<LanguageId, string> = {
  java: "Java",
  python: "Python",
  kotlin: "Kotlin",
  yaml: "YAML",
  json: "JSON",
  javascript: "JavaScript",
  typescript: "TypeScript",
  go: "Go",
};

/** Language for a path (or explicit hint such as `Main.kt`), `null` for plain text. */
export function languageForPath(path: string): LanguageId | null {
  const name = path.split(/[\\/]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  if (dot < 0) return null;
  return EXTENSIONS[name.slice(dot + 1).toLowerCase()] ?? null;
}

export function languageLabel(path: string): string {
  const id = languageForPath(path);
  return id ? LANGUAGE_LABELS[id] : "Plain text";
}

/** Lazily imports and builds the CodeMirror extension for a language. */
export async function loadLanguage(id: LanguageId): Promise<Extension> {
  switch (id) {
    case "java":
      return (await import("@codemirror/lang-java")).java();
    case "python":
      return (await import("@codemirror/lang-python")).python();
    case "yaml":
      return (await import("@codemirror/lang-yaml")).yaml();
    case "json":
      return (await import("@codemirror/lang-json")).json();
    case "javascript":
      return (await import("@codemirror/lang-javascript")).javascript({
        jsx: true,
      });
    case "typescript":
      return (await import("@codemirror/lang-javascript")).javascript({
        jsx: true,
        typescript: true,
      });
    case "kotlin": {
      const { kotlin } = await import("@codemirror/legacy-modes/mode/clike");
      return StreamLanguage.define(kotlin);
    }
    case "go": {
      const { go } = await import("@codemirror/legacy-modes/mode/go");
      return StreamLanguage.define(go);
    }
  }
}

export type { LanguageSupport };
