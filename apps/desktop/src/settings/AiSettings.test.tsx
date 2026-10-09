import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createMockAiApi, type MockAiOptions } from "../ai/mockApi";
import { AiSettings } from "./AiSettings";

function setup(options: MockAiOptions = {}) {
  const api = createMockAiApi({
    repos: {
      "/Users/ada/code/shop-web": "Allowed",
      "/Users/ada/code/payments-service": "Declined",
    },
    ...options,
  });
  render(<AiSettings api={api} />);
  return { api, user: userEvent.setup() };
}

describe("Settings › AI › Provider", () => {
  it("shows the provider form with a masked key and where it is stored", async () => {
    setup();
    expect(
      await screen.findByRole("radio", { name: "Anthropic" }),
    ).toBeChecked();
    expect(screen.getByLabelText("Model")).toHaveValue("claude-opus-5");
    expect(screen.getByLabelText("Base URL")).toHaveValue(
      "https://api.anthropic.com",
    );
    expect(screen.getByLabelText("Effort")).toHaveValue("high");
    expect(screen.getByTestId("ai-key-mask")).toHaveTextContent(
      "•••• •••• a9F2",
    );
    expect(screen.getByText("in macOS Keychain")).toBeVisible();
    // Only the last four characters ever reach the page.
    expect(document.body.textContent).not.toContain("sk-ant-mock");
    expect(screen.getByRole("button", { name: "Replace key…" })).toBeVisible();
  });

  it("switching provider saves it, and Ollama has no key row", async () => {
    const { api, user } = setup();
    await user.click(
      await screen.findByRole("radio", { name: "Ollama (local)" }),
    );
    await waitFor(() => expect(api.settings.provider).toBe("ollama"));
    expect(api.settings.confirmed).toBe(true);
    expect(screen.getByLabelText("Model")).toHaveValue("qwen2.5-coder");
    expect(screen.getByLabelText("Base URL")).toHaveValue(
      "http://localhost:11434",
    );
    expect(screen.queryByTestId("ai-key-mask")).toBeNull();
    expect(screen.queryByLabelText("Effort")).toBeNull();
    expect(screen.getByText(/No API key is needed/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Test connection" }));
    expect(await screen.findByText(/Connected · qwen2.5-coder/)).toBeVisible();
  });

  it("model and URL are saved on blur, not on every keystroke", async () => {
    const { api, user } = setup();
    const model = await screen.findByLabelText("Model");
    await user.clear(model);
    await user.type(model, "claude-sonnet-5-5");
    expect(api.settings.providers?.anthropic).toBeUndefined();
    await user.tab();
    await waitFor(() =>
      expect(api.settings.providers?.anthropic?.model).toBe(
        "claude-sonnet-5-5",
      ),
    );
    // An emptied field reverts instead of saving nothing.
    await user.clear(model);
    await user.tab();
    expect(model).toHaveValue("claude-sonnet-5-5");
  });

  it("Test connection reports success with the latency", async () => {
    const { user } = setup();
    await user.click(
      await screen.findByRole("button", { name: "Test connection" }),
    );
    expect(
      await screen.findByText("Connected · claude-opus-5 responded in 0.8 s"),
    ).toBeVisible();
  });

  it("Test connection reports the provider's error", async () => {
    const { user } = setup();
    await user.click(
      await screen.findByRole("button", { name: "Replace key…" }),
    );
    await user.type(screen.getByLabelText("API key"), "bad");
    await user.click(screen.getByRole("button", { name: "Test connection" }));
    expect(
      await screen.findByText(/rejected the credentials: invalid x-api-key/),
    ).toBeVisible();
  });

  it("Replace key stores the new key and shows only its last four characters", async () => {
    const { api, user } = setup();
    await user.click(
      await screen.findByRole("button", { name: "Replace key…" }),
    );
    const input = screen.getByLabelText("API key");
    expect(input).toHaveAttribute("type", "password");
    await user.type(input, "sk-ant-api03-new-key-Q7x1");
    await user.click(screen.getByRole("button", { name: "Save key" }));
    await waitFor(() =>
      expect(screen.getByTestId("ai-key-mask")).toHaveTextContent(
        "•••• •••• Q7x1",
      ),
    );
    expect((await api.keyInfo("anthropic")).last4).toBe("Q7x1");
    expect(
      screen.queryByLabelText("API key", { selector: "input" }),
    ).toBeNull();
    expect(document.body.textContent).not.toContain("new-key");
    expect(JSON.stringify(api.settings)).not.toContain("new-key");
  });

  it("without a key it offers Add key and can remove one", async () => {
    const a = setup({ hasKey: false });
    expect(await screen.findByTestId("ai-key-mask")).toHaveTextContent(
      "No key stored",
    );
    expect(a.user).toBeDefined();
    expect(screen.getByRole("button", { name: "Add key…" })).toBeVisible();
  });

  it("removing the key leaves none stored", async () => {
    const { api, user } = setup();
    await user.click(
      await screen.findByRole("button", {
        name: /Remove key from macOS Keychain/,
      }),
    );
    await waitFor(() =>
      expect(screen.getByTestId("ai-key-mask")).toHaveTextContent(
        "No key stored",
      ),
    );
    expect((await api.keyInfo("anthropic")).present).toBe(false);
  });
});

describe("Settings › AI › Privacy", () => {
  it("lists the default exclusion globs as removable chips", async () => {
    const { api, user } = setup();
    const group = await screen.findByRole("group", {
      name: "Never send files matching",
    });
    for (const g of ["**/.env*", "**/*secret*", "**/*.pem", "**/*.key"])
      expect(within(group).getByText(g)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Remove **/*.pem" }));
    await waitFor(() =>
      expect(api.settings.privacy?.excludeGlobs).not.toContain("**/*.pem"),
    );
    expect(screen.queryByText("**/*.pem")).toBeNull();
  });

  it("adds a glob on Enter and rejects a broken one", async () => {
    const { api, user } = setup();
    const input = await screen.findByLabelText("Add exclusion glob");
    await user.type(input, "vendor/**{Enter}");
    await waitFor(() =>
      expect(api.settings.privacy?.excludeGlobs).toContain("vendor/**"),
    );
    expect(input).toHaveValue("");
    await user.type(input, "a[[b{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent(/Unbalanced/);
    expect(api.settings.privacy?.excludeGlobs).not.toContain("a[b");
  });

  it("repositories can be disabled and enabled", async () => {
    const { api, user } = setup();
    expect(await screen.findByText("shop-web")).toBeVisible();
    expect(screen.getAllByText("AI allowed")).toHaveLength(2);
    expect(screen.getByText("Declined")).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "Disable AI for shop-web" }),
    );
    await waitFor(() =>
      expect(api.settings.privacy?.repos?.["/Users/ada/code/shop-web"]).toBe(
        "Declined",
      ),
    );
    await user.click(
      screen.getByRole("button", { name: "Enable AI for payments-service" }),
    );
    await waitFor(() =>
      expect(
        api.settings.privacy?.repos?.["/Users/ada/code/payments-service"],
      ).toBe("Allowed"),
    );
  });

  it("the data-sharing notice can be shown again", async () => {
    const { api, user } = setup();
    expect(await screen.findByTestId("ai-notice-state")).toHaveTextContent(
      "accepted",
    );
    await user.click(screen.getByRole("button", { name: "Show it again" }));
    await waitFor(() => expect(api.settings.noticeAccepted).toBe(false));
    expect(screen.getByTestId("ai-notice-state")).toHaveTextContent(
      "not accepted",
    );
  });
});

