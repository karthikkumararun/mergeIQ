import type { Analysis, StructuralResolve } from "./bindings";
import { fixture } from "../merge-editor/__fixtures__";
import { structuralFixtures } from "../merge-editor/__fixtures__/structural";
import { syntheticAnalysis } from "../merge-editor/__fixtures__/synthetic";
import type { MergeDocument, SaveResult } from "../merge-editor/model/types";

/** True when the app runs against fixtures instead of the Tauri backend (`VITE_IPC_MOCK=1`). */
export const isIpcMock = import.meta.env.VITE_IPC_MOCK === "1";

declare global {
  interface Window {
    /** Saves captured by the mock host, for Playwright assertions. */
    __mergeiqSaves?: SaveResult[];
    __mergeiqCancels?: number;
  }
}

const LANGUAGE_BY_FIXTURE: Record<string, string> = {
  "java-sample": "Main.java",
  "kotlin-sample": "Main.kt",
  "python-sample": "main.py",
};

/** `synthetic-<lines>-<every>[-shift]` builds a large generated file; anything else is an exported fixture. */
function analysisFor(name: string) {
  const structural = structuralFixtures[name];
  if (structural) return structural.analysis;
  const m = name.match(/^synthetic-(\d+)-(\d+)(-shift)?$/);
  return m
    ? syntheticAnalysis(Number(m[1]), Number(m[2]), !!m[3])
    : fixture(name);
}

/** Builds a `MergeDocument` from an engine-exported fixture, with placeholder labels. */
export function mockMergeDocument(name: string): MergeDocument {
  const analysis = analysisFor(name);
  const displayPath =
    structuralFixtures[name]?.path ??
    LANGUAGE_BY_FIXTURE[name] ??
    `${name}.txt`;
  return {
    pathToken: `mock:${name}`,
    displayPath,
    analysis,
    labels: {
      left: {
        role: "Your branch",
        refName: "main",
        shortSha: "a1b2c3d",
        subject: "Refactor parser",
        author: "Ada",
        gitTerm: "ours",
      },
      right: {
        role: "Incoming branch",
        refName: "feature",
        shortSha: "e4f5a6b",
        subject: "Add caching layer",
        author: "Linus",
        gitTerm: "theirs",
      },
    },
    context: {
      ours: [
        {
          sha: "a1b2c3d0",
          shortSha: "a1b2c3d",
          subject: "Refactor parser",
          author: "Ada",
          date: "2026-10-01T10:00:00Z",
        },
        {
          sha: "9f8e7d6c",
          shortSha: "9f8e7d6",
          subject: "Fix lexer",
          author: "Ada",
          date: "2026-09-28T10:00:00Z",
        },
      ],
      theirs: [
        {
          sha: "e4f5a6b0",
          shortSha: "e4f5a6b",
          subject: "Add caching layer",
          author: "Linus",
          date: "2026-10-02T10:00:00Z",
        },
        {
          sha: "5c4b3a29",
          shortSha: "5c4b3a2",
          subject: "Tune cache size",
          author: "Linus",
          date: "2026-09-30T10:00:00Z",
        },
      ],
    },
  };
}

export function recordMockSave(result: SaveResult): Promise<void> {
  (window.__mergeiqSaves ??= []).push(result);
  return Promise.resolve();
}

export function recordMockCancel(): void {
  window.__mergeiqCancels = (window.__mergeiqCancels ?? 0) + 1;
}

/**
 * The mocked `structural_resolve`: returns the proposals the engine computed for the
 * matching structural fixture (none for other files). `?structural=slow|timeout|off`
 * simulates a slow backend, a timeout, or no structural support.
 */
export async function mockStructuralResolve(
  path: string,
  analysis: Analysis,
): Promise<StructuralResolve> {
  const mode = new URLSearchParams(window.location.search).get("structural");
  if (mode === "slow") await new Promise((r) => setTimeout(r, 1500));
  if (mode === "timeout") return { outcome: "TimedOut", elapsed_ms: 2000 };
  if (mode === "off") return { outcome: "Unsupported", elapsed_ms: 0 };
  const match = Object.values(structuralFixtures).find(
    (f) => f.path === path && f.analysis.base.text === analysis.base.text,
  );
  return match?.resolve ?? { outcome: { Proposals: [] }, elapsed_ms: 0 };
}
