import { expect, test } from "@playwright/test";
import { counter, open } from "./helpers";

test("Change policy before progress", async ({ page }) => {
  await open(page, "mixed-changes", "&reanalyze=simple-conflict");
  await page.getByLabel("Whitespace").selectOption("TrimTrailing");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  await expect(page.getByLabel("Whitespace")).toHaveValue("TrimTrailing");
});

test("Change policy after progress asks to reset", async ({ page }) => {
  await open(page, "mixed-changes", "&reanalyze=simple-conflict");
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  await page.getByLabel("Whitespace").selectOption("IgnoreAll");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Reset your progress?");
  await dialog.getByRole("button", { name: "Keep editing" }).click();
  await expect(counter(page)).toHaveText("2 changes · 1 conflict left");
  await page.getByLabel("Whitespace").selectOption("IgnoreAll");
  await page.getByRole("button", { name: "Recompute" }).click();
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
});

test("Whitespace selector is disabled without a re-analysis host", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  await expect(page.getByLabel("Whitespace")).toBeDisabled();
});
