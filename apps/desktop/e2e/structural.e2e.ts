import { expect, test, type Page } from "@playwright/test";
import { counter, mod, open, paneText } from "./helpers";

const indicators = (page: Page) =>
  page.getByRole("button", { name: "Show structural proposal" });
const bulk = (page: Page, n: number) =>
  page.getByRole("button", { name: `Resolve structurally (${n})` });

async function undo(page: Page) {
  await page.locator("[data-pane=result] .cm-content").click();
  await page.keyboard.press(`${mod}+z`);
}

test("Chunks with a proposal get an S indicator, others do not", async ({
  page,
}) => {
  await open(page, "structural-package-json");
  // Three conflicts, two of them have proposals (the pnpm version was changed by both).
  await expect(indicators(page)).toHaveCount(2);
  await expect(counter(page)).toHaveText("3 changes · 3 conflicts left");
  await expect(bulk(page, 2)).toBeVisible();
  await expect(page.getByTestId("structural-status")).toHaveText(
    "Proposals ready · computed in 12 ms",
  );
});

test("Preview shows the change and Apply resolves the chunk", async ({
  page,
}) => {
  await open(page, "structural-package-json");
  await indicators(page).first().click();
  const dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("heading", { name: "Structural proposal · scripts" }),
  ).toBeVisible();
  await expect(dialog).toContainText("Left added lint and right added test.");
  await expect(dialog).toContainText('"lint": "eslint .",');
  await expect(dialog).toContainText("Covers 1 conflict");
  await expect(dialog).toContainText("Validated: parses without errors");

  await dialog.getByRole("button", { name: "Apply proposal" }).click();
  await expect(dialog).toHaveCount(0);
  const result = await paneText(page, "result");
  expect(result).toContain(
    '    "build": "vite build",\n    "lint": "eslint .",\n    "test": "vitest"\n  },',
  );
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
  await expect(indicators(page)).toHaveCount(1);
  await expect(bulk(page, 1)).toBeVisible();
});

test("Dismiss and Escape leave the Result alone", async ({ page }) => {
  await open(page, "structural-package-json");
  const before = await paneText(page, "result");

  await indicators(page).first().click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);

  await indicators(page).first().click();
  await page.getByRole("button", { name: "Dismiss" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await paneText(page, "result")).toBe(before);
  await expect(indicators(page)).toHaveCount(1);
  await expect(bulk(page, 1)).toBeVisible();
});

test("Toolbar bulk structural resolves three of four and one undo reverts all", async ({
  page,
}) => {
  await open(page, "structural-ts");
  await expect(counter(page)).toHaveText("4 changes · 4 conflicts left");
  const base = await paneText(page, "result");

  await bulk(page, 3).click();
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  const resolved = await paneText(page, "result");
  expect(resolved).toContain('import { a, b, c } from "./a";');
  expect(resolved).toContain("  verbose: boolean;\n  retries: number;");
  expect(resolved).toContain("  Safe,\n  Auto,\n  Debug\n}");
  await expect(indicators(page)).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: /Resolve structurally/ }),
  ).toHaveCount(0);

  await undo(page);
  await expect(counter(page)).toHaveText("4 changes · 4 conflicts left");
  expect(await paneText(page, "result")).toBe(base);
  await expect(indicators(page)).toHaveCount(3);
  await expect(bulk(page, 3)).toBeVisible();
});

test("Files without proposals show nothing", async ({ page }) => {
  await open(page, "simple-conflict");
  await expect(indicators(page)).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: /Resolve structurally/ }),
  ).toHaveCount(0);
});

test("While proposals are computed the editor stays usable", async ({
  page,
}) => {
  await open(page, "structural-package-json", "&structural=slow");
  await expect(page.getByTestId("structural-status")).toHaveText(
    "Finding structural merges…",
  );
  // Regular actions work meanwhile; a half-applied conflict still gets its proposal.
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  expect(await paneText(page, "result")).toContain('"lint": "eslint ."');
  await expect(bulk(page, 2)).toBeVisible({ timeout: 5000 });
  await expect(page.getByTestId("structural-status")).toHaveText(
    "Proposals ready · computed in 12 ms",
  );
});

test("A timeout or an unsupported file shows nothing", async ({ page }) => {
  await open(page, "structural-package-json", "&structural=timeout");
  await expect(page.getByTestId("structural-status")).toHaveText("");
  await expect(indicators(page)).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: /Resolve structurally/ }),
  ).toHaveCount(0);
});

test("Proposals are keyboard reachable", async ({ page, browserName }) => {
  // WebKit only tabs to buttons with Option+Tab unless the OS "Keyboard navigation" setting is on.
  const next = browserName === "webkit" ? "Alt+Tab" : "Tab";
  await open(page, "structural-package-json");
  await indicators(page).first().focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeFocused();
  await page.keyboard.press(next);
  await expect(dialog.getByRole("button", { name: "Dismiss" })).toBeFocused();
  await page.keyboard.press(next);
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
});

test.describe("Visual", () => {
  test.skip(
    ({ browserName }) => browserName !== "chromium",
    "visual baselines are Chromium-only",
  );

  for (const theme of ["dark", "light"]) {
    test(`Visual: open structural preview (${theme})`, async ({ page }) => {
      await open(page, "structural-package-json", `&theme=${theme}`);
      await indicators(page).first().click();
      await expect(page.getByRole("dialog")).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
      await page.mouse.move(0, 0);
      await expect(page).toHaveScreenshot(`structural-preview-${theme}.png`, {
        maxDiffPixelRatio: 0.01,
      });
    });
  }
});
