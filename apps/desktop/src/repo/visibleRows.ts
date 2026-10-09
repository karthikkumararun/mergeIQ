import type { ConflictEntry } from "../ipc/bindings";
import { splitPath } from "./describe";
import type { ViewMode } from "./repoStore";

export type Row =
  | { kind: "header"; key: string; dir: string; count: number }
  | { kind: "file"; key: string; entry: ConflictEntry };

/** Filtered, path-sorted rows; in folder view each folder gets a header row. */
export function visibleRows(
  conflicts: ConflictEntry[],
  filter: string,
  view: ViewMode,
): Row[] {
  const needle = filter.trim().toLowerCase();
  const files = conflicts
    .filter((c) => !needle || c.display.toLowerCase().includes(needle))
    .sort((a, b) =>
      a.display < b.display ? -1 : a.display > b.display ? 1 : 0,
    );
  if (view === "flat") {
    return files.map((entry) => ({ kind: "file", key: entry.path, entry }));
  }
  const rows: Row[] = [];
  const counts = new Map<string, number>();
  for (const f of files) {
    const dir = splitPath(f.display).dir;
    counts.set(dir, (counts.get(dir) ?? 0) + 1);
  }
  let current: string | null = null;
  for (const entry of files) {
    const dir = splitPath(entry.display).dir;
    if (dir !== current) {
      current = dir;
      rows.push({
        kind: "header",
        key: `dir:${dir}`,
        dir: dir || "(repository root)",
        count: counts.get(dir) ?? 0,
      });
    }
    rows.push({ kind: "file", key: entry.path, entry });
  }
  return rows;
}
