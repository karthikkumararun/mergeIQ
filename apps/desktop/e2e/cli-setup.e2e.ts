import { expect, test, type Page } from "@playwright/test";

declare global {
  interface Window {
    __mergeiqCli?: {
      installs: { dir: string; admin: boolean }[];
      configured: boolean[];
      addedToPath: string[];
    };
  }
}

const calls = (page: Page) => page.evaluate(() => window.__mergeiqCli);

test("Install to a folder that is not on PATH warns with the exact line", async ({
  page,
}) => {
  await page.goto("/dev/cli?onPath=0");
  await page.getByRole("button", { name: "Install", exact: true }).click();
  const warning = page
    .getByRole("status")
    .filter({ hasText: "not on your PATH" });
  await expect(warning).toContainText('export PATH="$HOME/.local/bin:$PATH"');
  await expect(warning).toContainText("~/.zshrc");
  expect((await calls(page))?.installs).toEqual([
    { dir: "/Users/me/.local/bin", admin: false },
  ]);
});

test("Confirmed git config runs the commands; cancel runs nothing", async ({
  page,
}) => {
  await page.goto("/dev/cli");
  await page.getByRole("button", { name: "Run 3 commands…" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  expect((await calls(page))?.configured ?? []).toEqual([]);

  await page.getByRole("checkbox").check();
  await page.getByRole("button", { name: "Run 4 commands…" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Run 4 commands" })
    .click();
  await expect(page.getByText("git is configured")).toBeVisible();
  expect((await calls(page))?.configured).toEqual([true]);
});
