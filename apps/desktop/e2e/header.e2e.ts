import { expect, test } from "@playwright/test";
import { open } from "./helpers";

test("Popover lists the right side commits newest first", async ({ page }) => {
  await open(page, "mixed-changes");
  await page.locator("[data-header=right] button").first().click();
  const pop = page.getByRole("dialog", {
    name: "Commits on feature touching this file",
  });
  await expect(pop).toBeVisible();
  const items = pop.getByRole("listitem");
  await expect(items).toHaveCount(2);
  await expect(items.nth(0)).toContainText("Add caching layer");
  await expect(items.nth(1)).toContainText("Tune cache size");
  await expect(items.nth(0)).toContainText("e4f5a6b");
  await expect(items.nth(0)).toContainText("Linus");
  await expect(
    pop.getByRole("button", { name: "Copy SHA e4f5a6b" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(pop).toHaveCount(0);
});

test("Copy SHA copies the full commit id", async ({
  page,
  context,
  browserName,
}) => {
  test.skip(
    browserName !== "chromium",
    "clipboard permissions are Chromium-only in Playwright",
  );
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await open(page, "mixed-changes");
  await page.locator("[data-header=left] button").first().click();
  await page.getByRole("button", { name: "Copy SHA a1b2c3d" }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    "a1b2c3d0",
  );
});
