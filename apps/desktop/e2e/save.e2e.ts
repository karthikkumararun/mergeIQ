import { expect, test } from "@playwright/test";
import { open, saves } from "./helpers";

type Saved = {
  mode: string;
  lines: { text: string; term: string }[];
  unresolvedIds: number[];
  unresolved: unknown[];
};

test("Clean save", async ({ page }) => {
  await open(page, "simple-conflict");
  await page.getByRole("button", { name: /^Apply left change/ }).click();
  await page.getByRole("button", { name: "Ignore right change" }).click();
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const [saved] = (await saves(page)) as Saved[];
  expect(saved.mode).toBe("resolved");
  expect(saved.unresolvedIds).toEqual([]);
  expect(saved.lines.map((l) => l.text)).toEqual(["line1", "OURS", "line3"]);
});

test("Save with unresolved: continue resolving is the default", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("1 conflict and 2 changes are unresolved");
  await expect(dialog).toContainText("mixed-changes.txt");
  await expect(
    dialog.getByRole("button", { name: /Continue resolving/ }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId("cursor")).toHaveText("Ln 5, Col 1");
  expect(await saves(page)).toHaveLength(0);
});

test("Save with unresolved: conflict markers, not staged", async ({ page }) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await page
    .getByRole("button", { name: /Save with conflict markers/ })
    .click();
  const [saved] = (await saves(page)) as Saved[];
  expect(saved.mode).toBe("markers");
  expect(saved.unresolved).toHaveLength(1);
  expect(saved.unresolvedIds).toEqual([0, 1, 2]);
});

test("Save with unresolved: mark as resolved anyway", async ({ page }) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await page.getByRole("button", { name: /Mark as resolved anyway/ }).click();
  const [saved] = (await saves(page)) as Saved[];
  expect(saved.mode).toBe("force");
  expect(saved.lines.map((l) => l.text)).toEqual([
    "l1",
    "l2",
    "l3",
    "l4",
    "l5",
    "l6",
    "l7",
    "l8",
    "l9",
    "l10",
  ]);
});

test("Escape in the save dialog keeps editing", async ({ page }) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await saves(page)).toHaveLength(0);
});

test("Cancel without changes closes immediately", async ({ page }) => {
  await open(page, "mixed-changes");
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await page.evaluate(() => window.__mergeiqCancels)).toBe(1);
});

test("Cancel with changes asks before discarding", async ({ page }) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: /^Apply left change/ })
    .first()
    .click();
  await page.getByRole("button", { name: "Cancel" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Discard your changes?");
  await dialog.getByRole("button", { name: "Keep editing" }).click();
  expect(await page.evaluate(() => window.__mergeiqCancels ?? 0)).toBe(0);
  await page.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: "Discard changes" }).click();
  expect(await page.evaluate(() => window.__mergeiqCancels)).toBe(1);
});

test("Save errors are shown and the editor stays open", async ({ page }) => {
  await open(page, "simple-conflict", "&failSave=1");
  await page.getByRole("button", { name: /^Apply left change/ }).click();
  await page.getByRole("button", { name: "Ignore right change" }).click();
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "Could not save: disk full",
  );
});
