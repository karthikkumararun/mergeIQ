import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  commands,
  events,
  type AcceptSide,
  type Analysis,
  type BatchResult,
  type ConflictDetails,
  type ConflictLoad,
  type ControlOutcome,
  type EncodingInfo,
  type GoSumMerge,
  type IpcError,
  type LockfileCommand,
  type LockfileFinished,
  type ModifyDeleteView,
  type OutputStream,
  type PathToken,
  type RegenerateResult,
  type RenameOutcome,
  type RepoInfo,
  type RepoStatus,
  type StageBlob,
  type SubmoduleDetails,
  type WhitespacePolicy,
  type WorkingText,
} from "../ipc/bindings";
import { describeError } from "../merge-editor/hosts";

export type {
  AcceptSide,
  BatchResult,
  ControlOutcome,
  LockfileCommand,
  OutputStream,
  RegenerateResult,
  RepoInfo,
  RepoStatus,
  WorkingText,
};

/** How a lockfile command run ended: a result, or why it could not run. */
export interface RegenerateOutcome {
  result: RegenerateResult | null;
  error: string | null;
}

/** A started "take a side and regenerate" run. */
export interface LockfileRun {
  /** Resolves when the command has ended. */
  done: Promise<RegenerateOutcome>;
  /** Stops the command; the lockfile stays unstaged. */
  cancel(): void;
}

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

  // --- special conflicts ---
  /** Per-stage facts, renames and rename pairs for a conflicted path. */
  details(path: PathToken): Promise<ConflictDetails>;
  /** A stage's bytes for an image preview (up to 20 MB). */
  stageBlob(path: PathToken, stage: number): Promise<StageBlob>;
  /** The surviving side of a modify/delete conflict diffed against the base. */
  modifyDeleteView(path: PathToken): Promise<ModifyDeleteView>;
  /** Takes one side byte for byte and stages it (removes the path if it deleted it). */
  useSide(path: PathToken, side: AcceptSide): Promise<void>;
  /** Writes the surviving side unstaged and returns its text for editing. */
  keepAndEdit(path: PathToken, side: AcceptSide): Promise<WorkingText>;
  workingText(path: PathToken): Promise<WorkingText>;
  submoduleDetails(path: PathToken): Promise<SubmoduleDetails>;
  /** Chooses the final path of a rename/rename conflict. */
  renameChoose(path: PathToken): Promise<RenameOutcome>;
  goSumPreview(path: PathToken): Promise<GoSumMerge>;
  /** Writes the go.sum union merge and stages it. */
  goSumUnion(path: PathToken): Promise<void>;
  lockfileCommands(): Promise<LockfileCommand[]>;
  /**
   * Takes `side`'s lockfile and runs the (user-confirmed) `command`, streaming output lines.
   * Rejects if the command cannot be started.
   */
  lockfileRegenerate(
    path: PathToken,
    side: AcceptSide,
    command: string,
    onOutput: (stream: OutputStream, line: string) => void,
  ): Promise<LockfileRun>;
  openWorkingFile(path: PathToken): Promise<void>;
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
    details: (path) => unwrap(commands.conflictDetails(repo, path)),
    stageBlob: (path, stage) =>
      unwrap(commands.conflictStageBlob(repo, path, stage)),
    modifyDeleteView: (path) =>
      unwrap(commands.conflictModifyDeleteView(repo, path)),
    useSide: async (path, side) => {
      await unwrap(commands.conflictUseSide(repo, path, side));
    },
    keepAndEdit: (path, side) =>
      unwrap(commands.conflictKeepAndEdit(repo, path, side)),
    workingText: (path) => unwrap(commands.conflictWorkingText(repo, path)),
    submoduleDetails: (path) => unwrap(commands.submoduleDetails(repo, path)),
    renameChoose: (path) => unwrap(commands.renameChoose(repo, path)),
    goSumPreview: (path) => unwrap(commands.goSumPreview(repo, path)),
    goSumUnion: async (path) => {
      await unwrap(commands.goSumUnion(repo, path));
    },
    lockfileCommands: () => commands.getLockfileCommands(),
    lockfileRegenerate: async (path, side, command, onOutput) => {
      const window = getCurrentWindow();
      let run: number | null = null;
      // Events can arrive before the command returns the run id: keep them until it does.
      const early: { run: number; stream: OutputStream; line: string }[] = [];
      let earlyEnd: LockfileFinished | null = null;
      let finish!: (outcome: RegenerateOutcome) => void;
      const done = new Promise<RegenerateOutcome>(
        (resolve) => (finish = resolve),
      );
      const end = (e: LockfileFinished) =>
        finish({ result: e.result, error: e.error });
      const stopOutput = await events.lockfileOutput(window).listen((e) => {
        if (run === null) early.push(e.payload);
        else if (e.payload.run === run)
          onOutput(e.payload.stream, e.payload.line);
      });
      const stopFinished = await events.lockfileFinished(window).listen((e) => {
        if (run === null) earlyEnd = e.payload;
        else if (e.payload.run === run) end(e.payload);
      });
      const stop = () => {
        stopOutput();
        stopFinished();
      };
      try {
        run = await unwrap(
          commands.lockfileRegenerate(repo, path, side, command),
        );
      } catch (e) {
        stop();
        throw e;
      }
      const id = run;
      for (const e of early) if (e.run === id) onOutput(e.stream, e.line);
      if (earlyEnd && (earlyEnd as LockfileFinished).run === id)
        end(earlyEnd as LockfileFinished);
      return {
        done: done.finally(stop),
        cancel: () => void commands.lockfileCancel(id),
      };
    },
    openWorkingFile: async (path) => {
      await unwrap(commands.openWorkingFile(repo, path));
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
