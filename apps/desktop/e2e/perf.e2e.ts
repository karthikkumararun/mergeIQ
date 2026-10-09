import { expect, test } from "@playwright/test";
import { open } from "./helpers";

// Event Timing (duration includes presentation) is Chromium-only.
test.skip(
  ({ browserName }) => browserName !== "chromium",
  "performance budgets are measured in Chromium",
);

test("Large file smoke test: first paint within 1 s", async ({ page }) => {
  await open(page, "synthetic-20000-100");
  await page.waitForFunction(
    () => performance.getEntriesByName("mergeiq:first-paint").length > 0,
  );
  const paint = await page.evaluate(
    () =>
      performance.getEntriesByName("mergeiq:first-paint")[0].startTime -
      performance.getEntriesByName("mergeiq:analysis-ready")[0].startTime,
  );
  console.log(`first paint ${Math.round(paint)} ms after analysis`);
  expect(paint).toBeLessThan(1000);
});

test("Large file smoke test: typing latency under 50 ms", async ({ page }) => {
  await page.addInitScript(() => {
    (window as unknown as { __timings: number[] }).__timings = [];
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        if (
          ["keydown", "keypress", "keyup", "input", "beforeinput"].includes(
            e.name,
          )
        ) {
          (window as unknown as { __timings: number[] }).__timings.push(
            e.duration,
          );
        }
      }
    }).observe({ type: "event", durationThreshold: 16 });
  });
  await open(page, "synthetic-20000-100");
  await page
    .locator("[data-pane=result] .cm-line", { hasText: "row 10" })
    .first()
    .click();
  for (let i = 0; i < 40; i++) {
    await page.keyboard.type("x");
    await page.waitForTimeout(25);
  }
  await page.waitForTimeout(200);
  const timings = await page.evaluate(
    () => (window as unknown as { __timings: number[] }).__timings,
  );
  const worst = timings.length ? Math.max(...timings) : 0;
  console.log(`typing: ${timings.length} slow events, worst ${worst} ms`);
  expect(worst).toBeLessThan(50);
});
