import { describe, expect, it, vi } from "vitest";
import { createMockAiApi, type MockAiOptions } from "../../../ai/mockApi";
import type { ContextInput } from "../../../ai/api";
import { chunkAi, createAiStore, CONCURRENCY, type AiStore } from "./store";

function input(path = "src/cart.ts", id = 0): ContextInput {
  const span = { start: id, end: id + 1 };
  return {
    path,
    base: "base\n",
    left: { label: "main", text: `L${id}\n`.padStart(3, "x"), commits: [] },
    right: { label: "feature", text: `R${id}\n`.padStart(3, "x"), commits: [] },
    result: "base\n",
    chunk: {
      base: span,
      left: { start: 0, end: 1 },
      right: { start: 0, end: 1 },
      result: span,
    },
  };
}

function setup(options: MockAiOptions = {}, path = "src/cart.ts") {
  const api = createMockAiApi(options);
  const store = createAiStore({
    api,
    repo: null,
    scope: "standalone",
    scopeName: "this merge",
  });
  store.getState().bind({
    path,
    inputFor: (id) => input(path, id),
    spansFor: (id) => input(path, id).chunk,
  });
  return { api, store };
}

async function until(_store: AiStore, ok: () => boolean) {
  await vi.waitFor(() => expect(ok()).toBe(true), { timeout: 2000 });
}

describe("explain", () => {
  it("streams text incrementally and records usage", async () => {
    const { store } = setup({ delayMs: 1 });
    const seen: [string, number][] = [];
    store.subscribe((s) => {
      const e = chunkAi(s, 1).explain;
      seen.push([e.phase, e.text.length]);
    });
    await store.getState().explain(1);
    // Text grew while the request was still streaming, then it finished.
    expect(seen.some(([phase, n]) => phase === "streaming" && n > 0)).toBe(
      true,
    );
    const lengths = seen.filter(([p]) => p === "streaming").map(([, n]) => n);
    expect(lengths).toEqual([...lengths].sort((a, b) => a - b));
    const e = chunkAi(store.getState(), 1).explain;
    expect(e.phase).toBe("done");
    expect(e.text).toMatch(/^main changed this region/);
    expect(store.getState().session?.requests).toBe(1);
  });

  it("cancel stops the request and keeps what arrived", async () => {
    const { api, store } = setup({ delayMs: 15 });
    const run = store.getState().explain(1);
    await until(
      store,
      () => chunkAi(store.getState(), 1).explain.text.length > 0,
    );
    store.getState().cancel(1, "explain");
    expect(chunkAi(store.getState(), 1).explain.phase).toBe("cancelled");
    const kept = chunkAi(store.getState(), 1).explain.text;
    await run;
    expect(api.calls.cancel).toHaveLength(1);
    // Late words and results are ignored.
    expect(chunkAi(store.getState(), 1).explain.text).toBe(kept);
    expect(chunkAi(store.getState(), 1).explain.phase).toBe("cancelled");
    expect(store.getState().session).toBeNull();
  });

  it("shows provider failures in the chunk, not as a gate", async () => {
    const { store } = setup({ fail: { code: "network", message: "offline" } });
    await store.getState().explain(1);
    const e = chunkAi(store.getState(), 1).explain;
    expect(e.phase).toBe("error");
    expect(e.failure).toEqual({ code: "network", message: "offline" });
    expect(store.getState().gate).toBeNull();
  });
});

describe("suggest", () => {
  it("returns a suggestion, then regenerate replaces it and dismiss clears it", async () => {
    const { api, store } = setup();
    await store.getState().suggest(2);
    const first = chunkAi(store.getState(), 2).suggest;
    expect(first.phase).toBe("ready");
    expect(first.result?.checked.suggestion.strategy).toBe("both");
    await store.getState().suggest(2);
    expect(api.calls.suggest).toBe(2);
    expect(
      chunkAi(store.getState(), 2).suggest.result?.record.usage.cacheReadTokens,
    ).toBe(4800);
    store.getState().dismiss(2);
    expect(chunkAi(store.getState(), 2).suggest.phase).toBe("idle");
  });

  it("a refusal is an error state with its failure", async () => {
    const { store } = setup({
      fail: { code: "refused", category: "cyber", explanation: null },
    });
    expect(await store.getState().suggest(1)).toBe(false);
    const s = chunkAi(store.getState(), 1).suggest;
    expect(s.phase).toBe("error");
    expect(s.result).toBeNull();
    expect(s.failure).toEqual({
      code: "refused",
      category: "cyber",
      explanation: null,
    });
  });
});

