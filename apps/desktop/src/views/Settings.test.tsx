import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

const commands = vi.hoisted(() => ({
  getSettings: vi.fn().mockResolvedValue({ theme: "dark" }),
  updateSettings: vi.fn().mockResolvedValue({ status: "ok", data: {} }),
  getMergeEditorSettings: vi.fn().mockResolvedValue({
    autoApplyNonConflicting: false,
    showBase: false,
    collapseUnchanged: false,
    syncScroll: true,
    whitespacePolicy: "Exact",
    autoAdvanceAfterSave: true,
  }),
  updateMergeEditorSettings: vi
    .fn()
    .mockResolvedValue({ status: "ok", data: {} }),
}));
vi.mock("../ipc/bindings", () => ({ commands }));

import { Settings } from "./Settings";

describe("Settings › General", () => {
  it("toggles auto-advance after save and persists it", async () => {
    render(<Settings onClose={() => {}} />);
    const box = await screen.findByRole("checkbox", {
      name: /open the next conflicted file/,
    });
    await waitFor(() => expect(box).toBeEnabled());
    expect(box).toBeChecked();
    await userEvent.click(box);
    expect(box).not.toBeChecked();
    expect(commands.updateMergeEditorSettings).toHaveBeenCalledWith(
      expect.objectContaining({ autoAdvanceAfterSave: false }),
    );
  });
});
