import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { open } from "./helpers";

async function serious(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  return results.violations
    .filter((v) => v.impact === "serious" || v.impact === "critical")
    .map(
      (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`,
    );
}

for (const theme of ["dark", "light"]) {
  test(`No serious accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "mixed-changes", `&theme=${theme}`);
    expect(await serious(page)).toEqual([]);
  });

  test(`No serious accessibility violations with base and folds (${theme})`, async ({
    page,
  }) => {
    await open(
      page,
      "synthetic-300-40",
      `&theme=${theme}&showBase=1&collapse=1`,
    );
    expect(await serious(page)).toEqual([]);
  });
}

test("No serious accessibility violations with the commit popover open", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  await page.locator("[data-header=right] button").first().click();
  expect(await serious(page)).toEqual([]);
});

test("No serious accessibility violations in the save dialog", async ({
  page,
}) => {
  await open(page, "mixed-changes");
  await page
    .getByRole("button", { name: /^Apply/ })
    .filter({ hasText: "S" })
    .click();
  expect(await serious(page)).toEqual([]);
});

test("Every icon-only control has an accessible name", async ({ page }) => {
  await open(page, "mixed-changes");
  const unnamed = await page.evaluate(() =>
    [...document.querySelectorAll("button")]
      .filter(
        (b) => !(b.textContent ?? "").trim() && !b.getAttribute("aria-label"),
      )
      .map((b) => b.outerHTML.slice(0, 80)),
  );
  expect(unnamed).toEqual([]);
});
