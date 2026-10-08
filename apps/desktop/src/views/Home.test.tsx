import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Home } from "./Home";

vi.mock("../ipc/bindings", () => ({
  commands: {
    appInfo: vi.fn().mockResolvedValue({
      name: "MergeIQ",
      version: "0.1.0",
      platform: "macos",
    }),
    getSettings: vi.fn().mockResolvedValue({ theme: "dark" }),
    updateSettings: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: { theme: "dark" } }),
  },
}));

describe("Home", () => {
  it("shows the app name, version chip and theme control", async () => {
    render(<Home onOpenSettings={() => {}} />);

    expect(screen.getByText("MergeIQ")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByText("v0.1.0 · macOS")).toBeInTheDocument(),
    );

    expect(screen.getByRole("group", { name: "Theme" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Light" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dark" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "System" })).toBeInTheDocument();
  });
});
