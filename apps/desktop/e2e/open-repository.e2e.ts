import { expect, test, type Page } from "@playwright/test";

declare global {
  interface Window {
    __mergeiqHome?: { opened: string[]; removed: string[] };
  }
}

const calls = (page: Page) => page.evaluate(() => window.__mergeiqHome);

test("Opening a recent repository calls the backend and reorders recents", async ({
  page,
}) => {
  await page.goto("/dev/home");
  await page
    .getByRole("button", { name: /payments-service/ })
    .first()
    .click();
  await expect(page.getByRole("listitem").first()).toContainText(
    "payments-service",
  );
  expect((await calls(page))?.opened).toEqual(["/code/work/payments-service"]);
});

test("Not a repo: error shown, recents unchanged", async ({ page }) => {
  await page.goto("/dev/home?picked=/Downloads/brand-assets");
  await page.getByRole("button", { name: /Open repository/ }).click();
  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Not a git repository.");
  await expect(alert).toContainText("/Downloads/brand-assets");
  await expect(page.getByRole("listitem")).toHaveCount(3);
  await alert.getByRole("button", { name: "Dismiss" }).click();
  await expect(alert).toHaveCount(0);
});

test("Dropping a folder opens it", async ({ page }) => {
  await page.goto("/dev/home");
  await expect(page.getByTestId("drop-zone")).toBeVisible();
  await page.evaluate(() =>
    window.dispatchEvent(
      new CustomEvent("mergeiq:drop", { detail: ["/code/new-repo"] }),
    ),
  );
  await expect(page.getByRole("listitem").first()).toContainText("new-repo");
  expect((await calls(page))?.opened).toEqual(["/code/new-repo"]);
});

test("Removing a missing recent", async ({ page }) => {
  await page.goto("/dev/home");
  await expect(page.getByText("folder not found")).toBeVisible();
  await page
    .getByRole("button", { name: "Remove old-api from recents" })
    .click();
  await expect(page.getByText("old-api")).toHaveCount(0);
});

test("Keyboard shortcut opens the picker", async ({ page }) => {
  await page.goto("/dev/home?picked=/code/new-repo");
  await expect(page.getByRole("listitem")).toHaveCount(3);
  await page.keyboard.press("Control+o");
  await expect(page.getByRole("listitem").first()).toContainText("new-repo");
});
