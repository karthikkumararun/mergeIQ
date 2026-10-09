import { expect, test } from "@playwright/test";
import { open, paneText } from "./helpers";

/** Vertical offset of the pane line containing `text`, relative to the pane's top. */
async function lineY(
  page: import("@playwright/test").Page,
  pane: string,
  text: string,
) {
  return page.evaluate(
    ([p, t]) => {
      const root = document.querySelector(`[data-pane=${p}]`)!;
      const line = [...root.querySelectorAll(".cm-line")].find(
        (l) => l.textContent === t,
      );
      if (!line) return null;
      return (
        line.getBoundingClientRect().top - root.getBoundingClientRect().top
      );
    },
    [pane, text] as const,
  );
}

test("Next conflict", async ({ page }) => {
  await open(page, "multi-conflict-file");
  await page.getByRole("button", { name: "Next conflict (F7)" }).click();
  await expect(
    page.locator("[data-pane=result] .cm-merge-current"),
  ).toHaveCount(1);
  await expect(page.getByTestId("cursor")).toHaveText("Ln 1, Col 1");
  await page.keyboard.press("F7");
  await expect(page.getByTestId("cursor")).toHaveText("Ln 6, Col 1");
  // wraps with a visible hint
  await page.keyboard.press("F7");
  await expect(page.getByTestId("cursor")).toHaveText("Ln 1, Col 1");
  await expect(
    page.getByRole("status").filter({ hasText: "Wrapped to first conflict" }),
  ).toBeVisible();
  await page.keyboard.press("Shift+F7");
  await expect(page.getByTestId("cursor")).toHaveText("Ln 6, Col 1");
});

test("Next and previous change", async ({ page }) => {
  await open(page, "mixed-changes");
  await page.getByRole("button", { name: "Next change (Alt+Down)" }).click();
  await expect(page.getByTestId("cursor")).toHaveText("Ln 2, Col 1");
  await page.keyboard.press("Alt+ArrowDown");
  await expect(page.getByTestId("cursor")).toHaveText("Ln 5, Col 1");
  await page.getByRole("button", { name: "Previous change (Alt+Up)" }).click();
  await expect(page.getByTestId("cursor")).toHaveText("Ln 2, Col 1");
});

test("Scroll alignment", async ({ page }) => {
  await open(page, "synthetic-400-40-shift");
  const left = page.locator("[data-pane=left] .cm-scroller");
  await left.evaluate((el) => {
    el.scrollTop = el.scrollHeight - el.clientHeight - 200;
  });
  // pick an unchanged line visible in the left pane and compare its position everywhere
  const pick = () =>
    page.evaluate(() => {
      const root = document.querySelector("[data-pane=left]")!;
      const box = root.getBoundingClientRect();
      const onScreen = [...root.querySelectorAll(".cm-line")].filter((l) => {
        const r = l.getBoundingClientRect();
        return (
          r.top > box.top + 40 &&
          r.bottom < box.bottom - 40 &&
          /^row \d+$/.test(l.textContent ?? "")
        );
      });
      // after the scroll has rendered, the visible rows are near the end of the file
      const mid = onScreen[Math.floor(onScreen.length / 2)];
      return mid && Number(mid.textContent!.slice(4)) > 300
        ? mid.textContent!
        : null;
    });
  await expect.poll(pick).not.toBeNull();
  const target = (await pick())!;
  await expect
    .poll(async () => {
      const [l, r, x] = [
        await lineY(page, "left", target),
        await lineY(page, "result", target),
        await lineY(page, "right", target),
      ];
      return (
        l !== null &&
        r !== null &&
        x !== null &&
        Math.abs(l - r) <= 22 &&
        Math.abs(l - x) <= 22
      );
    })
    .toBe(true);
});

test("Sync scroll can be turned off", async ({ page }) => {
  await open(page, "synthetic-400-40");
  await page.getByRole("button", { name: "Sync scroll" }).click();
  await expect(
    page.getByRole("button", { name: "Sync scroll" }),
  ).toHaveAttribute("aria-pressed", "false");
  await page.locator("[data-pane=left] .cm-scroller").evaluate((el) => {
    el.scrollTop = 600;
  });
  await page.waitForTimeout(200);
  expect(
    await page
      .locator("[data-pane=result] .cm-scroller")
      .evaluate((el) => el.scrollTop),
  ).toBe(0);
});

test("Toggle base", async ({ page }) => {
  await open(page, "mixed-changes");
  await expect(page.locator("[data-pane=base]")).toHaveCount(0);
  await page.getByRole("button", { name: "Show base" }).click();
  await expect(page.locator("[data-pane=base]")).toHaveCount(1);
  expect(await paneText(page, "base")).toBe(
    "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n",
  );
  await expect(page.locator("[data-pane=base] .cm-merge-con")).toHaveCount(1);
  await expect(
    page.locator("[data-pane=base] .cm-merge-marks [aria-label]"),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Show base" }).click();
  await expect(page.locator("[data-pane=base]")).toHaveCount(0);
});

test("Collapse unchanged folds aligned regions", async ({ page }) => {
  await open(page, "synthetic-400-100");
  await page.getByRole("button", { name: "Collapse unchanged" }).click();
  for (const pane of ["left", "result", "right"]) {
    await expect(
      page.locator(`[data-pane=${pane}] .cm-foldPlaceholder`).first(),
    ).toBeVisible();
  }
  const counts = await Promise.all(
    ["left", "result", "right"].map((p) =>
      page.locator(`[data-pane=${p}] .cm-foldPlaceholder`).count(),
    ),
  );
  expect(new Set(counts).size).toBe(1);
  const labels = await Promise.all(
    ["left", "result", "right"].map((p) =>
      page
        .locator(`[data-pane=${p}] .cm-foldPlaceholder`)
        .first()
        .textContent(),
    ),
  );
  expect(new Set(labels).size).toBe(1);
  await page.getByRole("button", { name: "Collapse unchanged" }).click();
  await expect(page.locator(".cm-foldPlaceholder")).toHaveCount(0);
});

test("Show base persists across reloads", async ({ page }) => {
  await open(page, "mixed-changes");
  await page.getByRole("button", { name: "Show base" }).click();
  await page.getByRole("button", { name: "Collapse unchanged" }).click();
  await page.reload();
  await expect(page.locator("[data-pane=base]")).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Show base" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    page.getByRole("button", { name: "Collapse unchanged" }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Show base" }).click();
  await page.getByRole("button", { name: "Collapse unchanged" }).click();
});
