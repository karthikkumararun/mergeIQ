import { describe, expect, it } from "vitest";
import type { Proposal, StructuralResolve } from "../../../ipc/bindings";
import { structuralFixtures } from "../../__fixtures__/structural";
import { createResultState } from "../../model/state";
import { chunksOf } from "../../model/session";
import { ignoreChunk } from "../../model/actions";
import { applicableProposals, createStructuralStore } from "./store";

const fx = structuralFixtures["structural-package-json"];
const proposals = (fx.resolve.outcome as { Proposals: Proposal[] }).Proposals;

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe("Proposals are computed in the background", () => {
  it("runs, then holds the proposals", async () => {
    const d = deferred<StructuralResolve>();
    const store = createStructuralStore(() => d.promise);
    const done = store.getState().compute("package.json", fx.analysis);
    expect(store.getState().phase).toBe("running");
    d.resolve(fx.resolve);
    await done;
    expect(store.getState().phase).toBe("ready");
    expect(store.getState().proposals).toHaveLength(2);
    expect(store.getState().elapsedMs).toBe(12);
  });

  it("ignores a stale response after a newer request", async () => {
    const first = deferred<StructuralResolve>();
    const second = deferred<StructuralResolve>();
    const queue = [first, second];
    const store = createStructuralStore(() => queue.shift()!.promise);
    const a = store.getState().compute("a.json", fx.analysis);
    const b = store.getState().compute("a.json", fx.analysis);
    second.resolve({ outcome: { Proposals: [] }, elapsed_ms: 1 });
    await b;
    first.resolve(fx.resolve);
    await a;
    expect(store.getState().proposals).toHaveLength(0);
    expect(store.getState().elapsedMs).toBe(1);
  });

  it("Timeout shows nothing", async () => {
    const store = createStructuralStore(() =>
      Promise.resolve({ outcome: "TimedOut", elapsed_ms: 2000 }),
    );
    await store.getState().compute("package.json", fx.analysis);
    expect(store.getState().phase).toBe("timeout");
    expect(store.getState().proposals).toEqual([]);
  });

  it("Unsupported extension shows nothing", async () => {
    const store = createStructuralStore(() =>
      Promise.resolve({ outcome: "Unsupported", elapsed_ms: 0 }),
    );
    await store.getState().compute("README.md", fx.analysis);
    expect(store.getState().phase).toBe("unsupported");
  });

  it("a failing backend only means no proposals", async () => {
    const store = createStructuralStore(() =>
      Promise.reject(new Error("boom")),
    );
    await store.getState().compute("package.json", fx.analysis);
    expect(store.getState().phase).toBe("error");
    expect(store.getState().proposals).toEqual([]);
  });

  it("starting over clears dismissals and the open preview", async () => {
    const store = createStructuralStore(() => Promise.resolve(fx.resolve));
    await store.getState().compute("package.json", fx.analysis);
    store.getState().open(0);
    store.getState().dismiss(1);
    expect(store.getState().dismissed).toEqual([1]);
    await store.getState().compute("package.json", fx.analysis);
    expect(store.getState().dismissed).toEqual([]);
    expect(store.getState().openIndex).toBeNull();
  });

  it("dismissing the open proposal closes it", async () => {
    const store = createStructuralStore(() => Promise.resolve(fx.resolve));
    await store.getState().compute("package.json", fx.analysis);
    store.getState().open(0);
    store.getState().dismiss(0);
    expect(store.getState().openIndex).toBeNull();
  });
});

describe("Which proposals are still offered", () => {
  it("offers all unresolved, undismissed proposals", () => {
    const chunks = chunksOf(createResultState(fx.analysis));
    const offered = applicableProposals({ proposals, dismissed: [] }, chunks);
    expect(offered.map((o) => o.index)).toEqual([0, 1]);
    expect(
      applicableProposals({ proposals, dismissed: [0] }, chunks).map(
        (o) => o.index,
      ),
    ).toEqual([1]);
  });

  it("drops a proposal once one of its chunks is resolved", () => {
    let state = createResultState(fx.analysis);
    state = state.update(ignoreChunk(state, fx.analysis, 1)!).state;
    const offered = applicableProposals(
      { proposals, dismissed: [] },
      chunksOf(state),
    );
    expect(offered.map((o) => o.index)).toEqual([0]);
  });
});
