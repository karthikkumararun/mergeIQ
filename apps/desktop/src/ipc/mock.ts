import { fixture } from "../merge-editor/__fixtures__";
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

/** Builds a `MergeDocument` from an engine-exported fixture, with placeholder labels. */
export function mockMergeDocument(name: string): MergeDocument {
  const analysis = fixture(name);
  const displayPath = LANGUAGE_BY_FIXTURE[name] ?? `${name}.txt`;
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
