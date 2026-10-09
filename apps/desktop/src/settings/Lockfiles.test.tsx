import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createMockLockfileApi } from "./lockfileApi";
import { Lockfiles } from "./Lockfiles";

describe("Settings › Lockfiles", () => {
  it("lists the default command of every regenerable lockfile", async () => {
    render(<Lockfiles api={createMockLockfileApi()} />);
    expect(await screen.findByLabelText("pnpm command")).toHaveValue(
      "pnpm install --lockfile-only",
    );
    expect(screen.getByLabelText("npm command")).toHaveValue(
      "npm install --package-lock-only",
    );
    expect(screen.getByLabelText("yarn command")).toHaveValue(
      "yarn install --mode update-lockfile",
    );
    expect(screen.getByLabelText("poetry command")).toHaveValue(
      "poetry lock --no-update",
    );
    expect(screen.getByLabelText("cargo command")).toHaveValue(
      "cargo update --workspace",
    );
    expect(screen.getByLabelText("gradle command")).toHaveValue(
      "./gradlew dependencies --write-locks",
    );
    expect(screen.queryByLabelText("go.sum command")).toBeNull();
  });

  it("Commands are editable and a customised command is marked and can be reset", async () => {
    const user = userEvent.setup();
    render(<Lockfiles api={createMockLockfileApi()} />);
    const input = await screen.findByLabelText("pnpm command");
    const save = screen.getAllByRole("button", { name: "Save" })[1];
    expect(save).toBeDisabled();
    await user.clear(input);
    await user.type(input, "pnpm install --lockfile-only --offline");
    await user.click(save);
    await waitFor(() => expect(screen.getByText("customised")).toBeVisible());
    expect(screen.getByLabelText("pnpm command")).toHaveValue(
      "pnpm install --lockfile-only --offline",
    );
    expect(screen.getByText("pnpm install --lockfile-only")).toBeVisible();
    await user.click(screen.getAllByRole("button", { name: "Reset" })[1]);
    await waitFor(() => expect(screen.queryByText("customised")).toBeNull());
    expect(screen.getByLabelText("pnpm command")).toHaveValue(
      "pnpm install --lockfile-only",
    );
  });

  it("an invalid command is rejected with the reason", async () => {
    const user = userEvent.setup();
    render(<Lockfiles api={createMockLockfileApi()} />);
    const input = await screen.findByLabelText("npm command");
    await user.clear(input);
    await user.type(input, "npm install 'oops");
    await user.click(screen.getAllByRole("button", { name: "Save" })[0]);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "unterminated quote",
    );
  });

  it("a stored customisation is shown on open", async () => {
    render(
      <Lockfiles
        api={createMockLockfileApi({ Cargo: "cargo update -w --offline" })}
      />,
    );
    expect(await screen.findByLabelText("cargo command")).toHaveValue(
      "cargo update -w --offline",
    );
    expect(screen.getByText("customised")).toBeVisible();
  });
});
