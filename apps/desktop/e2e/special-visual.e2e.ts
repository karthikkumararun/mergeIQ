import { expect, test, type Page } from "@playwright/test";

// Baselines are per platform and Chromium-only, like the other visual tests
// (`pnpm exec playwright test special-visual --update-snapshots`).
test.skip(
  ({ browserName }) => browserName !== "chromium",
  "visual baselines are Chromium-only",
);

async function ready(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(0, 0);
}

async function openPanel(page: Page, theme: string, file: string, extra = "") {
  // Relative dates ("8d") must not depend on today's date.
  await page.clock.setFixedTime(new Date("2026-10-09T12:00:00Z"));
  await page.goto(`/dev/repo?scenario=special&theme=${theme}${extra}`);
  await page.getByRole("region", { name: "Operation in progress" }).waitFor();
  await page
    .getByTestId("conflict-row")
    .filter({ hasText: file })
    .getByRole("button", { name: /^Merge/ })
    .click();
}

interface Case {
  name: string;
  file: string;
  panel: string;
  extra?: string;
  /** Extra steps once the panel is open, then a text that must be visible. */
  prepare?: (page: Page) => Promise<void>;
  wait: string | RegExp;
}

const chooseRight = async (page: Page) =>
  page.getByRole("radio", { name: /Take Right and regenerate/ }).click();
const run = async (page: Page) => {
  await chooseRight(page);
  await page.getByRole("button", { name: "Run command" }).click();
};

const CASES: Case[] = [
  {
    name: "modify-delete",
    file: "Coupon.kt",
    panel: "Modify/delete conflict",
    wait: "What main changed since base",
  },
  {
    name: "image",
    file: "logo.png",
    panel: "Binary conflict",
    // Every preview must have been decoded and measured before the screenshot.
    prepare: async (page) => {
      await expect(page.getByRole("img")).toHaveCount(3);
      await page.waitForFunction(() =>
        [...document.images].every((i) => i.complete && i.naturalWidth > 0),
      );
    },
    wait: "1024 × 1024 px",
  },
  {
    name: "symlink",
    file: "current",
    panel: "Symlink conflict",
    wait: "→ ../envs/prod-us",
  },
  {
    name: "lfs",
    file: "hero.mp4",
    panel: "Git LFS conflict",
    wait: /oid c25e88…b413/,
  },
  {
    name: "oversized",
    file: "orders-2026.csv",
    panel: "Oversized file conflict",
    wait: "52.3 MB",
  },
  {
    name: "submodule",
    file: "ui-kit",
    panel: "Submodule conflict",
    wait: "Recommended",
  },
  {
    name: "submodule-unknown",
    file: "icons",
    panel: "Submodule conflict",
    wait: /Ancestry unknown/,
  },
  {
    name: "rename",
    file: "src/promo/",
    panel: "Rename conflict",
    wait: "Final path",
  },
  {
    name: "gosum",
    file: "go.sum",
    panel: "go.sum conflict",
    wait: "Result preview",
  },
  {
    name: "lockfile",
    file: "pnpm-lock.yaml",
    panel: "Lockfile conflict",
    prepare: chooseRight,
    wait: "MergeIQ will run this command",
  },
  {
    name: "lockfile-running",
    file: "pnpm-lock.yaml",
    panel: "Lockfile conflict",
    extra: "&lockfile=hang",
    prepare: run,
    wait: "Running…",
  },
  {
    name: "lockfile-success",
    file: "pnpm-lock.yaml",
    panel: "Lockfile conflict",
    prepare: run,
    wait: /Exited 0 in 4\.2 s/,
  },
  {
    name: "lockfile-failed",
    file: "pnpm-lock.yaml",
    panel: "Lockfile conflict",
    extra: "&lockfile=fail",
    prepare: run,
    wait: /Exited 1 · not staged/,
  },
];

for (const theme of ["dark", "light"]) {
  for (const c of CASES) {
    test(`Visual: ${c.name} (${theme})`, async ({ page }) => {
      await openPanel(page, theme, c.file, c.extra);
      const panel = page.getByRole("region", { name: c.panel });
      await c.prepare?.(page);
      await expect(panel.getByText(c.wait).first()).toBeVisible();
      await ready(page);
      // Just the panel: the conflict list beside it scrolls by timing.
      await expect(panel).toHaveScreenshot(`special-${c.name}-${theme}.png`, {
        maxDiffPixelRatio: 0.01,
      });
    });
  }
}
