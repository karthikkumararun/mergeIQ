import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { createMockRepoApi, type MockRepoOptions } from "../repo/mockRepoApi";
import { SpecialConflictPanel } from "./SpecialConflictPanel";

async function mount(display: string, options: MockRepoOptions = {}) {
  const api = createMockRepoApi({ scenario: "special", ...options });
  const load = await api.loadConflict(`mock:${display}`);
  const handlers = {
    onBack: vi.fn(),
    onResolved: vi.fn().mockResolvedValue(undefined),
    onAccept: vi.fn(),
    onDelete: vi.fn(),
    onStaged: vi.fn().mockResolvedValue(undefined),
    onRenameChosen: vi.fn().mockResolvedValue(undefined),
    onMergeByHand: vi.fn(),
  };
  render(
    <SpecialConflictPanel
      load={load}
      api={api}
      repoRoot="/code/shop-web"
      {...handlers}
    />,
  );
  return { api, handlers, user: userEvent.setup() };
}

const calls = () => window.__mergeiqRepo?.calls ?? [];

describe("Modify/delete resolution", () => {
  it("shows which side deleted it and the surviving side's diff", async () => {
    await mount("src/main/kotlin/com/shop/promo/Coupon.kt");
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(
      "feature/checkout-v2 deleted this file, but main changed it",
    );
    const [leftCard, rightCard] = [0, 1].map(
      (i) => document.querySelectorAll("[class*=sideCard]")[i],
    );
    expect(leftCard).toHaveTextContent("Modified");
    expect(rightCard).toHaveTextContent("Deleted");
    expect(leftCard).toHaveTextContent("Refactor parser");
    const diff = await screen.findByRole("group", {
      name: "Changes since base",
    });
    expect(diff).toHaveTextContent("LocalDateTime");
    expect(diff).toHaveTextContent("val zone: ZoneId = ZoneOffset.UTC,");
    expect(screen.getByText("1 change · +2 −1")).toBeVisible();
  });

  it("Keep modified writes the surviving side and stages it", async () => {
    const { user, handlers } = await mount(
      "src/main/kotlin/com/shop/promo/Coupon.kt",
    );
    await user.click(
      await screen.findByRole("button", { name: "Keep modified" }),
    );
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Kept modified"),
    );
    expect(calls()).toContain(
      "useSide Ours mock:src/main/kotlin/com/shop/promo/Coupon.kt",
    );
  });

  it("Delete chosen removes the file", async () => {
    const { user, handlers } = await mount(
      "src/main/kotlin/com/shop/promo/Coupon.kt",
    );
    await user.click(
      await screen.findByRole("button", { name: "Delete file" }),
    );
    expect(handlers.onDelete).toHaveBeenCalledTimes(1);
  });

  it("Keep and edit opens the survivor in a plain editor and resolves on save", async () => {
    const { user, handlers } = await mount(
      "src/main/kotlin/com/shop/promo/Coupon.kt",
    );
    await user.click(
      await screen.findByRole("button", { name: "Keep and edit" }),
    );
    expect(
      await screen.findByRole("button", { name: "Save and mark resolved" }),
    ).toBeVisible();
    expect(calls()).toContain(
      "keepAndEdit Ours mock:src/main/kotlin/com/shop/promo/Coupon.kt",
    );
    expect(document.querySelector("[data-pane=plain]")).toBeTruthy();
    await user.click(
      screen.getByRole("button", { name: "Save and mark resolved" }),
    );
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Kept modified"),
    );
    expect(
      calls().some((c) =>
        c.startsWith(
          "save mock:src/main/kotlin/com/shop/promo/Coupon.kt stage=true",
        ),
      ),
    ).toBe(true);
    expect(
      window.__mergeiqRepo?.saves[(window.__mergeiqRepo?.saves.length ?? 1) - 1]
        ?.text,
    ).toContain("val zone: ZoneId");
  });

  it("Cancelling the editor returns to the choices without resolving", async () => {
    const { user, handlers } = await mount(
      "src/main/kotlin/com/shop/promo/Coupon.kt",
    );
    await user.click(
      await screen.findByRole("button", { name: "Keep and edit" }),
    );
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(
      await screen.findByRole("button", { name: "Keep modified" }),
    ).toBeVisible();
    expect(handlers.onResolved).not.toHaveBeenCalled();
  });
});

