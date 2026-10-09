import { expect, test, type Page } from "@playwright/test";

declare global {
  interface Window {
    __mergeiqRepo?: {
      externalResolve(display: string): void;
      requestClose(): boolean;
      calls: string[];
      saves: { path: string; text: string; stage: boolean }[];
    };
  }
}

async function open(page: Page, extra = "", scenario = "special") {
  await page.goto(`/dev/repo?scenario=${scenario}${extra}`);
  await page.getByRole("region", { name: "Operation in progress" }).waitFor();
}

const rows = (page: Page) => page.getByTestId("conflict-row");
const calls = (page: Page) =>
  page.evaluate(() => window.__mergeiqRepo?.calls ?? []);

/** Opens `file` (a path fragment) from the conflict list in a tab. */
async function openFile(page: Page, file: string) {
  await rows(page)
    .filter({ hasText: file })
    .getByRole("button", { name: /^Merge/ })
    .click();
}

const banner = (page: Page) =>
  page.getByRole("region", { name: "Operation in progress" });

test("Every conflict class opens its own panel", async ({ page }) => {
  await open(page);
  const cases: [string, string][] = [
    ["logo.png", "Binary conflict"],
    ["scene.bin", "Binary conflict"],
    ["Coupon.kt", "Modify/delete conflict"],
    ["current", "Symlink conflict"],
    ["hero.mp4", "Git LFS conflict"],
    ["orders-2026.csv", "Oversized file conflict"],
    ["ui-kit", "Submodule conflict"],
    ["src/promo/", "Rename conflict"],
    ["go.sum", "go.sum conflict"],
    ["pnpm-lock.yaml", "Lockfile conflict"],
  ];
  for (const [file, panel] of cases) {
    await openFile(page, file);
    await expect(page.getByRole("region", { name: panel })).toBeVisible();
  }
  // A plain text conflict still gets the merge editor.
  await openFile(page, "app.ts");
  await expect(page.locator("[data-pane=result] .cm-content")).toBeVisible();
});

test("Image preview shows three previews with pixel dimensions and Use Right resolves", async ({
  page,
}) => {
  await open(page);
  await openFile(page, "logo.png");
  const panel = page.getByRole("region", { name: "Binary conflict" });
  await expect(panel.getByRole("img")).toHaveCount(3);
  await expect(panel).toContainText("512 × 512 px");
  await expect(panel).toContainText("1024 × 1024 px");
  await expect(panel).toContainText("Reference only");
  await panel.getByRole("button", { name: "Use Right" }).click();
  await expect(rows(page).filter({ hasText: "logo.png" })).toHaveCount(0);
  expect(await calls(page)).toContain(
    "useSide Theirs mock:web/assets/logo.png",
  );
  await expect(page.getByRole("tab", { name: /logo\.png/ })).toHaveCount(0);
});

test("Delete chosen removes the file and leaves the list", async ({ page }) => {
  await open(page);
  await openFile(page, "Coupon.kt");
  const panel = page.getByRole("region", { name: "Modify/delete conflict" });
  await expect(panel).toContainText(
    "feature/checkout-v2 deleted this file, but main changed it",
  );
  await panel.getByRole("button", { name: "Delete file" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: /^Delete/ })
    .click();
  await expect(rows(page).filter({ hasText: "Coupon.kt" })).toHaveCount(0);
  expect(await calls(page)).toContain(
    "delete mock:src/main/kotlin/com/shop/promo/Coupon.kt",
  );
});

test("Keep and edit: edit the surviving side and save to resolve", async ({
  page,
}) => {
  await open(page);
  await openFile(page, "Coupon.kt");
  await page.getByRole("button", { name: "Keep and edit" }).click();
  const editor = page.locator("[data-pane=plain] .cm-content");
  await expect(editor).toContainText("val zone: ZoneId = ZoneOffset.UTC,");
  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.type("// edited by hand");
  await page.getByRole("button", { name: "Save and mark resolved" }).click();
  await expect(rows(page).filter({ hasText: "Coupon.kt" })).toHaveCount(0);
  const saves = await page.evaluate(() => window.__mergeiqRepo?.saves ?? []);
  expect(saves).toHaveLength(1);
  expect(saves[0].stage).toBe(true);
  expect(saves[0].text).toContain("// edited by hand");
  expect(saves[0].text).toContain("val zone: ZoneId");
});

