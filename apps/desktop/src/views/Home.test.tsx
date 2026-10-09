import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { createMockCliApi } from "../settings/mockCliApi";
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
    render(<Home onOpenSettings={() => {}} cliApi={createMockCliApi()} />);

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

describe("Home › Set up cards", () => {
  it("shows Not installed / Not configured and opens the Command line settings", async () => {
    const onOpenSettings = vi.fn();
    const api = createMockCliApi();
    render(<Home onOpenSettings={onOpenSettings} cliApi={api} />);
    expect(await screen.findByText("Not installed")).toBeInTheDocument();
    expect(screen.getByText("Not configured")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Configure…" }));
    expect(onOpenSettings).toHaveBeenCalledWith("cli");
  });

  it("shows Installed / Configured when set up", async () => {
    const api = createMockCliApi({ installed: true, configured: true });
    render(<Home onOpenSettings={() => {}} cliApi={api} />);
    expect(await screen.findByText("Installed")).toBeInTheDocument();
    expect(screen.getByText("Configured")).toBeInTheDocument();
  });
});
