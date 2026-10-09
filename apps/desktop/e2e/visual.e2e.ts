import { expect, test, type Page } from "@playwright/test";
import { open } from "./helpers";

// Baselines are per platform (font rasterisation differs); they are generated on
// the developer's machine with `pnpm exec playwright test visual --update-snapshots`.
test.skip(
  ({ browserName }) => browserName !== "chromium",
  "visual baselines are Chromium-only",
);

async function ready(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(0, 0);
}

test("Visual: default (dark)", async ({ page }) => {
  await open(page, "kotlin-sample", "&theme=dark");
  await ready(page);
  await expect(page).toHaveScreenshot("default-dark.png", {
    maxDiffPixelRatio: 0.01,
  });
});

test("Visual: default (light)", async ({ page }) => {
  await open(page, "kotlin-sample", "&theme=light");
  await ready(page);
  await expect(page).toHaveScreenshot("default-light.png", {
    maxDiffPixelRatio: 0.01,
  });
});

test("Visual: show base", async ({ page }) => {
  await open(page, "simple-resolvable", "&theme=dark&showBase=1");
  await ready(page);
  await expect(page).toHaveScreenshot("show-base.png", {
    maxDiffPixelRatio: 0.01,
  });
});

test("Visual: resolved chunks and current conflict", async ({ page }) => {
  await open(page, "mixed-changes", "&theme=dark");
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  await page.keyboard.press("F7");
  await ready(page);
  await expect(page).toHaveScreenshot("resolved-and-current.png", {
    maxDiffPixelRatio: 0.01,
  });
});

test("Visual: light theme with commit popover", async ({ page }) => {
  await open(page, "mixed-changes", "&theme=light");
  await page.locator("[data-header=right] button").first().click();
  await ready(page);
  await expect(page).toHaveScreenshot("light-popover.png", {
    maxDiffPixelRatio: 0.01,
  });
});

test("Visual: save dialog", async ({ page }) => {
  await open(page, "mixed-changes", "&theme=dark");
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await ready(page);
  await expect(page).toHaveScreenshot("save-dialog.png", {
    maxDiffPixelRatio: 0.01,
  });
});
