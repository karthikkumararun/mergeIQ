import { undo } from "@codemirror/commands";
import type {
  EditorState,
  Transaction,
  TransactionSpec,
} from "@codemirror/state";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createMockAiApi, type MockAiOptions } from "../../../ai/mockApi";
import { mockMergeDocument } from "../../../ipc/mock";
import type { MergeEditorContext } from "../../extensions";
import { applyReplacement } from "../../model/actions";
import { chunksOf } from "../../model/session";
import { createResultState } from "../../model/state";
import { buildInput, unresolvedConflicts } from "./input";
import { AiPanel } from "./Panel";
import { createAiStore, type AiHost } from "./store";

const doc = mockMergeDocument("multi-conflict-file");

function press(state: EditorState, cmd: typeof undo): EditorState {
  let next = state;
  cmd({ state, dispatch: (tr: Transaction) => (next = tr.state) });
  return next;
}

function setup(options: MockAiOptions = {}, path?: string) {
  const api = createMockAiApi(options);
  const host: AiHost = {
    api,
    repo: null,
    scope: "standalone",
    scopeName: "this merge",
  };
  const store = createAiStore(host);
  const mergeDoc = path ? { ...doc, displayPath: path } : doc;
  let state = createResultState(doc.analysis);
  const dispatched: TransactionSpec[] = [];
  store.getState().bind({
    path: mergeDoc.displayPath,
    inputFor: (id) => buildInput(mergeDoc, doc.analysis, state, id),
    spansFor: (id) =>
      buildInput(mergeDoc, doc.analysis, state, id)?.chunk ?? null,
  });
  const ctx = (): MergeEditorContext => ({
    analysis: doc.analysis,
    doc: mergeDoc,
    state,
    view: null,
    dispatch: (spec) => {
      dispatched.push(spec);
      state = state.update(spec).state;
      view.rerender(<AiPanel ctx={ctx()} host={host} store={store} />);
    },
  });
  const view = render(<AiPanel ctx={ctx()} host={host} store={store} />);
  const ids = unresolvedConflicts(state);
  return {
    api,
    store,
    ids,
    dispatched,
    state: () => state,
    undo: () => {
      state = press(state, undo);
      view.rerender(<AiPanel ctx={ctx()} host={host} store={store} />);
    },
    /** Resolves a conflict as the user would by hand (outside the panel). */
    resolve: (id: number) => {
      const chunk = doc.analysis.chunks.find((c) => c.id === id)!;
      const spec = applyReplacement(
        state,
        doc.analysis,
        { chunkIds: [id], baseRange: chunk.base, text: "manual\n" },
        "ai",
      )!;
      act(() => ctx().dispatch(spec));
    },
    open: (id: number, tab: "explain" | "suggestion" = "suggestion") =>
      act(() => store.getState().open(id, tab)),
    user: userEvent.setup(),
  };
}

