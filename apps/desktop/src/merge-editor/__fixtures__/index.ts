import type { Analysis } from "../../ipc/bindings";

const modules = import.meta.glob<Analysis>("./*.json", { eager: true, import: "default" });

/** Engine-produced `Analysis` fixtures keyed by file name without extension. */
export const fixtures: Record<string, Analysis> = Object.fromEntries(
  Object.entries(modules).map(([path, analysis]) => [path.replace(/^\.\/(.*)\.json$/, "$1"), analysis]),
);

export function fixture(name: string): Analysis {
  const a = fixtures[name];
  if (!a) throw new Error(`unknown merge-editor fixture: ${name}`);
  return a;
}
