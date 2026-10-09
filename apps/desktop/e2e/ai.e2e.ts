import { expect, test, type Page } from "@playwright/test";
import { counter, mod, open, paneText } from "./helpers";

declare global {
  interface Window {
    __mergeiqAi?: {
      calls: { sent: number; suggest: number; openSettings: string[] };
      settings: {
        noticeAccepted?: boolean;
        privacy?: { repos?: Record<string, string> };
      };
    };
  }
}

const sparkles = (page: Page) =>
  page.getByRole("button", { name: "AI assistant for this conflict" });
const panel = (page: Page) =>
  page.getByRole("complementary", { name: "AI assistant" });
const remaining = (page: Page, n: number) =>
  page.getByRole("button", { name: `AI: suggest remaining (${n})` });
const sent = (page: Page) =>
  page.evaluate(() => window.__mergeiqAi?.calls.sent);

async function suggest(page: Page) {
  await sparkles(page).first().click();
  await panel(page)
    .getByRole("button", { name: "Suggest a resolution" })
    .click();
}

test("Not configured: only a Set up AI link is shown", async ({ page }) => {
  await open(page, "multi-conflict-file", "&ai=unconfigured");
  await expect(page.getByRole("button", { name: "Set up AI" })).toBeVisible();
  await expect(sparkles(page)).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: /AI: suggest remaining/ }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Set up AI" }).click();
  expect(
    await page.evaluate(() => window.__mergeiqAi?.calls.openSettings),
  ).toEqual(["ai"]);
});

test("Conflicts get a ✦ marker and the toolbar shows the count and the token estimate", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await expect(sparkles(page)).toHaveCount(2);
  await expect(remaining(page, 2)).toBeVisible();
  await expect(page.getByTestId("ai-estimate")).toHaveText(
    "≈ 12k input tokens",
  );
  // The marker opens the panel for its conflict and outlines it in the Result.
  await sparkles(page).nth(1).click();
  await expect(panel(page)).toBeVisible();
  await expect(sparkles(page).nth(1)).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator("[data-pane=result] .cm-ai-active")).toHaveCount(1);
});

test("Explain streams text incrementally and Cancel stops it", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&aiDelay=80");
  await sparkles(page).first().click();
  await panel(page).getByRole("tab", { name: "Explain" }).click();
  await panel(page)
    .getByRole("button", { name: "Explain this conflict" })
    .click();

  const text = page.getByTestId("ai-explanation");
  // The first words arrive quickly, then the text keeps growing.
  await expect(text).not.toHaveText("", { timeout: 3000 });
  const early = (await text.textContent())!.length;
  await expect
    .poll(async () => (await text.textContent())!.length, { timeout: 5000 })
    .toBeGreaterThan(early);

  await panel(page).getByRole("button", { name: "Cancel" }).click();
  await expect(panel(page).getByText("Cancelled.")).toBeVisible();
  const stopped = await text.textContent();
  await page.waitForTimeout(500);
  expect(await text.textContent()).toBe(stopped);
  await expect(
    panel(page).getByRole("button", { name: "Try again" }),
  ).toBeVisible();
});

test("Explain finishes with usage and offers a suggestion", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await sparkles(page).first().click();
  await panel(page).getByRole("tab", { name: "Explain" }).click();
  await panel(page)
    .getByRole("button", { name: "Explain this conflict" })
    .click();
  await expect(page.getByTestId("ai-explanation")).toContainText(
    "A correct merge keeps both changes",
  );
  await expect(page.getByTestId("ai-usage")).toContainText(
    "claude-opus-5 · 6.2k in · 410 out · 3.1 s",
  );
  await panel(page)
    .getByRole("button", { name: "Suggest a resolution" })
    .click();
  await expect(panel(page).getByText("High confidence")).toBeVisible();
});

test("Suggest shows the preview; Apply replaces the chunk in one undoable step", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
  const before = await paneText(page, "result");

  await suggest(page);
  await expect(panel(page).getByText("High confidence")).toBeVisible();
  await expect(panel(page).getByText("Strategy: both")).toBeVisible();
  await expect(
    panel(page).getByText("Current result → suggestion"),
  ).toBeVisible();
  const diff = panel(page).getByLabel("Changes the suggestion makes");
  await expect(diff).toContainText("A1");
  await expect(diff).toContainText("A2");
  await expect(
    panel(page).getByRole("heading", { name: "Risks" }),
  ).toBeVisible();
  // Nothing changes until Apply is clicked.
  expect(await paneText(page, "result")).toBe(before);

  await panel(page).getByRole("button", { name: "Apply" }).click();
  await expect(panel(page)).toHaveCount(0);
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  expect(await paneText(page, "result")).toContain("A1\nA2\nb");
  await expect(remaining(page, 1)).toBeVisible();
  await expect(sparkles(page)).toHaveCount(1);

  await page.locator("[data-pane=result] .cm-content").click();
  await page.keyboard.press(`${mod}+z`);
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
  expect(await paneText(page, "result")).toBe(before);
});

test("Regenerate asks again; Dismiss clears the suggestion", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await suggest(page);
  await panel(page).getByRole("button", { name: "Regenerate" }).click();
  await expect(page.getByTestId("ai-usage")).toContainText(
    "6.2k in (4.8k cached)",
  );
  await expect(page.getByTestId("ai-usage")).toContainText(
    "Session: 2 requests",
  );
  await panel(page).getByRole("button", { name: "Dismiss" }).click();
  await expect(
    panel(page).getByRole("button", { name: "Suggest a resolution" }),
  ).toBeVisible();
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
});

