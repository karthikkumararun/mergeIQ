import { beforeEach, describe, expect, it } from "vitest";
import { createMockRepoApi } from "./mockRepoApi";
import {
  MAX_TABS,
  createRepoStore,
  nextUnresolved,
  sortedConflicts,
  type RepoStore,
} from "./repoStore";

const p = (display: string) => `mock:${display}`;

async function start(
  scenario: Parameters<typeof createMockRepoApi>[0] = {},
  initial = {},
): Promise<RepoStore> {
  const store = createRepoStore(createMockRepoApi(scenario), initial);
  await store.start();
  return store;
}

const paths = (store: RepoStore) =>
  (store.getState().status?.conflicts ?? []).map((c) => c.display);

describe("repoStore", () => {
  beforeEach(() => {
    window.__mergeiqRepo = undefined;
  });

  it("loads info, status and conflicts", async () => {
    const store = await start();
    const s = store.getState();
    expect(s.info?.name).toBe("shop-web");
    expect(s.status?.operation.kind).toBe("Merge");
    expect(paths(store).sort()).toEqual([
      "src/app.ts",
      "src/util.ts",
      "web/yarn.lock",
    ]);
  });

  it("refresh preserves selection and drops paths that left the list", async () => {
    const store = await start();
    store.getState();
    store.setSelection([p("src/app.ts"), p("src/util.ts")]);
    window.__mergeiqRepo!.externalResolve("src/util.ts");
    await store.refresh();
    expect(store.getState().selection).toEqual([p("src/app.ts")]);
  });

  it("opening a file twice focuses its tab", async () => {
    const store = await start();
    store.openFile(p("src/app.ts"));
    store.openFile(p("src/util.ts"));
    store.openFile(p("src/app.ts"));
    const s = store.getState();
    expect(s.tabs.map((t) => t.display)).toEqual(["src/app.ts", "src/util.ts"]);
    expect(s.activeTab).toBe(p("src/app.ts"));
  });

  it("a tab dot follows the editor's dirty state", async () => {
    const store = await start();
    store.openFile(p("src/app.ts"));
    store.setDirty(p("src/app.ts"), true);
    expect(store.getState().tabs[0].dirty).toBe(true);
    expect(store.anyDirty()).toBe(true);
    store.setDirty(p("src/app.ts"), false);
    expect(store.anyDirty()).toBe(false);
  });

  it("caps tabs at 10 and closes the least recently used clean tab", async () => {
    const store = await start({ scenario: "many" });
    const list = sortedConflicts(store.getState().status).slice(
      0,
      MAX_TABS + 1,
    );
    list.slice(0, MAX_TABS).forEach((c) => store.openFile(c.path));
    // Touch the first so the second becomes the oldest; make the oldest dirty.
    store.focusTab(list[0].path);
    store.setDirty(list[1].path, true);
    store.openFile(list[MAX_TABS].path);
    const open = store.getState().tabs.map((t) => t.path);
    expect(open).toHaveLength(MAX_TABS);
    expect(open).toContain(list[1].path); // dirty tabs are never dropped
    expect(open).not.toContain(list[2].path); // oldest clean tab went
    expect(open).toContain(list[MAX_TABS].path);
  });

  it("refuses a new tab when every tab has unsaved changes", async () => {
    const store = await start({ scenario: "many" });
    const list = sortedConflicts(store.getState().status).slice(
      0,
      MAX_TABS + 1,
    );
    list.slice(0, MAX_TABS).forEach((c) => {
      store.openFile(c.path);
      store.setDirty(c.path, true);
    });
    store.openFile(list[MAX_TABS].path);
    expect(store.getState().tabs).toHaveLength(MAX_TABS);
    expect(store.getState().error).toMatch(/unsaved changes/);
  });

  it("auto-advance opens the next unresolved file in the same tab", async () => {
    const store = await start();
    const list = sortedConflicts(store.getState().status);
    store.openFile(list[0].path);
    await store.api.save(
      list[0].path,
      "x",
      { encoding: "Utf8", bom: false },
      true,
    );
    await store.fileResolved(list[0].path, "Merged");
    const s = store.getState();
    expect(s.tabs.map((t) => t.path)).toEqual([list[1].path]);
    expect(s.activeTab).toBe(list[1].path);
    expect(s.resolved).toEqual([
      { path: list[0].path, display: list[0].display, method: "Merged" },
    ]);
  });

  it("without auto-advance the saved tab just closes", async () => {
    const store = await start({}, { autoAdvance: false });
    const list = sortedConflicts(store.getState().status);
    store.openFile(list[0].path);
    await store.api.save(
      list[0].path,
      "x",
      { encoding: "Utf8", bom: false },
      true,
    );
    await store.fileResolved(list[0].path, "Merged");
    expect(store.getState().tabs).toEqual([]);
  });

  it("auto-advance after the last file leaves no tab", async () => {
    const store = await start({ scenario: "modifydelete" });
    const [first, second] = sortedConflicts(store.getState().status);
    await store.api.acceptSide(second.path, "Ours");
    await store.refresh();
    store.openFile(first.path);
    await store.api.save(
      first.path,
      "x",
      { encoding: "Utf8", bom: false },
      true,
    );
    await store.fileResolved(first.path, "Merged");
    expect(store.getState().tabs).toEqual([]);
    expect(paths(store)).toEqual([]);
  });

  it("batch accept resolves every selected file and logs the method", async () => {
    const store = await start();
    const list = sortedConflicts(store.getState().status);
    store.setSelection(list.map((c) => c.path));
    await store.acceptSelected("Theirs");
    const s = store.getState();
    expect(paths(store)).toEqual([]);
    expect(s.selection).toEqual([]);
    expect(s.resolved.map((r) => r.method)).toEqual([
      "Accepted Right",
      "Accepted Right",
      "Accepted Right",
    ]);
  });

  it("batch accept reports files that could not be applied", async () => {
    const store = await start();
    const list = sortedConflicts(store.getState().status);
    window.__mergeiqRepo!.externalResolve(list[0].display);
    store.setSelection(list.map((c) => c.path));
    await store.acceptSelected("Ours");
    expect(store.getState().error).toContain("no longer conflicted");
    expect(store.getState().resolved).toHaveLength(2);
  });

  it("reopen restores the conflict and removes it from resolved", async () => {
    const store = await start();
    const [a] = sortedConflicts(store.getState().status);
    await store.acceptFile(a.path, "Ours");
    expect(store.getState().resolved).toHaveLength(1);
    await store.reopen(a.path);
    expect(store.getState().resolved).toEqual([]);
    expect(paths(store)).toContain(a.display);
  });

  it("a file resolved outside flags its open tab and is not logged as ours", async () => {
    const store = await start();
    store.openFile(p("src/app.ts"));
    window.__mergeiqRepo!.externalResolve("src/app.ts");
    await store.refresh();
    const s = store.getState();
    expect(s.tabs[0].notice).toBe("resolved-outside");
    expect(s.resolved).toEqual([]);
  });

  it("reload clears the notice and remounts the editor", async () => {
    const store = await start();
    store.openFile(p("src/app.ts"));
    window.__mergeiqRepo!.externalResolve("src/app.ts");
    await store.refresh();
    store.reloadTab(p("src/app.ts"));
    const tab = store.getState().tabs[0];
    expect(tab.notice).toBeNull();
    expect(tab.epoch).toBe(1);
  });

  it("closing the tab and refreshing leaves state consistent", async () => {
    const store = await start();
    store.openFile(p("src/app.ts"));
    store.closeTab(p("src/app.ts"));
    expect(store.getState().tabs).toEqual([]);
    expect(store.getState().activeTab).toBeNull();
  });

  it("continue after the last resolution finishes the merge", async () => {
    const store = await start();
    store.setSelection(
      sortedConflicts(store.getState().status).map((c) => c.path),
    );
    await store.acceptSelected("Ours");
    await store.runOperation("continue");
    const s = store.getState();
    expect(s.status?.operation.kind).toBe("None");
    expect(s.gitOutput).toBe("Successfully completed.");
    expect(s.opError).toBeNull();
  });

  it("a rebase continues to the next stop and the conflict list repopulates", async () => {
    const store = await start({ scenario: "rebase2" });
    expect(store.getState().status?.operation).toMatchObject({
      step: 1,
      total: 2,
    });
    store.setSelection(
      sortedConflicts(store.getState().status).map((c) => c.path),
    );
    await store.acceptSelected("Theirs");
    await store.runOperation("continue");
    expect(store.getState().status?.operation).toMatchObject({
      step: 2,
      total: 2,
    });
    expect(paths(store)).toEqual(["src/checkout.ts"]);
  });

  it("git failures are kept verbatim", async () => {
    const store = await start({
      failContinue: "error: could not apply 1234abc\nhint: fix it",
    });
    await store.runOperation("continue");
    expect(store.getState().opError).toBe(
      "error: could not apply 1234abc\nhint: fix it",
    );
    expect(store.getState().opBusy).toBe(false);
  });
});

describe("nextUnresolved", () => {
  const list = ["a", "b", "c"].map((d) => ({ path: d, display: d }) as never);
  it("takes the following entry, wrapping at the end", () => {
    expect(nextUnresolved(list, "a")?.path).toBe("b");
    expect(nextUnresolved(list, "c")?.path).toBe("a");
    expect(nextUnresolved([list[0]], "a")).toBeNull();
  });
});
