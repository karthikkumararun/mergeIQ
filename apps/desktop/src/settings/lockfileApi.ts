import {
  commands,
  type IpcError,
  type LockfileCommand,
  type LockfileKind,
} from "../ipc/bindings";
import { describeError } from "../merge-editor/hosts";

export type { LockfileCommand, LockfileKind };

/** The backend calls behind Settings › Lockfiles, so tests can swap them. */
export interface LockfileSettingsApi {
  list(): Promise<LockfileCommand[]>;
  /** Sets the command for `kind`; `null` resets it to the default. */
  set(kind: LockfileKind, command: string | null): Promise<LockfileCommand[]>;
}

async function unwrap<T>(
  call: Promise<
    { status: "ok"; data: T } | { status: "error"; error: IpcError }
  >,
): Promise<T> {
  const result = await call;
  if (result.status === "error") throw new Error(describeError(result.error));
  return result.data;
}

export const ipcLockfileApi: LockfileSettingsApi = {
  list: () => commands.getLockfileCommands(),
  set: (kind, command) => unwrap(commands.setLockfileCommand(kind, command)),
};

/** An in-memory implementation for tests and the dev routes. */
export function createMockLockfileApi(
  initial: Partial<Record<LockfileKind, string>> = {},
): LockfileSettingsApi {
  const defaults: [LockfileKind, string][] = [
    ["Npm", "npm install --package-lock-only"],
    ["Pnpm", "pnpm install --lockfile-only"],
    ["Yarn", "yarn install --mode update-lockfile"],
    ["Poetry", "poetry lock --no-update"],
    ["Cargo", "cargo update --workspace"],
    ["Gradle", "./gradlew dependencies --write-locks"],
  ];
  const custom = new Map<LockfileKind, string>(
    Object.entries(initial) as [LockfileKind, string][],
  );
  const view = (): LockfileCommand[] =>
    defaults.map(([kind, defaultCommand]) => ({
      kind,
      defaultCommand,
      command: custom.get(kind) ?? defaultCommand,
      custom: custom.has(kind),
    }));
  return {
    list: () => Promise.resolve(view()),
    set: (kind, command) => {
      const text = command?.trim();
      if (!text) custom.delete(kind);
      else if ((text.match(/'/g)?.length ?? 0) % 2 === 1)
        return Promise.reject(
          new Error("the command is not valid: unterminated quote"),
        );
      else custom.set(kind, text);
      return Promise.resolve(view());
    },
  };
}
