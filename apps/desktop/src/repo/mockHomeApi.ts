import type { HomeApi, RecentRepoDto } from "./homeApi";
import { NotARepoError } from "./homeApi";

declare global {
  interface Window {
    /** Calls captured by the mock home API, for Playwright assertions. */
    __mergeiqHome?: { opened: string[]; removed: string[] };
  }
}

export interface MockHomeOptions {
  recents?: RecentRepoDto[];
  /** Paths that are repositories; anything else is "Not a git repository". */
  repos?: string[];
  /** What the folder picker returns. */
  picked?: string | null;
}

const NOW = () => Math.floor(Date.now() / 1000);

export function sampleRecents(): RecentRepoDto[] {
  const now = NOW();
  return [
    {
      name: "shop-web",
      path: "/code/shop-web",
      openedAt: now - 120,
      exists: true,
    },
    {
      name: "payments-service",
      path: "/code/work/payments-service",
      openedAt: now - 90_000,
      exists: true,
    },
    {
      name: "old-api",
      path: "/code/old-api",
      openedAt: now - 86_400 * 60,
      exists: false,
    },
  ];
}

/** In-memory stand-in for the backend (dev route and unit tests). */
export function createMockHomeApi(options: MockHomeOptions = {}): HomeApi {
  let recents = options.recents ?? sampleRecents();
  const repos =
    options.repos ?? recents.filter((r) => r.exists).map((r) => r.path);
  const log = (window.__mergeiqHome ??= { opened: [], removed: [] });
  const dropHandlers = new Set<(paths: string[]) => void>();
  const onWindowDrop = (event: Event) => {
    const paths = (event as CustomEvent<string[]>).detail;
    dropHandlers.forEach((h) => h(paths));
  };
  window.addEventListener("mergeiq:drop", onWindowDrop);
  return {
    pickFolder: () => Promise.resolve(options.picked ?? null),
    openRepo: (path) => {
      if (!repos.includes(path)) return Promise.reject(new NotARepoError(path));
      log.opened.push(path);
      const name = path.split("/").pop() ?? path;
      recents = [
        { name, path, openedAt: NOW(), exists: true },
        ...recents.filter((r) => r.path !== path),
      ].slice(0, 15);
      return Promise.resolve();
    },
    recents: () => Promise.resolve(recents),
    removeRecent: (path) => {
      log.removed.push(path);
      recents = recents.filter((r) => r.path !== path);
      return Promise.resolve(recents);
    },
    onDrop: (handler) => {
      dropHandlers.add(handler);
      return Promise.resolve(() => {
        dropHandlers.delete(handler);
      });
    },
  };
}