describe("Settings › AI › Context and prices", () => {
  it("surrounding lines and budget are clamped and saved", async () => {
    const { api, user } = setup();
    const lines = await screen.findByLabelText("Surrounding lines");
    expect(lines).toHaveValue(40);
    expect(screen.getByLabelText("Input token budget")).toHaveValue(60000);
    await user.clear(lines);
    await user.type(lines, "9999");
    await user.tab();
    await waitFor(() =>
      expect(api.settings.context?.surroundingLines).toBe(500),
    );
    expect(lines).toHaveValue(500);
  });

  it("prices are editable, new models can be added and removed", async () => {
    const { api, user } = setup();
    const out = await screen.findByLabelText("claude-opus-5 output price");
    expect(out).toHaveValue(25);
    await user.clear(out);
    await user.type(out, "30");
    await user.tab();
    await waitFor(() =>
      expect(api.settings.prices?.entries?.["claude-opus-5"]?.output).toBe(30),
    );
    await user.type(
      screen.getByLabelText("Model to add a price for"),
      "claude-sonnet-5{Enter}",
    );
    expect(
      await screen.findByLabelText("claude-sonnet-5 input price"),
    ).toHaveValue(0);
    await user.click(
      screen.getByRole("button", { name: "Remove price for claude-sonnet-5" }),
    );
    await waitFor(() =>
      expect(api.settings.prices?.entries?.["claude-sonnet-5"]).toBeUndefined(),
    );
  });

  it("usage starts empty and Reset clears the session totals", async () => {
    const { api, user } = setup();
    expect(await screen.findByTestId("ai-usage-summary")).toHaveTextContent(
      "No requests since MergeIQ started",
    );
    await api.suggest(null, "r1", {
      path: "a.ts",
      base: "x\n",
      left: { label: "l", text: "L\n", commits: [] },
      right: { label: "r", text: "R\n", commits: [] },
      result: "x\n",
      chunk: {
        base: { start: 0, end: 1 },
        left: { start: 0, end: 1 },
        right: { start: 0, end: 1 },
        result: { start: 0, end: 1 },
      },
    });
    // The page reads usage on load; a fresh render shows it.
    const again = render(<AiSettings api={api} />);
    expect(
      await within(again.container).findByTestId("ai-usage-summary"),
    ).toHaveTextContent("Session: 1 request");
    await user.click(
      within(again.container).getByRole("button", { name: "Reset" }),
    );
    await waitFor(() =>
      expect(
        within(again.container).getByTestId("ai-usage-summary"),
      ).toHaveTextContent("No requests"),
    );
  });
});
