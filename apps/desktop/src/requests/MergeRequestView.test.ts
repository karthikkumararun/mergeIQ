import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = vi.hoisted(() => ({
  mergeRequestSave: vi.fn(),
  requestClose: vi.fn(),
}));
vi.mock("../ipc/bindings", () => ({ commands }));

import type { SaveResult } from "../merge-editor/model/types";
import { saveAndClose, toMergeDocument } from "./mergeRequest";

const result: SaveResult = {
  lines: [
    { text: "one", term: "Lf" },
    { text: "two", term: "None" },
  ],
  unresolvedIds: [],
  unresolved: [],
  mode: "resolved",
  encoding: { encoding: "Utf8", bom: false },
};

describe("saveAndClose", () => {
  beforeEach(() => {
    commands.mergeRequestSave.mockReset();
    commands.requestClose.mockReset();
  });

  it("saves the rendered text with the mode, then closes the window", async () => {
    commands.mergeRequestSave.mockResolvedValue({ status: "ok", data: null });
    commands.requestClose.mockResolvedValue({ status: "ok", data: null });
    await saveAndClose(7, result);
    expect(commands.mergeRequestSave).toHaveBeenCalledWith(
      7,
      "one\ntwo",
      { encoding: "Utf8", bom: false },
      "resolved",
    );
    expect(commands.requestClose).toHaveBeenCalledWith(7, "merge");
  });

  it("keeps the window open when saving fails", async () => {
    commands.mergeRequestSave.mockResolvedValue({
      status: "error",
      error: { kind: "Request", message: "disk full" },
    });
    await expect(saveAndClose(7, result)).rejects.toThrow("disk full");
    expect(commands.requestClose).not.toHaveBeenCalled();
  });
});

describe("toMergeDocument", () => {
  it("maps the backend document to the editor's", () => {
    const doc = toMergeDocument({
      displayPath: "a.txt",
      analysis: {} as never,
      labels: { left: {} as never, right: {} as never },
      context: null,
    });
    expect(doc.displayPath).toBe("a.txt");
    expect(doc.context).toBeUndefined();
  });
});