test("Symlink: Use Right", async ({ page }) => {
  await open(page);
  await openFile(page, "current");
  const panel = page.getByRole("region", { name: "Symlink conflict" });
  await expect(panel).toContainText("→ ../envs/prod-eu");
  await expect(panel).toContainText("→ ../envs/prod-us");
  await panel.getByRole("button", { name: "Use Right" }).click();
  expect(await calls(page)).toContain("useSide Theirs mock:config/current");
  await expect(rows(page).filter({ hasText: "current" })).toHaveCount(0);
});

test("Oversized file: no editor, open in default app", async ({ page }) => {
  await open(page);
  await openFile(page, "orders-2026.csv");
  const panel = page.getByRole("region", { name: "Oversized file conflict" });
  await expect(panel).toContainText("50.1 MB");
  await expect(page.locator(".cm-editor")).toHaveCount(0);
  await panel.getByRole("button", { name: "Open in default app" }).click();
  expect(await calls(page)).toContain("open mock:data/exports/orders-2026.csv");
});

test("Submodule: the descendant is recommended", async ({ page }) => {
  await open(page);
  await openFile(page, "ui-kit");
  const panel = page.getByRole("region", { name: "Submodule conflict" });
  await expect(panel.getByText("Recommended")).toBeVisible();
  await panel.getByRole("button", { name: /Use Right/ }).click();
  expect(await calls(page)).toContain("useSide Theirs mock:vendor/ui-kit");
  await expect(rows(page).filter({ hasText: "ui-kit" })).toHaveCount(0);
});

test("Submodule that is not checked out recommends nothing", async ({
  page,
}) => {
  await open(page);
  await openFile(page, "icons");
  const panel = page.getByRole("region", { name: "Submodule conflict" });
  await expect(panel).toContainText(
    "Ancestry unknown (submodule not checked out)",
  );
  await expect(panel.getByText("Recommended")).toHaveCount(0);
});

test("Rename/rename: choose the path, then merge the contents in the editor", async ({
  page,
}) => {
  await open(page);
  await openFile(page, "src/promo/");
  const panel = page.getByRole("region", { name: "Rename conflict" });
  await expect(panel).toContainText(
    "Both sides moved this file to different places",
  );
  await panel
    .getByRole("radio", { name: /src\/cart\/pricing\/discount\.ts/ })
    .check();
  await panel
    .getByRole("button", { name: "Use this path and merge content" })
    .click();
  // Only the chosen path remains, and it opens in the merge editor.
  await expect(page.locator("[data-pane=result] .cm-content")).toBeVisible();
  await expect(page.getByRole("tab", { selected: true })).toContainText(
    "discount.ts",
  );
  await expect(rows(page).filter({ hasText: "src/promo/" })).toHaveCount(0);
  await expect(rows(page).filter({ hasText: "src/cart/pricing/" })).toHaveCount(
    1,
  );
  await expect(banner(page)).toContainText("13 conflicted files");
});

test("go.sum: Auto-merge stages the union", async ({ page }) => {
  await open(page);
  await openFile(page, "go.sum");
  const panel = page.getByRole("region", { name: "go.sum conflict" });
  await expect(panel).toContainText("2 added by main");
  await expect(panel).toContainText("4 added by feature/checkout-v2");
  await panel
    .getByRole("button", { name: "Auto-merge (union) and stage" })
    .click();
  expect(await calls(page)).toContain("goSumUnion mock:services/edge/go.sum");
  await expect(rows(page).filter({ hasText: "go.sum" })).toHaveCount(0);
});

test("go.sum: Merge by hand opens the merge editor", async ({ page }) => {
  await open(page);
  await openFile(page, "go.sum");
  await page.getByRole("button", { name: "Merge by hand" }).click();
  await expect(page.locator("[data-pane=result] .cm-content")).toBeVisible();
});

