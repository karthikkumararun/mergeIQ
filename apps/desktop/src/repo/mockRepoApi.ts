import type {
  Analysis,
  ConflictClass,
  ConflictDetails,
  ConflictEntry,
  ConflictLoad,
  ConflictType,
  LockfileCommand,
  Operation,
  SideLabel,
  StageMeta,
} from "../ipc/bindings";
import { mockMergeDocument } from "../ipc/mock";
import {
  LOCKFILE_FAIL_LINES,
  LOCKFILE_OK_LINES,
  specialConflicts,
  stagesOf,
  type MockSpecial,
} from "./mockSpecial";
import type {
  LockfileRun,
  RegenerateOutcome,
  RepoApi,
  RepoStatus,
} from "./repoApi";
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
  "merge3" | "rebase2" | "modifydelete" | "clean" | "many" | "special";

interface MockConflict {
  display: string;
  type: ConflictType;
  /** Fixture that supplies the merge analysis; `null` for non-text files. */
  fixture: string | null;
  /** Conflict class (default text). */
  cls?: ConflictClass;
  /** Data for the special-conflict panels. */
  special?: MockSpecial;
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
    case "special":
      return [
        {
          operation: { kind: "Merge" },
          subject: "Promo code stacking",
          conflicts: specialConflicts(),
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
  /**
   * How a mocked lockfile command ends: `ok` (exit 0, staged), `fail` (exit 1), `hang`
   * (runs until cancelled) or `missing` (the program is not installed).
   */
  lockfile?: "ok" | "fail" | "hang" | "missing";
}

/** A scripted in-memory repository for UI tests (dev route and Vitest). */
export function createMockRepoApi(options: MockRepoOptions = {}): RepoApi {
  const script = steps(options.scenario ?? "merge3");
  let step = 0;
  const stageMetas = (c: MockConflict): StageMeta[] =>
    stagesOf(c.type).map((n) => {
      const over = c.special?.stages?.[n] ?? {};
      const isLink = c.cls?.class === "Symlink";
      const isGit = c.cls?.class === "Submodule";
      return {
        stage: n,
        mode: isGit ? "160000" : isLink ? "120000" : "100644",
        oid: `${"bot"[n - 1]}-${c.display}`,
        size: isGit ? null : 1024,
        symlinkTarget: null,
        lfs: null,
        ...over,
      };
    });
  const entry = (c: MockConflict): ConflictEntry => {
    const metas = stageMetas(c);
    return {
      path: `mock:${c.display}`,
      display: c.display,
      conflictType: c.type,
      stages: metas.map((m) => ({ stage: m.stage, oid: m.oid, mode: m.mode })),
      hasSymlink: c.cls?.class === "Symlink",
      hasGitlink: c.cls?.class === "Submodule",
      class: c.cls ?? { class: "Text" },
    };
  };
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
    details: (path) => {
      const c = requireConflict(path);
      const base = mockMergeDocument(c.fixture ?? "simple-conflict");
      const details: ConflictDetails = {
        entry: entry(c),
        labels: status().labels,
        context: base.context ?? { ours: [], theirs: [] },
        stages: stageMetas(c),
        renames: c.special?.rename?.renames ?? [],
        renamePair: c.special?.rename?.pair ?? null,
      };
      return Promise.resolve(details);
    },
    stageBlob: (path, stage) => {
      const c = requireConflict(path);
      const blob = c.special?.images?.[stage as 1 | 2 | 3];
      return blob
        ? Promise.resolve(blob)
        : Promise.reject(new RepoError("that side has no file content"));
    },
    modifyDeleteView: (path) => {
      const c = requireConflict(path);
      if (c.type !== "DeletedByUs" && c.type !== "DeletedByThem")
        return Promise.reject(
          new RepoError("this is not a modify/delete conflict"),
        );
      return Promise.resolve(
        c.special?.modifyDelete ?? {
          deletedBy:
            c.type === "DeletedByUs" ? ("Ours" as const) : ("Theirs" as const),
          baseText: "one\ntwo\nthree\n",
          survivorText: "one\nTWO\nthree\n",
          hunks: [
            { before: { start: 1, end: 2 }, after: { start: 1, end: 2 } },
          ],
          note: null,
        },
      );
    },
    useSide: (path, side) => {
      record(`useSide ${side} ${path}`);
      resolve(path);
      return Promise.resolve();
    },
    keepAndEdit: (path, side) => {
      record(`keepAndEdit ${side} ${path}`);
      const c = requireConflict(path);
      return Promise.resolve({
        text: c.special?.workingText ?? "",
        encoding: { encoding: "Utf8", bom: false },
      });
    },
    workingText: (path) => {
      const c = requireConflict(path);
      return Promise.resolve({
        text: c.special?.workingText ?? "",
        encoding: { encoding: "Utf8", bom: false },
      });
    },
    submoduleDetails: (path) => {
      const d = requireConflict(path).special?.submodule;
      return d
        ? Promise.resolve(d)
        : Promise.reject(new RepoError("this is not a submodule conflict"));
    },
    renameChoose: (path) => {
      record(`renameChoose ${path}`);
      const c = [...open.values()].find((x) => x.special?.rename);
      const pair = c?.special?.rename?.pair;
      if (!c || !pair)
        return Promise.reject(new RepoError("no rename conflict"));
      for (const display of [pair.from, pair.ours.to, pair.theirs.to])
        open.delete(`mock:${display}`);
      if (pair.contentsDiffer) {
        const merged: MockConflict = {
          display: path.replace(/^mock:/, ""),
          type: "BothModified",
          fixture: "mixed-changes",
        };
        open.set(path, merged);
        all.set(path, merged);
      }
      notify();
      return Promise.resolve({ chosen: path, needsMerge: pair.contentsDiffer });
    },
    goSumPreview: (path) => {
      const g = requireConflict(path).special?.goSum;
      return g
        ? Promise.resolve(g)
        : Promise.reject(new RepoError("this is not a go.sum conflict"));
    },
    goSumUnion: (path) => {
      record(`goSumUnion ${path}`);
      resolve(path);
      return Promise.resolve();
    },
    lockfileCommands: () => {
      const defaults: [LockfileCommand["kind"], string][] = [
        ["Npm", "npm install --package-lock-only"],
        ["Pnpm", "pnpm install --lockfile-only"],
        ["Yarn", "yarn install --mode update-lockfile"],
        ["Poetry", "poetry lock --no-update"],
        ["Cargo", "cargo update --workspace"],
        ["Gradle", "./gradlew dependencies --write-locks"],
      ];
      return Promise.resolve(
        defaults.map(([kind, command]) => ({
          kind,
          command,
          defaultCommand: command,
          custom: false,
        })),
      );
    },
    lockfileRegenerate: (path, side, command, onOutput) => {
      record(`lockfile ${side} ${path} ${command}`);
      requireConflict(path);
      const mode = options.lockfile ?? "ok";
      if (mode === "missing")
        return Promise.reject(
          new RepoError(`\`${command.split(" ")[0]}\` was not found`),
        );
      let cancelled = false;
      let wake: (() => void) | null = null;
      const pause = (ms: number) =>
        new Promise<void>((r) => {
          const t = setTimeout(r, ms);
          wake = () => {
            clearTimeout(t);
            r();
          };
        });
      const done = (async (): Promise<RegenerateOutcome> => {
        const lines = mode === "fail" ? LOCKFILE_FAIL_LINES : LOCKFILE_OK_LINES;
        for (const line of lines) {
          await pause(15);
          if (cancelled) break;
          onOutput(mode === "fail" ? "Stderr" : "Stdout", line);
        }
        if (mode === "hang" && !cancelled)
          await new Promise<void>((r) => (wake = r));
        const ok = mode === "ok" && !cancelled;
        // Like the real backend, our own staging does not notify the window: the panel
        // refreshes after it has recorded the resolution.
        if (ok) open.delete(path);
        return {
          error: null,
          result: {
            staged: ok,
            run: {
              exitCode: cancelled ? null : ok ? 0 : 1,
              cancelled,
              durationMs: cancelled ? 900 : 4200,
            },
          },
        };
      })();
      const run: LockfileRun = {
        done,
        cancel: () => {
          cancelled = true;
          wake?.();
        },
      };
      return Promise.resolve(run);
    },
    openWorkingFile: (path) => {
      record(`open ${path}`);
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
