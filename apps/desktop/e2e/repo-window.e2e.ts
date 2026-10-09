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

async function open(page: Page, scenario = "merge3", extra = "") {
  await page.goto(`/dev/repo?scenario=${scenario}${extra}`);
  await page
    .getByRole("region", { name: /Operation|Repository state/ })
    .waitFor();
}

const rows = (page: Page) => page.getByTestId("conflict-row");
const calls = (page: Page) =>
  page.evaluate(() => window.__mergeiqRepo?.calls ?? []);
const continueButton = (page: Page) =>
  page.getByRole("button", { name: "Continue", exact: true });

test("Banner names the operation with contextual labels", async ({ page }) => {
  await open(page);
  const banner = page.getByRole("region", { name: "Operation in progress" });
  await expect(banner).toContainText("Merging");
  await expect(banner).toContainText("feature/checkout-v2");
  await expect(banner).toContainText("3 conflicted files");
  await expect(continueButton(page)).toBeDisabled();
  await expect(
    banner.getByRole("button", { name: "Skip commit…" }),
  ).toHaveCount(0);
});

test("Rebase banner shows progress and Skip", async ({ page }) => {
  await open(page, "rebase2");
  const banner = page.getByRole("region", { name: "Operation in progress" });
  await expect(banner).toContainText("Rebasing");
  await expect(banner).toContainText("commit 1 of 2: Add promo code stacking");
  await expect(
    banner.getByRole("button", { name: "Skip commit…" }),
  ).toBeVisible();
  await expect(
    banner.getByRole("button", { name: "Abort rebase…" }),
  ).toBeVisible();
});