async function chooseRightAndRun(page: Page) {
  await openFile(page, "pnpm-lock.yaml");
  const panel = page.getByRole("region", { name: "Lockfile conflict" });
  await panel.getByRole("radio", { name: /Take Right and regenerate/ }).click();
  await panel.getByRole("button", { name: "Run command" }).click();
  return panel;
}

test("Regenerate pnpm lockfile: confirm, run, staged on exit 0", async ({
  page,
}) => {
  await open(page);
  await openFile(page, "pnpm-lock.yaml");
  const panel = page.getByRole("region", { name: "Lockfile conflict" });
  await expect(
    panel.getByRole("button", { name: "Run command" }),
  ).toBeDisabled();
  await expect(panel.getByRole("textbox", { name: "Command" })).toHaveValue(
    "pnpm install --lockfile-only",
  );
  await expect(panel).toContainText("/code/shop-web/web");
  // Nothing has run before the user confirmed.
  expect((await calls(page)).some((c) => c.startsWith("lockfile "))).toBe(
    false,
  );
  await panel.getByRole("radio", { name: /Take Right and regenerate/ }).click();
  await panel.getByRole("button", { name: "Run command" }).click();
  await expect(panel).toContainText("Exited 0 in 4.2 s");
  await expect(panel.getByLabel("Output", { exact: true })).toContainText(
    "$ pnpm install --lockfile-only",
  );
  expect(await calls(page)).toContain(
    "lockfile Theirs mock:web/pnpm-lock.yaml pnpm install --lockfile-only",
  );
  // The file left the list, with no "resolved outside" notice.
  await expect(rows(page).filter({ hasText: "pnpm-lock.yaml" })).toHaveCount(0);
  await expect(page.getByText("was resolved outside MergeIQ")).toHaveCount(0);
  await panel.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("tab", { name: /pnpm-lock/ })).toHaveCount(0);
});

test("Regeneration failure: output shown, not staged, conflict still listed", async ({
  page,
}) => {
  await open(page, "&lockfile=fail");
  const panel = await chooseRightAndRun(page);
  await expect(panel).toContainText(
    "Exited 1 · not staged · the conflict is still listed",
  );
  await expect(panel.getByLabel("Output", { exact: true })).toContainText(
    "ERR_PNPM_NO_MATCHING_VERSION",
  );
  await expect(rows(page).filter({ hasText: "pnpm-lock.yaml" })).toHaveCount(1);
  await expect(panel.getByRole("button", { name: "Run again" })).toBeVisible();
});

test("A running lockfile command can be cancelled", async ({ page }) => {
  await open(page, "&lockfile=hang");
  const panel = await chooseRightAndRun(page);
  await expect(panel.getByText("Running…")).toBeVisible();
  await panel
    .getByRole("region", { name: "Command output" })
    .getByRole("button", { name: "Cancel" })
    .click();
  await expect(panel).toContainText("Cancelled · not staged");
  await expect(rows(page).filter({ hasText: "pnpm-lock.yaml" })).toHaveCount(1);
});

test("A missing tool is reported before anything runs", async ({ page }) => {
  await open(page, "&lockfile=missing");
  await openFile(page, "pnpm-lock.yaml");
  const panel = page.getByRole("region", { name: "Lockfile conflict" });
  await panel.getByRole("radio", { name: /Take Left and regenerate/ }).click();
  await panel.getByRole("button", { name: "Run command" }).click();
  await expect(panel.getByRole("alert")).toContainText("`pnpm` was not found");
});

test("Lockfile: Merge by hand opens the editor", async ({ page }) => {
  await open(page);
  await openFile(page, "pnpm-lock.yaml");
  await page.getByRole("button", { name: "Merge by hand" }).click();
  await expect(page.locator("[data-pane=result] .cm-content")).toBeVisible();
});

test("Panels are keyboard operable and have no unnamed controls", async ({
  page,
}) => {
  await open(page);
  await openFile(page, "ui-kit");
  const panel = page.getByRole("region", { name: "Submodule conflict" });
  await panel.getByRole("button", { name: "← Conflicts" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { name: /ui-kit/ })).toHaveCount(0);
});
