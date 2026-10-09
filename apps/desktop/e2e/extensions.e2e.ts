import { expect, test } from "@playwright/test";
import { open, paneText } from "./helpers";

test("Extensions default to none", async ({ page }) => {
  await open(page, "mixed-changes");
  await expect(page.getByRole("button", { name: "Ext toolbar" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: /^Ext chunk/ })).toHaveCount(0);
});

test("Extensions can add toolbar items and chunk actions", async ({ page }) => {
  await open(page, "mixed-changes", "&extension=1");
  await expect(
    page.getByRole("button", { name: /^Ext chunk/ }).first(),
  ).toBeVisible();
  await page.getByRole("button", { name: "Ext toolbar" }).click();
  expect((await paneText(page, "result")).startsWith("ext\n")).toBe(true);
});
