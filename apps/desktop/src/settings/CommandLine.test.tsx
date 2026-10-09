import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { CommandLine } from "./CommandLine";
import { createMockCliApi, type MockCliOptions } from "./mockCliApi";

function setup(options: MockCliOptions = {}) {
  window.__mergeiqCli = undefined;
  const api = createMockCliApi(options);
  render(<CommandLine api={api} />);
  return api;
}

const log = () => window.__mergeiqCli!;

describe("Settings › Command line", () => {
  beforeEach(() => {
    window.__mergeiqCli = undefined;
  });

  it("install into a folder that is not on PATH shows the exact line to add", async () => {
    setup({ onPath: false });
    await userEvent.click(
      await screen.findByRole("button", { name: "Install" }),
    );
    expect(log().installs).toEqual([
      { dir: "/Users/me/.local/bin", admin: false },
    ]);
    const warning = await screen.findByText(/is not on your PATH/);
    expect(warning.closest("[role=status]")).toHaveTextContent(
      'export PATH="$HOME/.local/bin:$PATH"',
    );
    expect(warning.closest("[role=status]")).toHaveTextContent("~/.zshrc");
  });

  it("install into a folder on PATH reports it without a warning", async () => {
    setup({ onPath: true });
    await userEvent.click(
      await screen.findByRole("button", { name: "Install" }),
    );
    expect(await screen.findByText(/is on your PATH/)).toBeInTheDocument();
    expect(screen.queryByText(/is not on your PATH/)).toBeNull();
  });

  it("/usr/local/bin asks for admin rights", async () => {
    setup();
    await userEvent.selectOptions(
      await screen.findByRole("combobox"),
      "/usr/local/bin",
    );
    await userEvent.click(screen.getByRole("button", { name: "Install" }));
    await waitFor(() =>
      expect(log().installs).toEqual([{ dir: "/usr/local/bin", admin: true }]),
    );
  });

  it("nothing runs until the git config is confirmed", async () => {
    setup();
    await userEvent.click(
      await screen.findByRole("button", { name: "Run 3 commands…" }),
    );
    expect(log().configured).toEqual([]);
    const dialog = screen.getByRole("dialog");
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Cancel" }),
    );
    expect(log().configured).toEqual([]);

    await userEvent.click(
      screen.getByRole("button", { name: "Run 3 commands…" }),
    );
    await userEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Run 3 commands",
      }),
    );
    await waitFor(() => expect(log().configured).toEqual([false]));
    expect(await screen.findByText(/git is configured/)).toBeInTheDocument();
  });

  it("the keepBackup command is dimmed until its checkbox is ticked", async () => {
    setup();
    const optional = await screen.findByText(/mergetool\.keepBackup false/);
    expect(optional.className).toMatch(/dim/);
    await userEvent.click(screen.getByRole("checkbox"));
    expect(optional.className).not.toMatch(/dim/);
    await userEvent.click(
      screen.getByRole("button", { name: "Run 4 commands…" }),
    );
    await userEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Run 4 commands",
      }),
    );
    await waitFor(() => expect(log().configured).toEqual([true]));
  });

  it("Windows offers adding the install folder to PATH", async () => {
    setup({ platform: "windows", onPath: false });
    await userEvent.click(
      await screen.findByRole("button", { name: "Check PATH" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Add to PATH" }),
    );
    expect(log().addedToPath).toEqual([
      "C:\\Users\\me\\AppData\\Local\\MergeIQ",
    ]);
    expect(await screen.findByText(/is on your PATH/)).toBeInTheDocument();
  });
});
