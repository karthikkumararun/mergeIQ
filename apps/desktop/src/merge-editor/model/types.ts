import type {
  Analysis,
  ChunkKind,
  EncodingInfo,
  FileContext,
  PathToken,
  SideLabel,
  Terminator,
} from "../../ipc/bindings";

export type WhitespacePolicy =
  "Exact" | "TrimTrailing" | "IgnoreAmount" | "IgnoreAll";

/** Which pane a side action belongs to. `left` is ours, `right` is theirs. */
export type Side = "left" | "right";

export type SideStatus = "pending" | "applied" | "ignored" | "na";

export type ResolutionKind =
  "none" | "applied" | "edited" | "auto" | "whole-file";

/** The mutable part of a chunk's session state (everything except its position). */
export interface ChunkStatus {
  leftStatus: SideStatus;
  rightStatus: SideStatus;
  resolution: ResolutionKind;
}

/** Per-chunk session state, kept in the Result view's `EditorState`. */
export interface ChunkState extends ChunkStatus {
  id: number;
  kind: ChunkKind;
  /** Start of the chunk's range in the Result document. */
  from: number;
  /** End of the chunk's range in the Result document (exclusive; includes the last newline). */
  to: number;
}

export interface ResultLine {
  text: string;
  term: Terminator;
}

/** An unresolved conflict to render as conflict markers (mirrors `mergeiq-core`). */
export interface UnresolvedConflict {
  chunkId: number;
  /** Index into `lines` where the markers belong. */
  at: number;
  oursLabel: string;
  oursLines: ResultLine[];
  theirsLabel: string;
  theirsLines: ResultLine[];
}

export type SaveMode = "resolved" | "markers" | "force";

export interface SaveResult {
  lines: ResultLine[];
  unresolvedIds: number[];
  /** Conflicts to splice in as markers when `mode === "markers"`. */
  unresolved: UnresolvedConflict[];
  mode: SaveMode;
  encoding: EncodingInfo;
}

export interface MergeLabels {
  left: SideLabel;
  right: SideLabel;
  base?: SideLabel;
}

export interface MergeDocument {
  pathToken: PathToken;
  displayPath: string;
  analysis: Analysis;
  labels: MergeLabels;
  context?: FileContext;
  languageHint?: string;
}

export interface MergeSettings {
  autoApplyNonConflicting: boolean;
  showBase: boolean;
  collapseUnchanged: boolean;
  syncScroll: boolean;
  whitespacePolicy: WhitespacePolicy;
}

export const DEFAULT_SETTINGS: MergeSettings = {
  autoApplyNonConflicting: false,
  showBase: false,
  collapseUnchanged: false,
  syncScroll: true,
  whitespacePolicy: "Exact",
};