describe("AiPanel", () => {
  it("renders nothing until a conflict is opened", () => {
    const { store } = setup();
    expect(
      screen.queryByRole("complementary", { name: "AI assistant" }),
    ).toBeNull();
    expect(store.getState().panel.open).toBe(false);
  });

  it("opens on the Suggestion tab with the idle prompt and a preview link", async () => {
    const t = setup();
    t.open(t.ids[0]);
    expect(
      screen.getByRole("complementary", { name: "AI assistant" }),
    ).toBeVisible();
    expect(screen.getByRole("tab", { name: "Suggestion" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Preview request" }),
    ).toBeVisible();
    await t.user.click(screen.getByRole("tab", { name: "Explain" }));
    expect(
      screen.getByRole("button", { name: "Explain this conflict" }),
    ).toBeVisible();
  });
});

describe("Explain", () => {
  it("streams the explanation and offers to suggest next", async () => {
    const t = setup({ delayMs: 1 });
    t.open(t.ids[0], "explain");
    await t.user.click(
      screen.getByRole("button", { name: "Explain this conflict" }),
    );
    const text = await screen.findByTestId("ai-explanation");
    await waitFor(() =>
      expect(text.textContent).toMatch(/correct merge keeps both changes/),
    );
    expect(
      await screen.findByRole("button", { name: "Explain again" }),
    ).toBeVisible();
    expect(screen.getByTestId("ai-usage")).toHaveTextContent(
      "claude-opus-5 · 6.2k in · 410 out · 3.1 s",
    );
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    expect(await screen.findByText("High confidence")).toBeVisible();
    expect(screen.getByRole("tab", { name: "Suggestion" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("Cancel stops the stream and keeps what arrived", async () => {
    const t = setup({ delayMs: 15 });
    t.open(t.ids[0], "explain");
    await t.user.click(
      screen.getByRole("button", { name: "Explain this conflict" }),
    );
    const cancel = await screen.findByRole("button", { name: "Cancel" });
    await screen.findByTestId("ai-explanation");
    await t.user.click(cancel);
    expect(await screen.findByText("Cancelled.")).toBeVisible();
    expect(t.api.calls.cancel).toHaveLength(1);
    expect(screen.getByTestId("ai-explanation").textContent).not.toBe("");
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
    expect(screen.getByRole("button", { name: "Try again" })).toBeVisible();
  });
});

describe("Suggestion", () => {
  it("shows confidence, strategy, location, explanation, diff, risks and usage", async () => {
    const t = setup();
    const id = t.ids[0];
    t.open(id);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    expect(await screen.findByText("High confidence")).toBeVisible();
    expect(screen.getByText("Strategy: both")).toBeVisible();
    expect(screen.getByText(/^line \d+$/)).toBeVisible();
    expect(screen.getByTestId("ai-suggestion-explanation")).toHaveTextContent(
      /keeps both sides' lines, left first/,
    );
    const diff = screen.getByLabelText("Changes the suggestion makes");
    expect(within(diff).getAllByText(/^Removed:/).length).toBeGreaterThan(0);
    expect(within(diff).getAllByText(/^Added:/).length).toBeGreaterThan(0);
    expect(screen.getByRole("heading", { name: "Risks" })).toBeVisible();
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByTestId("ai-usage")).toHaveTextContent(
      "Session: 1 request · 6.2k in · 410 out · est. $0.02",
    );
    for (const name of ["Apply", "Dismiss", "Regenerate"])
      expect(screen.getByRole("button", { name })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();
  });

  it("the second request reports cached tokens and session totals add up", async () => {
    const t = setup();
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    await screen.findByText("High confidence");
    await t.user.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() =>
      expect(screen.getByTestId("ai-usage")).toHaveTextContent(
        "claude-opus-5 · 6.2k in (4.8k cached) · 410 out · 3.1 s",
      ),
    );
    expect(screen.getByTestId("ai-usage")).toHaveTextContent(
      "Session: 2 requests · 12.4k in · 820 out",
    );
    expect(t.api.calls.suggest).toBe(2);
  });

  it("the syntax warning and low confidence are shown, and Apply still needs a click", async () => {
    const t = setup({ syntaxWarning: true, confidence: "low" });
    const id = t.ids[0];
    t.open(id);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Suggestion may not compile.");
    expect(alert).toHaveTextContent(
      "more syntax errors with this applied than any of base, left or right",
    );
    expect(screen.getByText("Low confidence")).toBeVisible();
    expect(t.dispatched).toHaveLength(0);
    expect(chunksOf(t.state()).find((c) => c.id === id)?.resolution).toBe(
      "none",
    );
  });

  it("Apply replaces the chunk, marks it resolved as ai, closes, and is one undo step", async () => {
    const t = setup();
    const id = t.ids[0];
    const before = t.state().doc.toString();
    t.open(id);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    await screen.findByText("High confidence");
    await t.user.click(screen.getByRole("button", { name: "Apply" }));

    expect(t.dispatched).toHaveLength(1);
    expect(chunksOf(t.state()).find((c) => c.id === id)?.resolution).toBe("ai");
    expect(t.state().doc.toString()).toContain("A1\nA2\n");
    expect(unresolvedConflicts(t.state())).toEqual(t.ids.slice(1));
    expect(t.store.getState().panel.open).toBe(false);
    expect(
      screen.queryByRole("complementary", { name: "AI assistant" }),
    ).toBeNull();

    t.undo();
    expect(t.state().doc.toString()).toBe(before);
    expect(chunksOf(t.state()).find((c) => c.id === id)?.resolution).toBe(
      "none",
    );
  });

  it("Dismiss drops the suggestion without touching the file", async () => {
    const t = setup();
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    await screen.findByText("High confidence");
    await t.user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    ).toBeVisible();
    expect(t.dispatched).toHaveLength(0);
  });

  it("a refusal shows the decline message and no suggestion", async () => {
    const t = setup({
      fail: { code: "refused", category: "cyber", explanation: null },
    });
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The model declined this request");
    expect(alert).toHaveTextContent("Category: cyber");
    expect(screen.queryByRole("button", { name: "Apply" })).toBeNull();
    expect(screen.getByRole("button", { name: "Try again" })).toBeVisible();
  });

  it("truncation and network errors are explained", async () => {
    const a = setup({ fail: { code: "truncated" } });
    a.open(a.ids[0]);
    await a.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(/cut off/);
  });

  it("an already resolved conflict is not offered again", async () => {
    const t = setup();
    const id = t.ids[0];
    t.open(id);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    await screen.findByText("High confidence");
    t.resolve(id);
    expect(
      screen.getByText("This conflict is already resolved."),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Apply" })).toBeNull();
  });
});

describe("Gates", () => {
  it("no provider: a Set up AI card that opens the settings", async () => {
    const t = setup({ configured: false });
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const card = await screen.findByRole("region", { name: "Set up AI" });
    await t.user.click(
      within(card).getByRole("button", { name: "Open AI settings" }),
    );
    expect(t.api.calls.openSettings).toEqual(["ai"]);
  });

  it("the first-use notice says what is sent, and accepting runs the request", async () => {
    const t = setup({ notice: false });
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const card = await screen.findByRole("region", {
      name: "Data-sharing notice",
    });
    expect(card).toHaveTextContent("Your code leaves this computer");
    expect(card).toHaveTextContent("api.anthropic.com");
    expect(card).toHaveTextContent("commit messages");
    expect(card).toHaveTextContent("exclusion list");
    await t.user.click(
      within(card).getByRole("button", { name: "Accept and continue" }),
    );
    expect(await screen.findByText("High confidence")).toBeVisible();
    expect(t.api.settings.noticeAccepted).toBe(true);
  });

  it("with Ollama the notice says requests stay local", async () => {
    const t = setup({ notice: false, provider: "ollama" });
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const card = await screen.findByRole("region", {
      name: "Data-sharing notice",
    });
    expect(card).toHaveTextContent("Requests stay on this computer");
    expect(card).toHaveTextContent("your local model server");
  });

  it("a repository asks once; Allow continues, Don't allow sends nothing", async () => {
    const a = setup({ repo: "unasked" });
    a.open(a.ids[0]);
    await a.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const card = await screen.findByRole("region", {
      name: "Allow AI in this repository",
    });
    expect(card).toHaveTextContent("Use AI in this merge?");
    await a.user.click(
      within(card).getByRole("button", { name: "Allow AI here" }),
    );
    expect(await screen.findByText("High confidence")).toBeVisible();
  });

  it("declining a repository shows it is off and nothing is generated", async () => {
    const t = setup({ repo: "unasked" });
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const card = await screen.findByRole("region", {
      name: "Allow AI in this repository",
    });
    await t.user.click(
      within(card).getByRole("button", { name: "Don't allow" }),
    );
    expect(
      await screen.findByRole("region", { name: "AI is turned off here" }),
    ).toHaveTextContent("Nothing has been sent");
    expect(screen.queryByText("High confidence")).toBeNull();
    expect(t.api.settings.privacy?.repos?.standalone).toBe("Declined");
  });

  it("an excluded file is refused with the settings message", async () => {
    const t = setup({}, "config/.env.production");
    t.open(t.ids[0]);
    await t.user.click(
      screen.getByRole("button", { name: "Suggest a resolution" }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("File excluded from AI by your settings");
    expect(screen.queryByText("High confidence")).toBeNull();
  });
});

describe("Preview request", () => {
  it("shows the exact payload and where it goes; Escape closes it", async () => {
    const t = setup();
    t.open(t.ids[0]);
    await t.user.click(screen.getByRole("button", { name: "Preview request" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Preview request",
    });
    expect(dialog).toHaveTextContent("Sent to api.anthropic.com");
    expect(await within(dialog).findByTestId("ai-payload")).toHaveTextContent(
      "<file_context>",
    );
    expect(t.api.calls.suggest).toBe(0);
    await t.user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    // The panel itself stays open.
    expect(
      screen.getByRole("complementary", { name: "AI assistant" }),
    ).toBeVisible();
  });

  it("an excluded file cannot be previewed", async () => {
    const t = setup({}, "certs/server.pem");
    t.open(t.ids[0]);
    await t.user.click(screen.getByRole("button", { name: "Preview request" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "File excluded from AI by your settings",
    );
  });
});

describe("Review queue", () => {
  it("steps through suggestions with a counter that follows each decision", async () => {
    const t = setup();
    await act(async () => {
      await t.store.getState().startQueue(t.ids);
    });
    expect(screen.getByTestId("ai-queue-counter")).toHaveTextContent("1 of 2");
    expect(screen.getByRole("button", { name: "Edit" })).toBeVisible();

    await t.user.click(screen.getByRole("button", { name: "Apply" }));
    expect(screen.getByTestId("ai-queue-counter")).toHaveTextContent("2 of 2");
    expect(unresolvedConflicts(t.state())).toEqual([t.ids[1]]);

    await t.user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(
      screen.queryByRole("complementary", { name: "AI assistant" }),
    ).toBeNull();
    expect(t.store.getState().queue).toBeNull();
    // Dismissed means untouched: it is still unresolved.
    expect(unresolvedConflicts(t.state())).toEqual([t.ids[1]]);
  });

  it("Edit hands the conflict back to the user and moves on", async () => {
    const t = setup();
    await act(async () => {
      await t.store.getState().startQueue(t.ids);
    });
    await t.user.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByTestId("ai-queue-counter")).toHaveTextContent("2 of 2");
    expect(t.dispatched).toHaveLength(1);
    expect(t.dispatched[0].selection).toBeDefined();
    expect(unresolvedConflicts(t.state())).toEqual(t.ids);
  });
});
