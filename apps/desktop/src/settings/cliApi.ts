import {
  commands,
  type CliSetupInfo,
  type InstallOutcome,
  type IpcError,
} from "../ipc/bindings";
import { describeError } from "../merge-editor/hosts";

/** The backend calls behind Settings › Command line, so tests can swap them. */
export interface CliSetupApi {
  info(): Promise<CliSetupInfo>;
  install(dir: string, admin: boolean): Promise<InstallOutcome>;
  addToPath(dir: string): Promise<void>;
  /** The `git config --global` lines; the last is `keepBackup` when `noBackup`. */
  commands(noBackup: boolean): Promise<string[]>;
  configure(noBackup: boolean): Promise<void>;
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

export const ipcCliApi: CliSetupApi = {
  info: () => unwrap(commands.cliSetupInfo()),
  install: (dir, admin) => unwrap(commands.cliInstall(dir, admin)),
  addToPath: async (dir) => {
    await unwrap(commands.cliAddToPath(dir));
  },
  commands: (noBackup) => commands.gitMergetoolCommands(noBackup),
  configure: async (noBackup) => {
    await unwrap(commands.gitMergetoolConfigure(noBackup));
  },
};
