import { beforeEach, describe, expect, it, vi } from "vitest";
import { fixture } from "./__fixtures__";

vi.mock("../ipc/mock", () => ({ isIpcMock: false }));
const getMergeEditorSettings = vi.fn();
const updateMergeEditorSettings = vi.fn();
const conflictAnalyze = vi.fn();
vi.mock("../ipc/bindings", () => ({
  commands: {
    getMergeEditorSettings: (...a: unknown[]) => getMergeEditorSettings(...a),
    updateMergeEditorSettings: (...a: unknown[]) =>
      updateMergeEditorSettings(...a),
    conflictAnalyze: (...a: unknown[]) => conflictAnalyze(...a),
  },
}));

import { loadMergeSettings, reanalyzeViaIpc, saveMergeSettings } from "./hosts";
import { DEFAULT_SETTINGS } from "./model/types";

describe("Settings persistence", () => {
  beforeEach(() => vi.clearAllMocks());

  it("fills missing fields with defaults", async () => {
    getMergeEditorSettings.mockResolvedValue({ showBase: true });
    expect(await loadMergeSettings()).toEqual({
      ...DEFAULT_SETTINGS,
      showBase: true,
    });
  });

  it("saves through the backend", async () => {
    updateMergeEditorSettings.mockResolvedValue({ status: "ok", data: {} });
    await saveMergeSettings({ ...DEFAULT_SETTINGS, showBase: true });
    expect(updateMergeEditorSettings).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      showBase: true,
    });
  });

  it("surfaces save errors", async () => {
    updateMergeEditorSettings.mockResolvedValue({
      status: "error",
      error: { kind: "Settings", message: "disk full" },
    });
    await expect(saveMergeSettings(DEFAULT_SETTINGS)).rejects.toThrow(
      "disk full",
    );
  });
});

describe("Whitespace policy switch", () => {
  it("re-analyses through conflict_analyze", async () => {
    const analysis = fixture("simple-conflict");
    conflictAnalyze.mockResolvedValue({ status: "ok", data: analysis });
    const reanalyze = reanalyzeViaIpc(3, "token");
    expect(await reanalyze("TrimTrailing")).toBe(analysis);
    expect(conflictAnalyze).toHaveBeenCalledWith(3, "token", "TrimTrailing");
  });

  it("reports git failures", async () => {
    conflictAnalyze.mockResolvedValue({
      status: "error",
      error: {
        kind: "Git",
        message: { kind: "Unsupported", what: "binary file" },
      },
    });
    await expect(reanalyzeViaIpc(3, "token")("Exact")).rejects.toThrow(
      "binary file",
    );
  });
});
