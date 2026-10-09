import type { CliSetupApi } from "./cliApi";

declare global {
  interface Window {
    /** Calls captured by the mock API, for Playwright assertions. */
    __mergeiqCli?: {
      installs: { dir: string; admin: boolean }[];
      configured: boolean[];
      addedToPath: string[];
    };
  }
}

export interface MockCliOptions {
  platform?: string;
  /** Whether installed folders count as being on PATH. */
  onPath?: boolean;
  installed?: boolean;
  configured?: boolean;
}

const LINES = [
  "git config --global merge.tool mergeiq",
  `git config --global mergetool.mergeiq.cmd 'mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED"'`,
  "git config --global mergetool.mergeiq.trustExitCode true",
  "git config --global mergetool.keepBackup false",
];

/** In-memory stand-in for the backend (dev route and unit tests). */
export function createMockCliApi(options: MockCliOptions = {}): CliSetupApi {
  const platform = options.platform ?? "macos";
  const state = {
    installed: options.installed ?? false,
    configured: options.configured ?? false,
  };
  const log = (window.__mergeiqCli ??= {
    installs: [],
    configured: [],
    addedToPath: [],
  });
  const dirs =
    platform === "windows"
      ? [
          {
            path: "C:\\Users\\me\\AppData\\Local\\MergeIQ",
            label: "C:\\Users\\me\\AppData\\Local\\MergeIQ",
            needsAdmin: false,
          },
        ]
      : [
          {
            path: "/Users/me/.local/bin",
            label: "~/.local/bin",
            needsAdmin: false,
          },
          {
            path: "/usr/local/bin",
            label: "/usr/local/bin (asks for admin password)",
            needsAdmin: true,
          },
        ];
  return {
    info: () =>
      Promise.resolve({
        platform,
        dirs,
        installedAt: state.installed ? `${dirs[0].path}/mergeiq` : null,
        mergetoolConfigured: state.configured,
      }),
    install: (dir, admin) => {
      log.installs.push({ dir, admin });
      state.installed = true;
      const onPath = options.onPath ?? true;
      return Promise.resolve({
        linkPath: `${dir}/mergeiq`,
        onPath,
        pathHint:
          onPath || platform === "windows"
            ? null
            : {
                rcFile: "~/.zshrc",
                line: 'export PATH="$HOME/.local/bin:$PATH"',
              },
      });
    },
    addToPath: (dir) => {
      log.addedToPath.push(dir);
      return Promise.resolve();
    },
    commands: (noBackup) =>
      Promise.resolve(noBackup ? LINES : LINES.slice(0, 3)),
    configure: (noBackup) => {
      log.configured.push(noBackup);
      state.configured = true;
      return Promise.resolve();
    },
  };
}
