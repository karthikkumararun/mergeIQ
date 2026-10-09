import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  commands,
  events,
  type AcceptSide,
  type Analysis,
  type BatchResult,
  type ConflictLoad,
  type ControlOutcome,
  type EncodingInfo,
  type IpcError,
  type PathToken,
  type RepoInfo,
  type RepoStatus,
  type WhitespacePolicy,
} from "../ipc/bindings";
import { describeError } from "../merge-editor/hosts";

export type { AcceptSide, BatchResult, ControlOutcome, RepoInfo, RepoStatus };

/** Error from a git operation, text kept verbatim for the "Git output" panel. */
export class RepoError extends Error {}

/** One repository window's backend, so the UI can run against a mock. */
export interface RepoApi {
  info(): Promise<RepoInfo>;
  status(): Promise<RepoStatus>;
  loadConflict(path: PathToken): Promise<ConflictLoad>;
  analyze(path: PathToken, whitespace: WhitespacePolicy): Promise<Analysis>;
  /** Writes the result; with `stage` also stages it (resolved). */
  save(
    path: PathToken,
    text: string,
    encoding: EncodingInfo,
    stage: boolean,
  ): Promise<void>;
  acceptSide(path: PathToken, side: AcceptSide): Promise<void>;
  acceptMany(paths: PathToken[], side: AcceptSide): Promise<BatchResult>;
  deleteFile(path: PathToken): Promise<void>;
  restore(path: PathToken): Promise<void>;
  continueOperation(): Promise<ControlOutcome>;
  abortOperation(): Promise<ControlOutcome>;
  skipOperation(): Promise<ControlOutcome>;
  /** Subscribes to `repo-changed`; returns the unsubscribe function. */
  onChanged(handler: () => void): Promise<() => void>;
  /**
   * Intercepts closing this window. `handler` returns `true` to let it close; otherwise
   * the window stays open. Returns the unsubscribe function.
   */
  onCloseRequested(handler: () => boolean): Promise<() => void>;
  /** Closes this window without asking again. */
  closeWindow(): Promise<void>;
}

async function unwrap<T>(
  call: Promise<
    { status: "ok"; data: T } | { status: "error"; error: IpcError }
  >,
): Promise<T> {
  const result = await call;
  if (result.status === "error") {
    const { error } = result;
    const text =
      error.kind === "Git" && error.message.kind === "CommandFailed"
        ? error.message.stderr
        : describeError(error);
    throw new RepoError(text);
  }
  return result.data;
}

/** The real backend for the window showing repository `repo`. */
export function ipcRepoApi(repo: number): RepoApi {
  return {
    info: () => unwrap(commands.repoInfo(repo)),
    status: () => unwrap(commands.repoStatus(repo)),
    loadConflict: (path) => unwrap(commands.conflictLoad(repo, path)),
    analyze: (path, ws) => unwrap(commands.conflictAnalyze(repo, path, ws)),
    save: async (path, text, encoding, stage) => {
      await unwrap(commands.conflictSave(repo, path, text, encoding, stage));
    },
    acceptSide: async (path, side) => {
      await unwrap(commands.conflictAcceptSide(repo, path, side));
    },
    acceptMany: (paths, side) =>
      unwrap(commands.conflictAcceptMany(repo, paths, side)),
    deleteFile: async (path) => {
      await unwrap(commands.conflictDelete(repo, path));
    },
    restore: async (path) => {
      await unwrap(commands.conflictRestore(repo, path));
    },
    continueOperation: () => unwrap(commands.opContinue(repo)),
    abortOperation: () => unwrap(commands.opAbort(repo)),
    skipOperation: () => unwrap(commands.opSkip(repo)),
    onChanged: async (handler) => {
      const window = getCurrentWindow();
      return events.repoChanged(window).listen(() => handler());
    },
    closeWindow: () => getCurrentWindow().destroy(),
    onCloseRequested: async (handler) => {
      const window = getCurrentWindow();
      return window.onCloseRequested((event) => {
        if (!handler()) event.preventDefault();
      });
    },
  };
}
