import { expect, test } from "@playwright/test";
import { counter, open, paneText } from "./helpers";

test("Open a conflicted file", async ({ page }) => {
  await open(page, "mixed-changes");
  await expect(page.getByTestId("left-label")).toHaveText("main");
  await expect(page.getByTestId("right-label")).toHaveText("feature");
  await expect(page.locator("[data-header=left]")).toContainText(
    "Left · ours · read-only",
  );
  await expect(page.locator("[data-header=right]")).toContainText(
    "Right · theirs · read-only",
  );
  await expect(page.getByText("mixed-changes.txt").first()).toBeVisible();
  await expect(page.getByText("Merging feature into main")).toBeVisible();
});

test("Default open leaves the result equal to base", async ({ page }) => {
  await open(page, "mixed-changes");
  expect(await paneText(page, "result")).toBe(
    "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n",
  );
  await expect(counter(page)).toHaveText("3 changes · 1 conflict left");
});

test("Auto-apply enabled applies non-conflicting chunks on open", async ({
  page,
}) => {
  await open(page, "mixed-changes", "&autoApply=1");
  expect(await paneText(page, "result")).toBe(
    "l1\nOURS2\nl3\nl4\nl5\nl6\nl7\nl8\nTHEIRS9\nl10\n",
  );
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  await page.locator("[data-pane=result] .cm-content").click();
  await page.keyboard.press("ControlOrMeta+z");
  expect(await paneText(page, "result")).toBe(
    "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n",
  );
});

test("Panes default to equal widths and are resizable", async ({ page }) => {
  await open(page, "mixed-changes");
  const width = (p: string) =>
    page
      .locator(`[data-pane=${p}]`)
      .evaluate((e) => e.getBoundingClientRect().width);
  const [l, r, x] = [
    await width("left"),
    await width("result"),
    await width("right"),
  ];
  expect(Math.abs(l - r)).toBeLessThan(2);
  expect(Math.abs(r - x)).toBeLessThan(2);
  const handle = page.getByRole("separator", { name: "Resize right pane" });
  await handle.focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  expect(await width("result")).toBeGreaterThan(r + 20);
  expect(await width("right")).toBeLessThan(x - 20);
});

test("Chunk highlighting: conflict colors and non-color marks", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  for (const pane of ["left", "result", "right"]) {
    const line = page.locator(`[data-pane=${pane}] .cm-line.cm-merge-con`);
    await expect(line).toHaveCount(1);
    await expect(
      page.locator(`[data-pane=${pane}] [aria-label=Conflict]`),
    ).toHaveText("!");
  }
  await expect(
    page.locator("[data-pane=result] [aria-label=Modified]"),
  ).toHaveCount(2);
  // The ours-only change is not highlighted in the (unchanged) right pane.
  await expect(page.locator("[data-pane=right] .cm-merge-mod")).toHaveCount(1);
});

test("Word highlight marks changed tokens", async ({ page }) => {
  await open(page, "simple-resolvable");
  await expect(
    page.locator("[data-pane=left] .cm-merge-em-con").first(),
  ).toHaveText("x");
  await expect(
    page.locator("[data-pane=right] .cm-merge-em-con").first(),
  ).toHaveText("y");
  await expect(
    page.locator("[data-pane=result] .cm-merge-em-con").first(),
  ).toHaveText("a");
});

test("Resolved chunks are muted with a check", async ({ page }) => {
  await open(page, "simple-conflict");
  await page
    .getByRole("button", { name: /^Apply left change to result/ })
    .click();
  await page
    .getByRole("button", { name: /^Append right change to result/ })
    .click();
  await expect(page.locator("[data-pane=result] .cm-merge-res")).toHaveCount(2);
  await expect(
    page.locator("[data-pane=result] [aria-label=Resolved]"),
  ).toHaveText("✓");
});

test("Kotlin file gets syntax highlighting in all panes", async ({ page }) => {
  await open(page, "kotlin-sample");
  await expect(page.getByText("Kotlin").first()).toBeVisible();
  for (const pane of ["left", "result", "right"]) {
    await expect
      .poll(() =>
        page.locator(`[data-pane=${pane}] .cm-content span[class]`).count(),
      )
      .toBeGreaterThan(0);
  }
});
