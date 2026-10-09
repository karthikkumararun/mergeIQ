import { expect, test, type Page } from "@playwright/test";

async function openEditor(page: Page, extra = "") {
  await page.goto(`/dev/repo?scenario=merge3${extra}`);
  await page.getByRole("region", { name: "Operation in progress" }).waitFor();
  await page
    .getByTestId("conflict-row")
    .first()
    .getByRole("button", { name: /^Merge/ })
    .click();
  await page.locator("[data-pane=result] .cm-content").waitFor();
}

const panel = (page: Page) =>
  page.getByRole("complementary", { name: "AI assistant" });

test("In a repository window the opt-in names the repository", async ({
  page,
}) => {
  await openEditor(page, "&ai=unasked");
  await page
    .getByRole("button", { name: "AI assistant for this conflict" })
    .first()
    .click();
  await panel(page)
    .getByRole("button", { name: "Suggest a resolution" })
    .click();
  const card = panel(page).getByRole("region", {
    name: "Allow AI in this repository",
  });
  await expect(card).toContainText("Use AI in shop-web?");
  await card.getByRole("button", { name: "Allow AI here" }).click();
  await expect(panel(page).getByText("High confidence")).toBeVisible();
});

test("An allowed repository goes straight to the suggestion", async ({
  page,
}) => {
  await openEditor(page);
  await page
    .getByRole("button", { name: "AI assistant for this conflict" })
    .first()
    .click();
  await panel(page)
    .getByRole("button", { name: "Suggest a resolution" })
    .click();
  await expect(panel(page).getByText("High confidence")).toBeVisible();
});
