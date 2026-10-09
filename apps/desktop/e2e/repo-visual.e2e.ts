import { expect, test, type Page } from "@playwright/test";

// Baselines are per platform and Chromium-only, like the merge editor's
// (`pnpm exec playwright test repo-visual --update-snapshots`).
test.skip(
  ({ browserName }) => browserName !== "chromium",
  "visual baselines are Chromium-only",
);

async function ready(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(0, 0);
}

async function openRepo(page: Page, theme: string, scenario = "rebase2") {
  await page.goto(`/dev/repo?scenario=${scenario}&theme=${theme}`);
  await page.getByRole("region", { name: "Operation in progress" }).waitFor();
}

for (const theme of ["dark", "light"]) {
  test(`Visual: repository window, conflicts remaining (${theme})`, async ({
    page,
  }) => {
    await openRepo(page, theme);
    await page.getByRole("checkbox", { name: "Select cart.ts" }).check();
    await page
      .getByTestId("conflict-row")
      .nth(1)
      .getByRole("button", { name: /^Merge/ })
      .click();
    await ready(page);
    await expect(page).toHaveScreenshot(`repo-conflicts-${theme}.png`, {
      maxDiffPixelRatio: 0.01,
    });
  });

  test(`Visual: repository window, all resolved (${theme})`, async ({
    page,
  }) => {
    await openRepo(page, theme);
    await page
      .getByRole("checkbox", { name: "Select all shown files" })
      .check();
    await page
      .getByRole("button", { name: "Accept Right", exact: true })
      .click();
    await page
      .getByRole("dialog")
      .getByRole("button", { name: "Accept Right" })
      .click();
    await page.getByText("Resolved in this session · 2").click();
    await ready(page);
    await expect(page).toHaveScreenshot(`repo-resolved-${theme}.png`, {
      maxDiffPixelRatio: 0.01,
    });
  });
}

test("Visual: home view (dark)", async ({ page }) => {
  await page.goto("/dev/home?theme=dark");
  await page.getByRole("listitem").first().waitFor();
  await ready(page);
  await expect(page).toHaveScreenshot("home-dark.png", {
    maxDiffPixelRatio: 0.01,
  });
});
