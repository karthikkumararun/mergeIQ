import { expect, test, type Page } from "@playwright/test";

declare global {
  interface Window {
    __mergeiqAi?: {
      settings: {
        provider?: string;
        confirmed?: boolean;
        privacy?: { excludeGlobs?: string[]; repos?: Record<string, string> };
      };
    };
  }
}

async function open(page: Page, extra = "") {
  await page.goto(`/dev/ai-settings?x=1${extra}`);
  await page.getByRole("heading", { name: "AI", level: 1 }).waitFor();
}

test("Provider form shows the masked key and its store", async ({ page }) => {
  await open(page);
  await expect(page.getByRole("radio", { name: "Anthropic" })).toBeChecked();
  await expect(page.getByLabel("Model", { exact: true })).toHaveValue(
    "claude-opus-5",
  );
  await expect(page.getByTestId("ai-key-mask")).toHaveText("•••• •••• a9F2");
  await expect(page.getByText("in macOS Keychain")).toBeVisible();
  await expect(page.locator("body")).not.toContainText("sk-ant-mock");
});

test("Test connection reports success, and the provider's error for a bad key", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Test connection" }).click();
  await expect(page.getByTestId("ai-test-result")).toHaveText(
    "Connected · claude-opus-5 responded in 0.8 s",
  );
  await page.getByRole("button", { name: "Replace key…" }).click();
  await page.getByLabel("API key").fill("bad");
  await page.getByRole("button", { name: "Test connection" }).click();
  await expect(page.getByTestId("ai-test-result")).toContainText(
    "invalid x-api-key",
  );
});

test("Switching to Ollama removes the key row and saves the choice", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("radio", { name: "Ollama (local)" }).click();
  await expect(page.getByLabel("Model", { exact: true })).toHaveValue(
    "qwen2.5-coder",
  );
  await expect(page.getByLabel("Base URL")).toHaveValue(
    "http://localhost:11434",
  );
  await expect(page.getByTestId("ai-key-mask")).toHaveCount(0);
  await expect(page.getByText("No API key is needed.")).toBeVisible();
  expect(await page.evaluate(() => window.__mergeiqAi?.settings.provider)).toBe(
    "ollama",
  );
});

test("Replacing the key shows only the last four characters", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Replace key…" }).click();
  await page.getByLabel("API key").fill("sk-ant-api03-brand-new-Zq81");
  await page.getByRole("button", { name: "Save key" }).click();
  await expect(page.getByTestId("ai-key-mask")).toHaveText("•••• •••• Zq81");
  await expect(page.locator("body")).not.toContainText("brand-new");
  expect(
    await page.evaluate(() => JSON.stringify(window.__mergeiqAi?.settings)),
  ).not.toContain("brand-new");
});

test("Exclusion globs are chips; repositories can be enabled and disabled", async ({
  page,
}) => {
  await open(page);
  const globs = page.getByRole("group", { name: "Never send files matching" });
  await expect(globs.getByText("**/.env*")).toBeVisible();
  await globs.getByLabel("Add exclusion glob").fill("vendor/**");
  await page.keyboard.press("Enter");
  await expect(globs.getByText("vendor/**")).toBeVisible();
  await page.getByRole("button", { name: "Remove **/*.key" }).click();
  await expect(globs.getByText("**/*.key")).toHaveCount(0);
  expect(
    await page.evaluate(
      () => window.__mergeiqAi?.settings.privacy?.excludeGlobs,
    ),
  ).toEqual(["**/.env*", "**/*secret*", "**/*.pem", "vendor/**"]);

  await page.getByRole("button", { name: "Disable AI for shop-web" }).click();
  await expect(
    page.getByRole("button", { name: "Enable AI for shop-web" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Enable AI for payments-service" })
    .click();
  await expect(
    page.getByRole("button", { name: "Disable AI for payments-service" }),
  ).toBeVisible();
});

test("Prices are editable", async ({ page }) => {
  await open(page);
  const out = page.getByLabel("claude-opus-5 output price");
  await out.fill("30");
  await out.blur();
  await page.getByLabel("Model to add a price for").fill("claude-sonnet-5");
  await page.getByRole("button", { name: "Add model" }).click();
  await expect(page.getByLabel("claude-sonnet-5 input price")).toHaveValue("0");
});

test("The AI section is reachable from the Settings navigation", async ({
  page,
}) => {
  await open(page);
  await expect(
    page.getByRole("button", { name: "AI", exact: true }),
  ).toHaveAttribute("aria-current", "page");
});
