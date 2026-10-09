import type { Locator, Page } from "@playwright/test";

export const mod = process.platform === "darwin" ? "Meta" : "Control";

export async function open(page: Page, fixture: string, extra = "") {
  await page.goto(`/dev/merge?fixture=${fixture}${extra}`);
  await page.locator("[data-pane=result] .cm-content").waitFor();
}

export const resultContent = (page: Page): Locator =>
  page.locator("[data-pane=result] .cm-content");

/** Text of a pane's lines, joined with `\n` (placeholders and widgets excluded). */
export async function paneText(
  page: Page,
  pane: "left" | "right" | "base" | "result",
): Promise<string> {
  return page
    .locator(`[data-pane=${pane}] .cm-content`)
    .evaluate((el) =>
      [...el.querySelectorAll(".cm-line")]
        .map((l) => l.textContent ?? "")
        .join("\n"),
    );
}

export const counter = (page: Page) => page.getByTestId("counter");

export async function saves(page: Page) {
  return page.evaluate(() => window.__mergeiqSaves ?? []);
}

declare global {
  interface Window {
    __mergeiqSaves?: unknown[];
    __mergeiqCancels?: number;
  }
}
