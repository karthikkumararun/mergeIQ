import type {
  Analysis,
  ConflictEntry,
  ConflictLoad,
  ConflictType,
  Operation,
  SideLabel,
} from "../ipc/bindings";
import { mockMergeDocument } from "../ipc/mock";
import type { RepoApi, RepoStatus } from "./repoApi";
import { RepoError } from "./repoApi";

declare global {
  interface Window {
    /** Test hooks for the mock repository (Playwright). */
    __mergeiqRepo?: {
      /** Simulates `git checkout --theirs <file> && git add <file>` in a terminal. */
      externalResolve(display: string): void;
      /** Simulates the window's close button; resolves to whether it would close. */
      requestClose(): boolean;
      calls: string[];
      saves: { path: string; text: string; stage: boolean }[];
    };
  }
}

export type ScenarioName =
  "merge3" | "rebase2" | "modifydelete" | "clean" | "many";

interface MockConflict {
  display: string;
  type: ConflictType;
  /** Fixture that supplies the merge analysis; `null` for non-text files. */
  fixture: string | null;
}

interface MockStep {
  operation: Operation;
  subject: string;
  conflicts: MockConflict[];
}

const label = (
  role: string,
  refName: string | null,
  subject: string | null,
  term: string,
): SideLabel => ({
  role,
  refName,
  shortSha: subject ? "a1b2c3d" : null,
  subject,
  author: subject ? "Ada" : null,
  gitTerm: term,
});

const text = (display: string, fixture: string): MockConflict => ({
  display,
  type: "BothModified",
  fixture,
});

function steps(name: ScenarioName): MockStep[] {
  switch (name) {
    case "merge3":
      return [
        {
          operation: { kind: "Merge" },
          subject: "Add caching layer",
          conflicts: [
            text("src/app.ts", "simple-conflict"),
            text("src/util.ts", "mixed-changes"),
            text("web/yarn.lock", "non-overlapping"),
          ],
        },
      ];
    case "rebase2":
      return [
        {
          operation: { kind: "Rebase", step: 1, total: 2, onto: "abc1234" },
          subject: "Add promo code stacking",
          conflicts: [
            text("src/cart.ts", "simple-conflict"),
            text("src/promo.ts", "mixed-changes"),
          ],
        },
        {
          operation: { kind: "Rebase", step: 2, total: 2, onto: "abc1234" },
          subject: "Tidy checkout",
          conflicts: [text("src/checkout.ts", "non-overlapping")],
        },
      ];
    case "modifydelete":
      return [
        {
          operation: { kind: "Merge" },
          subject: "Remove coupons",
          conflicts: [
            text("src/cart.ts", "simple-conflict"),
            {
              display: "src/promo/Coupon.kt",
              type: "DeletedByUs",
              fixture: null,
            },
          ],
        },
      ];
    case "many":
      return [
        {
          operation: { kind: "Merge" },
          subject: "Regenerate lockfiles",
          conflicts: Array.from({ length: 1200 }, (_, i) =>
            text(
              `packages/pkg-${String(i % 40).padStart(2, "0")}/file-${String(i).padStart(4, "0")}.json`,
              "simple-conflict",
            ),
          ),
        },
      ];
    case "clean":
      return [{ operation: { kind: "None" }, subject: "", conflicts: [] }];
  }
}

const NONE_STEP: MockStep = {
  operation: { kind: "None" },
  subject: "",
  conflicts: [],
};

export interface MockRepoOptions {
  scenario?: ScenarioName;
  /** Make `continue` fail with this git output. */
  failContinue?: string;
}