describe("gates", () => {
  it("no provider: nothing is sent and a gate is shown", async () => {
    const { api, store } = setup({ configured: false });
    await store.getState().suggest(1);
    expect(store.getState().gate?.failure.code).toBe("notConfigured");
    expect(store.getState().status?.blocked?.code).toBe("notConfigured");
    expect(chunkAi(store.getState(), 1).suggest.phase).toBe("idle");
    expect(api.calls.suggest).toBe(1); // attempted, refused by the backend gate
    expect(api.calls.inputs).toHaveLength(1);
  });

  it("the notice must be accepted, then the action resumes by itself", async () => {
    const { api, store } = setup({ notice: false });
    await store.getState().suggest(1);
    expect(store.getState().gate?.failure.code).toBe("noticeRequired");
    await store.getState().acceptNotice();
    await until(
      store,
      () => chunkAi(store.getState(), 1).suggest.phase === "ready",
    );
    expect(store.getState().gate).toBeNull();
    expect(api.settings.noticeAccepted).toBe(true);
  });

  it("repo opt-in: allowing resumes, declining sends nothing", async () => {
    const a = setup({ repo: "unasked" });
    await a.store.getState().explain(1);
    expect(a.store.getState().gate?.failure.code).toBe("repoUnasked");
    await a.store.getState().decideRepo("Allowed");
    await until(
      a.store,
      () => chunkAi(a.store.getState(), 1).explain.phase === "done",
    );
    expect(a.api.settings.privacy?.repos?.standalone).toBe("Allowed");

    const b = setup({ repo: "unasked" });
    await b.store.getState().suggest(1);
    await b.store.getState().decideRepo("Declined");
    expect(b.store.getState().gate?.failure.code).toBe("repoDeclined");
    expect(b.api.calls.suggest).toBe(1); // only the refused first attempt
    expect(chunkAi(b.store.getState(), 1).suggest.phase).toBe("idle");
    expect(b.api.settings.privacy?.repos?.standalone).toBe("Declined");
  });

  it("excluded files are refused with the settings message", async () => {
    const { store } = setup({}, "config/.env.production");
    await store.getState().suggest(1);
    const gate = store.getState().gate;
    expect(gate?.failure.code).toBe("excluded");
    expect(store.getState().status?.blocked?.code).toBe("excluded");
  });
});

describe("queue", () => {
  it("runs at most three at a time and steps through decisions in order", async () => {
    const { api, store } = setup({ delayMs: 20 });
    let running = 0;
    let peak = 0;
    const original = api.suggest;
    api.suggest = async (...args) => {
      running += 1;
      peak = Math.max(peak, running);
      try {
        return await original(...args);
      } finally {
        running -= 1;
      }
    };
    const ids = [4, 7, 9, 12];
    const run = store.getState().startQueue(ids);
    expect(store.getState().queue?.ids).toEqual(ids);
    expect(store.getState().panel).toMatchObject({
      open: true,
      chunkId: 4,
      tab: "suggestion",
    });
    await run;
    expect(peak).toBe(CONCURRENCY);
    for (const id of ids)
      expect(chunkAi(store.getState(), id).suggest.phase).toBe("ready");

    store.getState().decide(4, "applied");
    expect(store.getState().queue?.review).toBe(1);
    expect(store.getState().panel.chunkId).toBe(7);
    store.getState().decide(7, "dismissed");
    store.getState().decide(9, "applied");
    expect(store.getState().queue?.review).toBe(3);
    store.getState().decide(12, "skipped");
    expect(store.getState().queue).toBeNull();
    expect(store.getState().panel.open).toBe(false);
  });

  it("a gate stops the whole run", async () => {
    const { api, store } = setup({ notice: false });
    await store.getState().startQueue([1, 2, 3, 4, 5]);
    expect(store.getState().queue).toBeNull();
    expect(store.getState().gate?.failure.code).toBe("noticeRequired");
    expect(api.calls.suggest).toBeLessThanOrEqual(CONCURRENCY);
  });

  it("cancelling the queue cancels what is in flight", async () => {
    const { api, store } = setup({ delayMs: 30 });
    const run = store.getState().startQueue([1, 2, 3, 4, 5, 6]);
    await until(store, () => api.calls.suggest >= 1);
    store.getState().cancelQueue();
    await run;
    expect(store.getState().queue).toBeNull();
    expect(api.calls.cancel.length).toBeGreaterThan(0);
  });

  it("selecting another queued conflict moves the review position", async () => {
    const { store } = setup();
    await store.getState().startQueue([3, 5, 8]);
    store.getState().select(8);
    expect(store.getState().queue?.review).toBe(2);
    expect(store.getState().panel.chunkId).toBe(8);
  });
});

describe("preview and estimate", () => {
  it("loads the exact request text", async () => {
    const { store } = setup();
    await store.getState().openPreview(1, "suggest");
    const p = store.getState().preview;
    expect(p?.phase).toBe("ready");
    expect(p?.data?.text).toContain("<file_context>");
    expect(p?.data?.destination).toBe("api.anthropic.com");
    store.getState().closePreview();
    expect(store.getState().preview).toBeNull();
  });

  it("the preview of an excluded file is refused", async () => {
    const { store } = setup({}, "certs/server.pem");
    await store.getState().openPreview(1, "suggest");
    expect(store.getState().preview?.phase).toBe("error");
    expect(store.getState().preview?.failure?.code).toBe("excluded");
  });

  it("estimates the whole run", async () => {
    const { store } = setup();
    await store.getState().refreshEstimate([1, 2, 3]);
    expect(store.getState().estimate?.totalTokens).toBe(18000);
    await store.getState().refreshEstimate([]);
    expect(store.getState().estimate).toBeNull();
  });
});
