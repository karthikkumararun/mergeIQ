import { expect, test, type Page } from "@playwright/test";
import { counter, open, paneText } from "./helpers";

const applyLeft = (page: Page) =>
  page.getByRole("button", { name: /^Apply left change to result/ });
const appendRight = (page: Page) =>
  page.getByRole("button", { name: /^Append right change to result/ });
const applyRight = (page: Page) =>
  page.getByRole("button", { name: /^Apply right change to result/ });

test("Apply then append", async ({ page }) => {
  await open(page, "simple-conflict");
  await applyLeft(page).click();
  expect(await paneText(page, "result")).toBe("line1\nOURS\nline3\n");
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  // the right side's Apply becomes Append
  await expect(applyRight(page)).toHaveCount(0);
  await appendRight(page).click();
  expect(await paneText(page, "result")).toBe("line1\nOURS\nTHEIRS\nline3\n");
  await expect(counter(page)).toHaveText("All changes processed");
});

test("Ignore both sides", async ({ page }) => {
  await open(page, "simple-conflict");
  await page.getByRole("button", { name: "Ignore left change" }).click();
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  await page.getByRole("button", { name: "Ignore right change" }).click();
  expect(await paneText(page, "result")).toBe("line1\nline2\nline3\n");
  await expect(counter(page)).toHaveText("All changes processed");
});

test("Undo restores status", async ({ page }) => {
  await open(page, "simple-conflict");
  await applyLeft(page).click();
  await expect(appendRight(page)).toBeVisible();
  await page.locator("[data-pane=result] .cm-content").click();
  await page.keyboard.press("ControlOrMeta+z");
  expect(await paneText(page, "result")).toBe("line1\nline2\nline3\n");
  await expect(applyLeft(page)).toBeVisible();
  await expect(applyRight(page)).toBeVisible();
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  await page.keyboard.press("ControlOrMeta+Shift+z");
  expect(await paneText(page, "result")).toBe("line1\nOURS\nline3\n");
});

test("Edit resolves chunk", async ({ page }) => {
  await open(page, "simple-conflict");
  await page
    .locator("[data-pane=result] .cm-line", { hasText: "line2" })
    .click();
  await page.keyboard.press("End");
  await page.keyboard.type("!");
  await expect(counter(page)).toHaveText("All changes processed");
  await expect(
    page.getByRole("button", { name: "Revert chunk to base" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Revert chunk to base" }).click();
  expect(await paneText(page, "result")).toBe("line1\nline2\nline3\n");
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
});

test("Band follows edits", async ({ page }) => {
  await open(page, "simple-conflict");
  const resultTop = () =>
    page
      .locator("[data-gutter=left] [data-band='0'] path")
      .nth(1)
      .getAttribute("d")
      .then((d) => {
        const nums = d!.match(/-?\d+(\.\d+)?/g)!.map(Number);
        return nums[nums.length - 1];
      });
  const before = await resultTop();
  await page
    .locator("[data-pane=result] .cm-line", { hasText: "line1" })
    .click();
  await page.keyboard.press("Home");
  await page.keyboard.type("a");
  await page.keyboard.press("Enter");
  await page.keyboard.type("b");
  await page.keyboard.press("Enter");
  await page.keyboard.type("c");
  await page.keyboard.press("Enter");
  await expect.poll(resultTop).toBe(before + 66);
});

test("Resolve simple conflicts (magic wand)", async ({ page }) => {
  await open(page, "simple-resolvable");
  await expect(counter(page)).toHaveText("3 changes · 3 conflicts left");
  await page.getByRole("button", { name: "Resolve simple" }).click();
  expect(await paneText(page, "result")).toBe(
    "a\nfoo(x, y)\nb\nbar(x, y)\nc\nk = 1\nd\n",
  );
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  await expect(
    page.getByRole("button", { name: "Resolve simple" }),
  ).toBeDisabled();
  await page.locator("[data-pane=result] .cm-content").click();
  await page.keyboard.press("ControlOrMeta+z");
  await expect(counter(page)).toHaveText("3 changes · 3 conflicts left");
});

test("Apply non-conflicting changes menu", async ({ page }) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: "Apply non-conflicting changes" })
    .click();
  await page.getByRole("menuitem", { name: "Left only" }).click();
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n",
  );
  await page
    .getByRole("button", { name: "Apply non-conflicting changes" })
    .click();
  await page.getByRole("menuitem", { name: /^All/ }).click();
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n",
  );
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
});

test("Accept Right replaces the whole result, confirming after manual edits", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  await page.getByRole("button", { name: "Accept Right" }).click();
  expect(await paneText(page, "result")).toBe(
    "l1\nl2\nl3\nl4\nTHEIRS5\nl6\nl7\nl8\nTHEIRS9\nl10\n",
  );
  await expect(counter(page)).toHaveText("All changes processed");

  await page.reload();
  await page.locator("[data-pane=result] .cm-line", { hasText: "l5" }).click();
  await page.keyboard.type("!");
  await page.getByRole("button", { name: "Accept Left" }).click();
  await expect(page.getByRole("dialog")).toContainText(
    "Replace the result with main?",
  );
  await page.getByRole("button", { name: "Accept Left" }).last().click();
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nOURS5\nl6\nl7\nl8\nl9\nl10\n",
  );
});