test("Batch accept right resolves selected files and unlocks Continue", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("checkbox", { name: "Select all shown files" }).check();
  await expect(page.getByText("3 selected")).toBeVisible();
  await page.getByRole("button", { name: "Accept Right", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Accept Right for 3 files?");
  await expect(
    dialog.getByRole("list", { name: "Affected files" }),
  ).toContainText("src/app.ts");
  await dialog.getByRole("button", { name: "Accept Right" }).click();
  await expect(rows(page)).toHaveCount(0);
  await expect(page.getByText("All conflicts resolved").first()).toBeVisible();
  await expect(continueButton(page)).toBeEnabled();
  await page.getByText("Resolved in this session · 3").click();
  await expect(page.getByText("Accepted Right")).toHaveCount(3);
});

test("Cancelling the batch confirmation changes nothing", async ({ page }) => {
  await open(page);
  await page.getByRole("checkbox", { name: "Select all shown files" }).check();
  await page.getByRole("button", { name: "Accept Left", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Cancel" })
    .click();
  await expect(rows(page)).toHaveCount(3);
  expect((await calls(page)).filter((c) => c.startsWith("acceptMany"))).toEqual(
    [],
  );
});

test("Continue finishes a merge", async ({ page }) => {
  await open(page);
  await page.getByRole("checkbox", { name: "Select all shown files" }).check();
  await page.getByRole("button", { name: "Accept Left", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Accept Left" })
    .click();
  await continueButton(page).click();
  await expect(page.getByText("No conflicts to resolve").first()).toBeVisible();
  await expect(page.getByText("On branch").first()).toBeVisible();
});

test("Rebase continues to the next stop and the list repopulates", async ({
  page,
}) => {
  await open(page, "rebase2");
  await page.getByRole("checkbox", { name: "Select all shown files" }).check();
  await page.getByRole("button", { name: "Accept Right", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Accept Right" })
    .click();
  await continueButton(page).click();
  const banner = page.getByRole("region", { name: "Operation in progress" });
  await expect(banner).toContainText("commit 2 of 2: Tidy checkout");
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first()).toContainText("checkout.ts");
});

test("Abort asks for confirmation", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Abort merge…" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Abort the merge?");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  expect(await calls(page)).not.toContain("abort");
  await page.getByRole("button", { name: "Abort merge…" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Abort merge" })
    .click();
  await expect(page.getByText("No conflicts to resolve").first()).toBeVisible();
  expect(await calls(page)).toContain("abort");
});

test("Git errors are shown verbatim in the output panel", async ({ page }) => {
  await open(
    page,
    "merge3",
    "&failContinue=error%3A%20commit%20failed%0Ahint%3A%20fix%20it",
  );
  await page.getByRole("checkbox", { name: "Select all shown files" }).check();
  await page.getByRole("button", { name: "Accept Left", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Accept Left" })
    .click();
  await continueButton(page).click();
  const output = page.getByText("Git output");
  await expect(output).toBeVisible();
  await expect(page.locator("details pre")).toHaveText(
    "error: commit failed\nhint: fix it",
  );
});

test("Modify/delete row shows per-side changes and opens the modify/delete panel", async ({
  page,
}) => {
  await open(page, "modifydelete");
  const row = rows(page).filter({ hasText: "Coupon.kt" });
  await expect(row.getByLabel("Left: Deleted, Right: Modified")).toBeVisible();
  await row.getByRole("button", { name: /^Merge/ }).click();
  const panel = page.getByRole("region", { name: "Modify/delete conflict" });
  await expect(panel).toContainText("Deleted on left");
  await panel.getByRole("button", { name: "Keep modified" }).click();
  await expect(rows(page)).toHaveCount(1);
  // The resolved file's tab is gone (auto-advance moves on to the remaining conflict).
  await expect(page.getByRole("tab", { name: /Coupon/ })).toHaveCount(0);
});

test("Merge opens a tab; opening it again focuses the same tab", async ({
  page,
}) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await expect(page.getByRole("tab")).toHaveCount(1);
  await rows(page)
    .nth(1)
    .getByRole("button", { name: /^Merge/ })
    .click();
  await expect(page.getByRole("tab")).toHaveCount(2);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await expect(page.getByRole("tab")).toHaveCount(2);
  await expect(page.getByRole("tab", { selected: true })).toContainText(
    "app.ts",
  );
});

test("Auto-advance: a resolved save opens the next file in the same tab", async ({
  page,
}) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  await page
    .getByRole("button", { name: "Ignore right change" })
    .first()
    .click();
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await expect(rows(page)).toHaveCount(2);
  await expect(page.getByRole("tab")).toHaveCount(1);
  await expect(page.getByRole("tab", { selected: true })).toContainText(
    "util.ts",
  );
  await page.getByText("Resolved in this session · 1").click();
  await expect(page.getByText("Merged")).toBeVisible();
});

test("Reopen conflict restores a resolved file", async ({ page }) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: "Accept Left for app.ts" })
    .click();
  await expect(rows(page)).toHaveCount(2);
  await page.getByText("Resolved in this session · 1").click();
  await page
    .getByRole("button", { name: "Reopen conflict src/app.ts" })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Reopen conflict" })
    .click();
  await expect(rows(page)).toHaveCount(3);
  await expect(page.getByText("Resolved in this session")).toHaveCount(0);
});

test("External resolution removes the file and flags its open tab", async ({
  page,
}) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await page.evaluate(() =>
    window.__mergeiqRepo!.externalResolve("src/app.ts"),
  );
  await expect(rows(page)).toHaveCount(2, { timeout: 1000 });
  const notice = page
    .getByRole("status")
    .filter({ hasText: "resolved outside MergeIQ" });
  await expect(notice).toBeVisible();
  await notice.getByRole("button", { name: "Close tab" }).click();
  await expect(page.getByRole("tab")).toHaveCount(0);
});

test("Unsaved work: a dirty tab shows a dot and guards Abort", async ({
  page,
}) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  await expect(
    page.getByRole("img", { name: "Unsaved changes" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Abort merge…" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Unsaved changes");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  await dialog.getByRole("button", { name: "Discard changes" }).click();
  await expect(page.getByRole("dialog")).toContainText("Abort the merge?");
});

test("Unsaved work: closing the window prompts; Cancel keeps it open", async ({
  page,
}) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  const allowed = await page.evaluate(() =>
    window.__mergeiqRepo!.requestClose(),
  );
  expect(allowed).toBe(false);
  await expect(page.getByRole("dialog")).toContainText("close this window");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Cancel" })
    .click();
  expect(await calls(page)).not.toContain("closeWindow");
  await page.evaluate(() => window.__mergeiqRepo!.requestClose());
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Discard changes" })
    .click();
  await expect.poll(() => calls(page)).toContain("closeWindow");
});

test("Unsaved work: closing a dirty tab prompts", async ({ page }) => {
  await open(page);
  await rows(page)
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  await page.getByRole("button", { name: "Close app.ts" }).click();
  await expect(page.getByRole("dialog")).toContainText("close this tab");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Discard changes" })
    .click();
  await expect(page.getByRole("tab")).toHaveCount(0);
});

test("Filter and folder grouping", async ({ page }) => {
  await open(page);
  await page.getByRole("searchbox", { name: "Filter files" }).fill("util");
  await expect(rows(page)).toHaveCount(1);
  await page.getByRole("searchbox", { name: "Filter files" }).fill("");
  await page.getByRole("button", { name: "Folders" }).click();
  await expect(page.getByText("src/", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("web/", { exact: true }).first()).toBeVisible();
  await expect(rows(page)).toHaveCount(3);
});

test("A thousand conflicts render only the visible rows", async ({ page }) => {
  await open(page, "many");
  await expect(page.getByText("1200 conflicted files")).toBeVisible();
  expect(await rows(page).count()).toBeLessThan(60);
  await page
    .getByRole("list", { name: "Conflicted files" })
    .evaluate((el) => (el.scrollTop = 30000));
  await expect(rows(page).first()).toBeVisible();
  expect(await rows(page).count()).toBeLessThan(60);
});

test("No operation and no conflicts shows the empty state", async ({
  page,
}) => {
  await open(page, "clean");
  await expect(page.getByText("No conflicts to resolve").first()).toBeVisible();
  await expect(page.getByText("On branch").first()).toBeVisible();
});

test("With no tab open the editor area offers the first unresolved file", async ({
  page,
}) => {
  await open(page);
  await expect(page.getByText("Select a file to resolve")).toBeVisible();
  await page.getByRole("main").getByRole("button", { name: "app.ts" }).click();
  await expect(page.getByRole("tab")).toHaveCount(1);
});