/** A scripted in-memory repository for UI tests (dev route and Vitest). */
export function createMockRepoApi(options: MockRepoOptions = {}): RepoApi {
  const script = steps(options.scenario ?? "merge3");
  let step = 0;
  const entry = (c: MockConflict): ConflictEntry => ({
    path: `mock:${c.display}`,
    display: c.display,
    conflictType: c.type,
    stages: [
      { stage: 1, oid: `b-${c.display}`, mode: "100644" },
      { stage: 2, oid: `o-${c.display}`, mode: "100644" },
      { stage: 3, oid: `t-${c.display}`, mode: "100644" },
    ],
    hasSymlink: false,
    hasGitlink: false,
  });
  // Conflicts still unresolved in the current step, keyed by path token.
  let open = new Map<string, MockConflict>();
  const reset = () => {
    open = new Map(
      (script[step]?.conflicts ?? []).map((c) => [`mock:${c.display}`, c]),
    );
  };
  reset();
  const all = new Map<string, MockConflict>(
    script.flatMap((s) => s.conflicts).map((c) => [`mock:${c.display}`, c]),
  );

  const changed = new Set<() => void>();
  const notify = () => changed.forEach((h) => h());
  const hooks = (window.__mergeiqRepo ??= {
    externalResolve: () => {},
    requestClose: () => true,
    calls: [],
    saves: [],
  });
  hooks.calls.length = 0;
  hooks.saves.length = 0;
  hooks.externalResolve = (display) => {
    open.delete(`mock:${display}`);
    notify();
  };
  const record = (call: string) => hooks.calls.push(call);

  const current = (): MockStep => script[step] ?? NONE_STEP;
  const status = (): RepoStatus => {
    const s = current();
    const rebase = s.operation.kind === "Rebase";
    return {
      root: "/code/shop-web",
      branch: rebase ? null : "main",
      operation: s.operation,
      labels: {
        ours: label(
          rebase ? "Upstream (rebasing onto main)" : "Your branch",
          "main",
          "Refactor parser",
          "ours",
        ),
        theirs: label(
          rebase ? "Your commit being replayed" : "Incoming: feature",
          "feature/checkout-v2",
          s.subject || null,
          "theirs",
        ),
      },
      conflicts: [...open.values()].map(entry),
    };
  };
  const requireConflict = (path: string) => {
    const c = open.get(path);
    if (!c)
      throw new RepoError(
        `error: path '${path}' does not have all necessary versions`,
      );
    return c;
  };
  const resolve = (path: string) => {
    requireConflict(path);
    open.delete(path);
    notify();
  };

  return {
    info: () => Promise.resolve({ name: "shop-web", path: "/code/shop-web" }),
    status: () => Promise.resolve(status()),
    loadConflict: (path) => {
      const c = requireConflict(path);
      const base = mockMergeDocument(c.fixture ?? "simple-conflict");
      const load: ConflictLoad = {
        entry: entry(c),
        labels: status().labels,
        context: base.context ?? { ours: [], theirs: [] },
        analysis: c.fixture ? base.analysis : null,
        analysisError: c.fixture
          ? null
          : "this file cannot be analysed as text",
      };
      return Promise.resolve(load);
    },
    analyze: (path) => {
      const c = requireConflict(path);
      return Promise.resolve(
        mockMergeDocument(c.fixture ?? "simple-conflict").analysis as Analysis,
      );
    },
    save: (path, textValue, _encoding, stage) => {
      record(`save ${path} stage=${stage}`);
      hooks.saves.push({ path, text: textValue, stage });
      if (stage) resolve(path);
      return Promise.resolve();
    },
    acceptSide: (path, side) => {
      record(`accept ${side} ${path}`);
      resolve(path);
      return Promise.resolve();
    },
    acceptMany: (paths, side) => {
      record(`acceptMany ${side} ${paths.length}`);
      const result = {
        done: [] as string[],
        failed: [] as { path: string; message: string }[],
      };
      for (const p of paths) {
        if (open.has(p)) {
          open.delete(p);
          result.done.push(p);
        } else {
          result.failed.push({ path: p, message: "no longer conflicted" });
        }
      }
      notify();
      return Promise.resolve(result);
    },
    deleteFile: (path) => {
      record(`delete ${path}`);
      resolve(path);
      return Promise.resolve();
    },
    restore: (path) => {
      record(`restore ${path}`);
      const original = all.get(path);
      if (original) open.set(path, original);
      notify();
      return Promise.resolve();
    },
    continueOperation: () => {
      record("continue");
      if (options.failContinue) {
        return Promise.reject(new RepoError(options.failContinue));
      }
      if (open.size > 0) {
        return Promise.reject(
          new RepoError(`${open.size} unresolved path(s) remain`),
        );
      }
      step += 1;
      reset();
      notify();
      return Promise.resolve({
        finished: step >= script.length,
        message:
          step >= script.length
            ? "Successfully completed."
            : "Stopped at the next commit.",
      });
    },
    abortOperation: () => {
      record("abort");
      step = script.length;
      reset();
      notify();
      return Promise.resolve({ finished: true, message: "Aborted." });
    },
    skipOperation: () => {
      record("skip");
      step += 1;
      reset();
      notify();
      return Promise.resolve({
        finished: step >= script.length,
        message: "Skipped.",
      });
    },
    onChanged: (handler) => {
      changed.add(handler);
      return Promise.resolve(() => {
        changed.delete(handler);
      });
    },
    closeWindow: () => {
      record("closeWindow");
      return Promise.resolve();
    },
    onCloseRequested: (handler) => {
      hooks.requestClose = handler;
      return Promise.resolve(() => {
        if (hooks.requestClose === handler) hooks.requestClose = () => true;
      });
    },
  };
}
