import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { OpenRepository } from "./OpenRepository";
import { openedLabel } from "./openedLabel";
import { createMockHomeApi, type MockHomeOptions } from "./mockHomeApi";

function setup(options: MockHomeOptions = {}) {
  window.__mergeiqHome = undefined;
  const api = createMockHomeApi({
    repos: ["/code/shop-web", "/code/work/payments-service", "/code/new-repo"],
    ...options,
  });
  render(<OpenRepository api={api} />);
  return api;
}

const names = () =>
  screen
    .getAllByRole("listitem")
    .map((li) => li.querySelector("span span")?.textContent);

describe("Open repository", () => {
  beforeEach(() => {
    window.__mergeiqHome = undefined;
  });

  it("opens a recent repository and moves it to the top (recents ordering)", async () => {
    setup();
    await screen.findByText("shop-web");
    expect(names()).toEqual(["shop-web", "payments-service", "old-api"]);
    await userEvent.click(
      screen.getByRole("button", { name: /^payments-service/ }),
    );
    await waitFor(() =>
      expect(names()).toEqual(["payments-service", "shop-web", "old-api"]),
    );
    expect(window.__mergeiqHome?.opened).toEqual([
      "/code/work/payments-service",
    ]);
  });

  it("a folder that is not a repo shows an error and leaves recents unchanged", async () => {
    setup({ picked: "/Downloads/brand-assets" });
    await screen.findByText("shop-web");
    await userEvent.click(
      screen.getByRole("button", { name: /Open repository/ }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Not a git repository.");
    expect(alert).toHaveTextContent("/Downloads/brand-assets");
    expect(names()).toEqual(["shop-web", "payments-service", "old-api"]);
    await userEvent.click(
      within(alert).getByRole("button", { name: "Dismiss" }),
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("the folder picker opens the chosen repository", async () => {
    setup({ picked: "/code/new-repo" });
    await screen.findByText("shop-web");
    await userEvent.click(
      screen.getByRole("button", { name: /Open repository/ }),
    );
    await waitFor(() =>
      expect(window.__mergeiqHome?.opened).toEqual(["/code/new-repo"]),
    );
    await screen.findByText("new-repo");
  });

  it("Ctrl+O opens the folder picker", async () => {
    setup({ picked: "/code/new-repo" });
    await screen.findByText("shop-web");
    await userEvent.keyboard("{Control>}o{/Control}");
    await waitFor(() =>
      expect(window.__mergeiqHome?.opened).toEqual(["/code/new-repo"]),
    );
  });

  it("a dropped folder opens like a picked one", async () => {
    setup();
    await screen.findByText("shop-web");
    act(() => {
      window.dispatchEvent(
        new CustomEvent("mergeiq:drop", { detail: ["/code/new-repo"] }),
      );
    });
    await waitFor(() =>
      expect(window.__mergeiqHome?.opened).toEqual(["/code/new-repo"]),
    );
  });

  it("missing paths are disabled with a remove action", async () => {
    setup();
    const missing = (await screen.findByText(/folder not found/)).closest(
      "[aria-disabled=true]",
    );
    expect(missing).not.toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Remove old-api from recents" }),
    );
    await waitFor(() => expect(screen.queryByText("old-api")).toBeNull());
    expect(window.__mergeiqHome?.removed).toEqual(["/code/old-api"]);
  });

  it("shows an empty hint without recents", async () => {
    setup({ recents: [] });
    expect(
      await screen.findByText("Repositories you open show up here."),
    ).toBeInTheDocument();
  });
});

describe("openedLabel", () => {
  const now = new Date("2026-10-09T12:00:00Z");
  const at = (secondsAgo: number) => now.getTime() / 1000 - secondsAgo;
  it("formats relative times", () => {
    expect(openedLabel(at(10), now)).toBe("Just now");
    expect(openedLabel(at(120), now)).toBe("2 min ago");
    expect(openedLabel(at(3 * 3600), now)).toBe("3 h ago");
    expect(openedLabel(at(30 * 3600), now)).toBe("Yesterday");
    expect(openedLabel(at(3 * 86400), now)).toBe("3 days ago");
    expect(openedLabel(at(40 * 86400), now)).toMatch(/Aug/);
  });
});
