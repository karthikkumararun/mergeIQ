//! Git adapter: conflict discovery, stage reads, resolve/stage.
#![warn(missing_docs)]

mod blobs;
mod classify;
mod conflicts;
mod context;
mod control;
mod details;
mod error;
mod exec;
mod load;
mod lockfiles;
mod operation;
mod paths;
mod renames;
mod repo;
mod submodule;
mod take;
mod watch;
mod write;

pub use blobs::StageBlobs;
pub use classify::{
    classify, is_image_name, lockfile_kind, parse_lfs_pointer, BlobFacts, ConflictClass,
    LfsPointer, LockfileKind, OVERSIZED_BYTES, SNIFF_BYTES,
};
pub use conflicts::{ConflictEntry, ConflictType, StageEntry};
pub use context::{CommitSummary, FileContext, MAX_CONTEXT_COMMITS};
pub use control::ControlOutcome;
pub use details::{
    mime_for, token_of, ConflictDetails, ModifyDeleteView, StageBlob, StageMeta, MAX_DIFF_BYTES,
    MAX_PREVIEW_BYTES,
};
pub use error::{GitError, Result};
pub use exec::{parse_version, GitExec, GitVersion, MIN_VERSION};
pub use load::{ConflictLoad, RepoStatus};
pub use lockfiles::{
    check_command, default_command, merge_go_sum, run_command, split_command, GoSumLine, GoSumMark,
    GoSumMerge, OutputStream, RegenerateResult, RunResult,
};
pub use operation::{Operation, SideLabel, SideLabels};
pub use paths::{PathToken, RepoPath};
pub use renames::{Rename, RenameInfo, RenameOutcome, RenamePair, RenameSide};
pub use repo::Repo;
pub use submodule::{SubmoduleCommit, SubmoduleDetails, SubmoduleRelation};
pub use watch::{RepoWatcher, DEBOUNCE};
pub use write::{write_file_atomic, AcceptSide};

/// This crate's version, from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