describe("Binary and image resolution", () => {
  it("Image preview shows base, left and right with sizes and blobs", async () => {
    await mount("web/assets/logo.png");
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(
      "Both sides changed this image",
    );
    for (const name of ["Base", "Left, main", "Right, feature/checkout-v2"]) {
      const card = screen.getByRole("region", { name });
      await waitFor(() => expect(within(card).getByRole("img")).toBeVisible());
      const img = within(card).getByRole("img") as HTMLImageElement;
      // An <img>: SVG never becomes inline markup.
      expect(img.tagName).toBe("IMG");
      expect(img.src.startsWith("data:image/svg+xml;base64,")).toBe(true);
    }
    expect(screen.getByRole("region", { name: "Base" })).toHaveTextContent(
      "18.4 KB",
    );
    expect(
      screen.getByRole("region", { name: "Left, main" }),
    ).toHaveTextContent("b72e90d");
    expect(
      screen.getByRole("region", { name: "Right, feature/checkout-v2" }),
    ).toHaveTextContent("41.2 KB");
    expect(screen.getByText("Reference only")).toBeVisible();
    expect(screen.getByText(/never inline markup/)).toBeVisible();
    expect(screen.getByText("Binary · image")).toBeVisible();
  });

  it("Use Right writes the right side", async () => {
    const { user, handlers } = await mount("web/assets/logo.png");
    await user.click(await screen.findByRole("button", { name: "Use Right" }));
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Accepted Right"),
    );
    expect(calls()).toContain("useSide Theirs mock:web/assets/logo.png");
  });

  it("a non-image binary shows the same cards without previews", async () => {
    const { user, handlers } = await mount("assets/models/scene.bin");
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(
      "Both sides changed this file",
    );
    expect(screen.queryByRole("img")).toBeNull();
    expect(
      screen.getByRole("region", { name: "Left, main" }),
    ).toHaveTextContent("2.4 MB");
    expect(screen.getByText("Binary")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Use Left" }));
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Accepted Left"),
    );
  });
});

describe("Symlink resolution", () => {
  it("Symlink targets differ", async () => {
    const { user, handlers } = await mount("config/current");
    const rows = await screen.findByText("→ ../envs/prod-eu");
    expect(rows).toBeVisible();
    expect(screen.getByText("→ ../envs/staging")).toBeVisible();
    expect(screen.getByText("→ ../envs/prod-us")).toBeVisible();
    expect(screen.getByText(/mode 120000/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Use Right" }));
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Accepted Right"),
    );
    expect(calls()).toContain("useSide Theirs mock:config/current");
  });
});

describe("LFS pointer resolution", () => {
  it("shows oid and size per side", async () => {
    await mount("assets/video/hero.mp4");
    expect(await screen.findByText("oid 4d7a21…e9c0 · 38.2 MB")).toBeVisible();
    expect(screen.getByText("oid 9b1f03…27aa · 36.9 MB")).toBeVisible();
    expect(screen.getByText("oid c25e88…b413 · 41.0 MB")).toBeVisible();
    expect(
      screen.getByText(/LFS fetches the content on checkout/),
    ).toBeVisible();
  });

  it("LFS pick stages that side's pointer", async () => {
    const { user, handlers } = await mount("assets/video/hero.mp4");
    await user.click(await screen.findByRole("button", { name: "Use Left" }));
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Accepted Left"),
    );
    expect(calls()).toContain("useSide Ours mock:assets/video/hero.mp4");
  });
});

describe("Oversized files", () => {
  it("50 MB text file: sizes and no editor", async () => {
    const { user } = await mount("data/exports/orders-2026.csv");
    expect(await screen.findByText("48.6 MB")).toBeVisible();
    expect(screen.getByText("50.1 MB")).toBeVisible();
    expect(screen.getByText("52.3 MB")).toBeVisible();
    expect(
      screen.getByText(/won’t load this file into the editor/),
    ).toBeVisible();
    expect(document.querySelector(".cm-editor")).toBeNull();
    await user.click(
      screen.getByRole("button", { name: "Open in default app" }),
    );
    expect(calls()).toContain("open mock:data/exports/orders-2026.csv");
  });
});

describe("Submodule resolution", () => {
  it("Fast-forwardable submodule marks Use Right as recommended", async () => {
    const { user, handlers } = await mount("vendor/ui-kit");
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(
      "Which ui-kit commit should this repository point to?",
    );
    expect(screen.getByText("Bump tokens to 3.8")).toBeVisible();
    expect(screen.getByText("Sep 30")).toBeVisible();
    expect(screen.getByRole("status")).toHaveTextContent(
      "a41f0c2 (left) is an ancestor of 7be9d13 (right). Using Right keeps both sides’ changes.",
    );
    expect(screen.getByText("Recommended")).toBeVisible();
    expect(
      screen.getByText(/Descendant of both\. Same as a fast-forward/),
    ).toBeVisible();
    expect(
      screen.getByText(
        /git update-index --cacheinfo 160000,7be9d13…,vendor\/ui-kit/,
      ),
    ).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: /Use Right · 7be9d13/ }),
    );
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Accepted Right"),
    );
    expect(calls()).toContain("useSide Theirs mock:vendor/ui-kit");
  });

  it("a submodule that is not checked out shows SHAs only and recommends nothing", async () => {
    await mount("vendor/icons");
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Ancestry unknown (submodule not checked out)",
    );
    expect(screen.queryByText("Recommended")).toBeNull();
    expect(screen.getByText("e7f1d90")).toBeVisible();
    expect(screen.getByText("2a9c6b4")).toBeVisible();
    expect(screen.queryByText("Sep 30")).toBeNull();
  });

  it("diverged commits warn that choosing one drops the other", async () => {
    await mount("vendor/charts");
    expect(await screen.findByRole("status")).toHaveTextContent(/diverged/);
    expect(screen.queryByText("Recommended")).toBeNull();
  });
});

