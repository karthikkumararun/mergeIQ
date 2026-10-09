import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { commands, type RecentRepoDto } from "../ipc/bindings";
import { describeError } from "../merge-editor/hosts";

export type { RecentRepoDto };

/** Opening a folder that is not inside a git working tree. */
export class NotARepoError extends Error {
  constructor(readonly path: string) {
    super("Not a git repository");
  }
}

/** Everything the home view needs from the app, so tests can swap it. */
export interface HomeApi {
  /** Folder picker; `null` when cancelled. */
  pickFolder(): Promise<string | null>;
  /** Opens (or focuses) the repository window. Rejects with [`NotARepoError`]. */
  openRepo(path: string): Promise<void>;
  recents(): Promise<RecentRepoDto[]>;
  removeRecent(path: string): Promise<RecentRepoDto[]>;
  /** Subscribes to folders dropped on the window; returns the unsubscribe function. */
  onDrop(handler: (paths: string[]) => void): Promise<() => void>;
}

export const ipcHomeApi: HomeApi = {
  async pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    return typeof picked === "string" ? picked : null;
  },
  async openRepo(path) {
    const result = await commands.repoOpen(path);
    if (result.status === "ok") return;
    const error = result.error;
    if (error.kind === "Git" && error.message.kind === "NotARepo") {
      throw new NotARepoError(path);
    }
    throw new Error(describeError(error));
  },
  recents: () => commands.recentsList(),
  async removeRecent(path) {
    const result = await commands.recentsRemove(path);
    if (result.status === "error") throw new Error(describeError(result.error));
    return result.data;
  },
  async onDrop(handler) {
    return getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") handler(event.payload.paths);
    });
  },
};