test("A suggestion that may not compile is flagged and still needs a click", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&ai=syntax");
  const before = await paneText(page, "result");
  await suggest(page);
  await expect(panel(page).getByRole("alert")).toContainText(
    "Suggestion may not compile.",
  );
  await expect(panel(page).getByText("Low confidence")).toBeVisible();
  expect(await paneText(page, "result")).toBe(before);
  await panel(page).getByRole("button", { name: "Apply" }).click();
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
});

test("A refusal shows the decline message and nothing to apply", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&ai=refused");
  await suggest(page);
  await expect(panel(page).getByRole("alert")).toContainText(
    "The model declined this request",
  );
  await expect(panel(page).getByRole("button", { name: "Apply" })).toHaveCount(
    0,
  );
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
});

test("Review queue: suggestions for every conflict, stepped through in order", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await remaining(page, 2).click();
  await expect(page.getByTestId("ai-queue-counter")).toHaveText("1 of 2");
  await expect(panel(page).getByText("High confidence")).toBeVisible();
  expect(await sent(page)).toBe(2);

  await panel(page).getByRole("button", { name: "Apply" }).click();
  await expect(page.getByTestId("ai-queue-counter")).toHaveText("2 of 2");
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");

  await panel(page).getByRole("button", { name: "Dismiss" }).click();
  await expect(panel(page)).toHaveCount(0);
  // The dismissed one is untouched.
  await expect(counter(page)).toHaveText("1 change · 1 conflict left");
  await expect(remaining(page, 1)).toBeVisible();
});

test("Review queue: Edit hands the conflict back and moves on", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await remaining(page, 2).click();
  await panel(page).getByRole("button", { name: "Edit" }).click();
  await expect(page.getByTestId("ai-queue-counter")).toHaveText("2 of 2");
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
});

test("An excluded file is refused with the settings message", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&path=config/.env.production");
  await expect(
    page.getByText("File excluded from AI by your settings").first(),
  ).toBeVisible();
  await expect(remaining(page, 2)).toBeDisabled();
  await suggest(page);
  await expect(panel(page).getByRole("alert")).toContainText(
    "File excluded from AI by your settings",
  );
  expect(await sent(page)).toBe(0);
});

test("Repository opt-in: asked once, and declining sends nothing", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&ai=unasked");
  await suggest(page);
  const card = panel(page).getByRole("region", {
    name: "Allow AI in this repository",
  });
  await expect(card).toBeVisible();
  await card.getByRole("button", { name: "Don't allow" }).click();
  await expect(
    panel(page).getByRole("region", { name: "AI is turned off here" }),
  ).toBeVisible();
  expect(await sent(page)).toBe(0);
  expect(
    await page.evaluate(
      () => window.__mergeiqAi?.settings.privacy?.repos?.standalone,
    ),
  ).toBe("Declined");
  await expect(counter(page)).toHaveText("2 changes · 2 conflicts left");
});

test("Repository opt-in: Allow continues straight to the suggestion", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&ai=unasked");
  await suggest(page);
  await panel(page).getByRole("button", { name: "Allow AI here" }).click();
  await expect(panel(page).getByText("High confidence")).toBeVisible();
  expect(await sent(page)).toBe(1);
});

test("A repository that was declined stays off", async ({ page }) => {
  await open(page, "multi-conflict-file", "&ai=declined");
  await suggest(page);
  await expect(
    panel(page).getByRole("region", { name: "AI is turned off here" }),
  ).toBeVisible();
  expect(await sent(page)).toBe(0);
});

test("First use shows the data-sharing notice before anything is sent", async ({
  page,
}) => {
  await open(page, "multi-conflict-file", "&ai=notice");
  await suggest(page);
  const card = panel(page).getByRole("region", { name: "Data-sharing notice" });
  await expect(card).toContainText("Your code leaves this computer");
  await expect(card).toContainText("api.anthropic.com");
  expect(await sent(page)).toBe(0);
  await card.getByRole("button", { name: "Accept and continue" }).click();
  await expect(panel(page).getByText("High confidence")).toBeVisible();
  expect(
    await page.evaluate(() => window.__mergeiqAi?.settings.noticeAccepted),
  ).toBe(true);
});

test("Preview request shows the exact text and where it goes", async ({
  page,
}) => {
  await open(page, "multi-conflict-file");
  await sparkles(page).first().click();
  await panel(page).getByRole("button", { name: "Preview request" }).click();
  const dialog = page.getByRole("dialog", { name: "Preview request" });
  await expect(dialog).toContainText("Sent to api.anthropic.com");
  await expect(page.getByTestId("ai-payload")).toContainText("<file_context>");
  expect(await sent(page)).toBe(0);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(panel(page)).toBeVisible();
});

test("The panel closes with Escape and the close button", async ({ page }) => {
  await open(page, "multi-conflict-file");
  await sparkles(page).first().click();
  await expect(panel(page)).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(panel(page)).toHaveCount(0);
  await sparkles(page).first().click();
  await panel(page).getByRole("button", { name: "Close AI assistant" }).click();
  await expect(panel(page)).toHaveCount(0);
});