describe("Rename awareness", () => {
  it("Rename/rename asks for the final path and enables the action once chosen", async () => {
    const { user, api, handlers } = await mount("src/promo/discount.ts");
    expect(
      await screen.findByText("Both sides moved this file to different places"),
    ).toBeVisible();
    // The opened path's side starts selected.
    expect(
      screen.getByRole("radio", { name: /src\/promo\/discount\.ts/ }),
    ).toBeChecked();
    expect(
      screen.getByRole("radio", { name: /src\/cart\/pricing\/discount\.ts/ }),
    ).not.toBeChecked();
    expect(screen.getByText(/The contents differ too/)).toBeVisible();
    await user.click(
      screen.getByRole("radio", { name: /src\/cart\/pricing\/discount\.ts/ }),
    );
    await user.click(
      screen.getByRole("button", { name: "Use this path and merge content" }),
    );
    await waitFor(() =>
      expect(handlers.onRenameChosen).toHaveBeenCalledWith({
        chosen: "mock:src/cart/pricing/discount.ts",
        needsMerge: true,
      }),
    );
    expect(calls()).toContain("renameChoose mock:src/cart/pricing/discount.ts");
    // The three involved paths are gone and the chosen one is a text conflict now.
    const status = await api.status();
    expect(status.conflicts.map((c) => c.display)).toContain(
      "src/cart/pricing/discount.ts",
    );
    expect(status.conflicts.map((c) => c.display)).not.toContain(
      "src/promo/discount.ts",
    );
    expect(status.conflicts.map((c) => c.display)).not.toContain(
      "src/cart/discount.ts",
    );
  });

  it("the original path opens the same panel and Cancel goes back", async () => {
    const { user, handlers } = await mount("src/cart/discount.ts");
    expect(
      await screen.findByText("Both sides moved this file to different places"),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Use this path and merge content" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(handlers.onBack).toHaveBeenCalled();
  });
});

describe("go.sum union merge", () => {
  it("previews the union with a legend and strikes removed lines", async () => {
    await mount("services/edge/go.sum");
    expect(await screen.findByText("Auto-merge checksums")).toBeVisible();
    await screen.findByText(/golang\.org\/x\/net v0\.28\.0/);
    const legend =
      screen.getByText(/added by main/).parentElement!.parentElement!;
    expect(legend).toHaveTextContent("L+ 2 added by main");
    expect(legend).toHaveTextContent("R+ 4 added by feature/checkout-v2");
    expect(legend).toHaveTextContent("− 2 removed");
    expect(legend).toHaveTextContent("4 unchanged");
    const removed = screen.getByText(/golang\.org\/x\/net v0\.28\.0/);
    expect(removed.closest("div")!.className).toMatch(/sumX/);
    expect(
      within(removed.closest("div")!).getByText("Removed:"),
    ).toBeInTheDocument();
  });

  it("Auto-merge stages the union", async () => {
    const { user, handlers } = await mount("services/edge/go.sum");
    await user.click(
      await screen.findByRole("button", {
        name: "Auto-merge (union) and stage",
      }),
    );
    await waitFor(() =>
      expect(handlers.onResolved).toHaveBeenCalledWith("Auto-merged"),
    );
    expect(calls()).toContain("goSumUnion mock:services/edge/go.sum");
  });

  it("Merge by hand opens the editor instead", async () => {
    const { user, handlers } = await mount("services/edge/go.sum");
    await user.click(
      await screen.findByRole("button", { name: "Merge by hand" }),
    );
    expect(handlers.onMergeByHand).toHaveBeenCalled();
  });
});

describe("Lockfile regeneration", () => {
  const radio = (name: string | RegExp) => screen.getByRole("radio", { name });

  it("needs a side before it can run and shows the exact command and directory", async () => {
    await mount("web/pnpm-lock.yaml");
    const run = await screen.findByRole("button", { name: "Run command" });
    expect(run).toBeDisabled();
    expect(screen.getByRole("textbox", { name: "Command" })).toHaveValue(
      "pnpm install --lockfile-only",
    );
    expect(screen.getByText("/code/shop-web/web")).toBeVisible();
    expect(screen.getByText(/asked every time/)).toBeVisible();
    expect(
      screen.getByText(/staged only if the command exits 0/),
    ).toBeVisible();
    expect(screen.getByText(/may change other files/)).toBeVisible();
    expect(radio(/Take Left and regenerate/)).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("Regenerate pnpm lockfile: takes the side, runs the confirmed command, stages on exit 0", async () => {
    const { user, handlers } = await mount("web/pnpm-lock.yaml");
    await user.click(
      await screen.findByRole("radio", { name: /Take Right and regenerate/ }),
    );
    await user.click(screen.getByRole("button", { name: "Run command" }));
    expect(await screen.findByText(/Exited 0 in 4\.2 s/)).toBeVisible();
    expect(
      screen.getByText(/pnpm-lock\.yaml/, { selector: "span span" }),
    ).toBeVisible();
    expect(screen.getByLabelText("Output")).toHaveTextContent(
      "$ pnpm install --lockfile-only",
    );
    expect(screen.getByLabelText("Output")).toHaveTextContent(
      "Progress: resolved 812",
    );
    expect(calls()).toContain(
      "lockfile Theirs mock:web/pnpm-lock.yaml pnpm install --lockfile-only",
    );
    expect(handlers.onStaged).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(handlers.onBack).toHaveBeenCalled();
  });

  it("an edited command is what runs", async () => {
    const { user } = await mount("web/pnpm-lock.yaml");
    const input = await screen.findByRole("textbox", { name: "Command" });
    await user.clear(input);
    await user.type(input, "pnpm install --lockfile-only --offline");
    await user.click(radio(/Take Left and regenerate/));
    await user.click(screen.getByRole("button", { name: "Run command" }));
    await screen.findByText(/Exited 0/);
    expect(calls()).toContain(
      "lockfile Ours mock:web/pnpm-lock.yaml pnpm install --lockfile-only --offline",
    );
  });

  it("Regeneration failure shows the output and does not stage", async () => {
    const { user, handlers } = await mount("web/pnpm-lock.yaml", {
      lockfile: "fail",
    });
    await user.click(await screen.findByRole("radio", { name: /Take Right/ }));
    await user.click(screen.getByRole("button", { name: "Run command" }));
    expect(
      await screen.findByText(
        /Exited 1 · not staged · the conflict is still listed/,
      ),
    ).toBeVisible();
    expect(screen.getByLabelText("Output")).toHaveTextContent(
      "ERR_PNPM_NO_MATCHING_VERSION",
    );
    expect(handlers.onStaged).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Run again" })).toBeVisible();
  });

  it("a running command can be cancelled and is not staged", async () => {
    const { user, handlers } = await mount("web/pnpm-lock.yaml", {
      lockfile: "hang",
    });
    await user.click(await screen.findByRole("radio", { name: /Take Right/ }));
    await user.click(screen.getByRole("button", { name: "Run command" }));
    expect(await screen.findByText("Running…")).toBeVisible();
    // The options are locked while it runs.
    expect(screen.getByRole("textbox", { name: "Command" })).toBeDisabled();
    await act(async () => {
      await user.click(
        within(
          screen.getByRole("region", { name: "Command output" }),
        ).getByRole("button", { name: "Cancel" }),
      );
    });
    expect(await screen.findByText(/Cancelled · not staged/)).toBeVisible();
    expect(handlers.onStaged).not.toHaveBeenCalled();
  });

  it("a missing program is reported and nothing runs", async () => {
    const { user } = await mount("web/pnpm-lock.yaml", { lockfile: "missing" });
    await user.click(await screen.findByRole("radio", { name: /Take Left/ }));
    await user.click(screen.getByRole("button", { name: "Run command" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "`pnpm` was not found",
    );
    expect(screen.queryByRole("region", { name: "Command output" })).toBeNull();
  });

  it("Merge by hand is offered", async () => {
    const { user, handlers } = await mount("web/pnpm-lock.yaml");
    await user.click(
      await screen.findByRole("button", { name: "Merge by hand" }),
    );
    expect(handlers.onMergeByHand).toHaveBeenCalled();
  });
});

describe("Panel dispatch", () => {
  it("every panel has a way back to the conflicts", async () => {
    for (const display of [
      "web/assets/logo.png",
      "config/current",
      "vendor/ui-kit",
      "services/edge/go.sum",
    ]) {
      const { user, handlers } = await mount(display);
      await screen.findByRole("button", { name: "← Conflicts" });
      await user.click(screen.getByRole("button", { name: "← Conflicts" }));
      expect(handlers.onBack).toHaveBeenCalled();
      document.body.innerHTML = "";
    }
  });
});
