import { expect, test } from "@playwright/test";
import { counter, open, paneText, saves } from "./helpers";

test("Keyboard-only resolution", async ({ page }) => {
  await open(page, "simple-conflict");
  await page.keyboard.press("F7");
  await expect(page.getByTestId("cursor")).toHaveText("Ln 2, Col 1");
  await page.keyboard.press("ControlOrMeta+Alt+ArrowLeft");
  expect(await paneText(page, "result")).toBe("line1\nOURS\nline3\n");
  await page.keyboard.press("ControlOrMeta+Alt+ArrowRight");
  expect(await paneText(page, "result")).toBe("line1\nOURS\nTHEIRS\nline3\n");
  await expect(counter(page)).toHaveText("All changes processed");
  await page.keyboard.press("ControlOrMeta+s");
  await expect.poll(async () => (await saves(page)).length).toBe(1);
});

test("Keyboard: apply left then save opens the save flow for what is left", async ({
  page,
}) => {
  await open(page, "simple-conflict");
  await page.keyboard.press("F7");
  await page.keyboard.press("ControlOrMeta+Alt+ArrowLeft");
  await page.keyboard.press("ControlOrMeta+s");
  await expect(page.getByRole("dialog")).toContainText("1 conflict");
});

test("Keyboard: ignore the current chunk and apply all non-conflicting", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  await page.keyboard.press("ControlOrMeta+Alt+a");
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n",
  );
  await page.keyboard.press("F7");
  await page.keyboard.press("ControlOrMeta+Alt+Backspace");
  await expect(counter(page)).toHaveText("All changes processed");
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n",
  );
});

test("Keyboard: resolve simple conflicts", async ({ page }) => {
  await open(page, "simple-resolvable");
  await page.keyboard.press("ControlOrMeta+Alt+m");
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
});

test("Keyboard: tab order reaches toolbar buttons with visible focus", async ({
  page,
  browserName,
}) => {
  test.skip(
    browserName === "webkit",
    "macOS WebKit does not Tab to buttons unless full keyboard access is on",
  );
  await open(page, "mixed-changes");
  await page.getByRole("button", { name: "Cancel" }).focus();
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("button", { name: /^Apply/ }).filter({ hasText: "S" }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("button", { name: "Apply non-conflicting changes" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("menu")).toBeVisible();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter"); // "Left only"
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n",
  );
});
