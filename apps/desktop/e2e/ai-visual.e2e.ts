import { expect, test, type Page } from "@playwright/test";

// Baselines are per platform and Chromium-only, like the other visual tests
// (`pnpm exec playwright test ai-visual --update-snapshots`).
test.skip(
  ({ browserName }) => browserName !== "chromium",
  "visual baselines are Chromium-only",
);

const FILE = "structural-ts";

async function ready(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(0, 0);
}

async function editor(page: Page, theme: string, extra = "") {
  await page.goto(`/dev/merge?fixture=${FILE}&theme=${theme}${extra}`);
  await page.locator("[data-pane=result] .cm-content").waitFor();
}

const panel = (page: Page) =>
  page.getByRole("complementary", { name: "AI assistant" });
const sparkles = (page: Page) =>
  page.getByRole("button", { name: "AI assistant for this conflict" });

async function open(page: Page) {
  await sparkles(page).first().click();
  await expect(panel(page)).toBeVisible();
}

async function suggest(page: Page) {
  await open(page);
  await panel(page)
    .getByRole("button", { name: "Suggest a resolution" })
    .click();
}

interface Case {
  name: string;
  extra?: string;
  /** Drives the editor into the state to capture; returns the element to shoot. */
  run: (page: Page) => Promise<void>;
  wait: string | RegExp;
  /** Capture the whole window instead of just the panel. */
  full?: boolean;
}

const CASES: Case[] = [
  {
    name: "suggestion",
    full: true,
    run: suggest,
    wait: "High confidence",
  },
  {
    name: "suggestion-syntax-warning",
    extra: "&ai=syntax",
    run: suggest,
    wait: "Suggestion may not compile.",
  },
  {
    name: "suggestion-medium",
    extra: "&ai=medium",
    run: suggest,
    wait: "Medium confidence",
  },
  {
    name: "suggestion-idle",
    run: open,
    wait: "Ask for a resolution of this conflict.",
  },
  {
    name: "explain-idle",
    run: async (page) => {
      await open(page);
      await panel(page).getByRole("tab", { name: "Explain" }).click();
    },
    wait: "Explain this conflict",
  },
  {
    name: "explain-streaming",
    extra: "&aiDelay=600000",
    run: async (page) => {
      await open(page);
      await panel(page).getByRole("tab", { name: "Explain" }).click();
      await panel(page)
        .getByRole("button", { name: "Explain this conflict" })
        .click();
    },
    wait: "Explaining…",
  },
  {
    name: "explain-done",
    run: async (page) => {
      await open(page);
      await panel(page).getByRole("tab", { name: "Explain" }).click();
      await panel(page)
        .getByRole("button", { name: "Explain this conflict" })
        .click();
      await panel(page)
        .getByRole("button", { name: "Explain again" })
        .waitFor();
    },
    wait: "Explain again",
  },
  {
    name: "waiting",
    extra: "&aiDelay=600000",
    run: suggest,
    wait: /Asking claude-opus-5/,
  },
  {
    name: "gate-notice",
    extra: "&ai=notice",
    run: suggest,
    wait: "Your code leaves this computer",
  },
  {
    name: "gate-notice-local",
    extra: "&ai=ollama-notice",
    run: suggest,
    wait: "Requests stay on this computer",
  },
  {
    name: "gate-optin",
    extra: "&ai=unasked",
    run: suggest,
    wait: "Use AI in standalone merges?",
  },
  {
    name: "gate-declined",
    extra: "&ai=declined",
    run: suggest,
    wait: /AI is turned off for/,
  },
  {
    name: "gate-excluded",
    extra: "&path=config/.env.production",
    run: suggest,
    wait: "File excluded from AI by your settings",
  },
  {
    name: "error-refused",
    extra: "&ai=refused",
    run: suggest,
    wait: "The model declined this request",
  },
  {
    name: "error-truncated",
    extra: "&ai=truncated",
    run: suggest,
    wait: "The response was cut off",
  },
  {
    name: "error-network",
    extra: "&ai=error",
    run: suggest,
    wait: "The request failed",
  },
  {
    name: "queue-review",
    full: true,
    run: async (page) => {
      await page.getByRole("button", { name: /AI: suggest remaining/ }).click();
      await panel(page).getByText("High confidence").waitFor();
    },
    wait: /\d of \d/,
  },
];

for (const theme of ["dark", "light"]) {
  for (const c of CASES) {
    test(`Visual: ${c.name} (${theme})`, async ({ page }) => {
      await editor(page, theme, c.extra);
      await c.run(page);
      await expect(page.getByText(c.wait).first()).toBeVisible();
      await ready(page);
      const target = c.full ? page : panel(page);
      await expect(target).toHaveScreenshot(`ai-${c.name}-${theme}.png`, {
        maxDiffPixelRatio: 0.01,
        animations: "disabled",
      });
    });
  }

  test(`Visual: preview request (${theme})`, async ({ page }) => {
    await editor(page, theme);
    await open(page);
    await panel(page).getByRole("button", { name: "Preview request" }).click();
    const dialog = page.getByRole("dialog", { name: "Preview request" });
    await expect(page.getByTestId("ai-payload")).toBeVisible();
    await ready(page);
    await expect(dialog).toHaveScreenshot(`ai-preview-${theme}.png`, {
      maxDiffPixelRatio: 0.01,
    });
  });

  test(`Visual: toolbar without a provider (${theme})`, async ({ page }) => {
    await editor(page, theme, "&ai=unconfigured");
    await expect(page.getByRole("button", { name: "Set up AI" })).toBeVisible();
    await ready(page);
    await expect(
      page.getByRole("toolbar", { name: "Merge actions" }),
    ).toHaveScreenshot(`ai-setup-link-${theme}.png`, {
      maxDiffPixelRatio: 0.01,
    });
  });

  test(`Visual: toolbar with AI (${theme})`, async ({ page }) => {
    await editor(page, theme);
    await expect(page.getByTestId("ai-estimate")).toBeVisible();
    await ready(page);
    await expect(
      page.getByRole("toolbar", { name: "Merge actions" }),
    ).toHaveScreenshot(`ai-toolbar-${theme}.png`, { maxDiffPixelRatio: 0.01 });
  });
}
