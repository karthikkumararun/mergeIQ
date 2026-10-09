import { expect, test, type Page } from "@playwright/test";

// Baselines are per platform and Chromium-only
// (`pnpm exec playwright test ai-settings-visual --update-snapshots`).
test.skip(
  ({ browserName }) => browserName !== "chromium",
  "visual baselines are Chromium-only",
);

async function open(page: Page, theme: string, extra = "") {
  await page.goto(`/dev/ai-settings?theme=${theme}${extra}`);
  await page.getByRole("heading", { name: "AI", level: 1 }).waitFor();
  await page.getByTestId("ai-notice-state").waitFor();
}

async function ready(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(0, 0);
}

interface Case {
  name: string;
  extra?: string;
  run?: (page: Page) => Promise<void>;
  wait?: string | RegExp;
}

const CASES: Case[] = [
  { name: "anthropic", wait: "Connected" },
  {
    name: "anthropic-tested",
    run: async (page) => {
      await page.getByRole("button", { name: "Test connection" }).click();
    },
    wait: "Connected · claude-opus-5 responded in 0.8 s",
  },
  {
    name: "test-failed",
    run: async (page) => {
      await page.getByRole("button", { name: "Replace key…" }).click();
      await page.getByLabel("API key").fill("bad");
      await page.getByRole("button", { name: "Test connection" }).click();
    },
    wait: /invalid x-api-key/,
  },
  {
    name: "ollama",
    extra: "&provider=ollama",
    wait: "No API key is needed.",
  },
  {
    name: "no-key",
    extra: "&key=0&configured=0&notice=0&repos=0",
    wait: "No key stored",
  },
];

for (const theme of ["dark", "light"]) {
  for (const c of CASES) {
    test(`Visual: settings ${c.name} (${theme})`, async ({ page }) => {
      await open(page, theme, c.extra);
      await c.run?.(page);
      if (c.wait && c.name !== "anthropic")
        await expect(page.getByText(c.wait).first()).toBeVisible();
      await ready(page);
      await expect(page).toHaveScreenshot(
        `ai-settings-${c.name}-${theme}.png`,
        {
          fullPage: true,
          maxDiffPixelRatio: 0.01,
        },
      );
    });
  }
}
