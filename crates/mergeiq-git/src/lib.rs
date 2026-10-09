//! Git adapter: conflict discovery, stage reads, resolve/stage.
#![warn(missing_docs)]
#![allow(dead_code)] // removed once all modules land

mod blobs;
mod conflicts;
mod context;
mod control;
mod error;
mod exec;
mod operation;
mod paths;
mod repo;
mod write;

pub use blobs::StageBlobs;
pub use conflicts::{ConflictEntry, ConflictType, StageEntry};
pub use context::{CommitSummary, FileContext, MAX_CONTEXT_COMMITS};
pub use control::ControlOutcome;
pub use error::{GitError, Result};
pub use exec::{parse_version, GitExec, GitVersion, MIN_VERSION};
pub use operation::{Operation, SideLabel, SideLabels};
pub use paths::{PathToken, RepoPath};
pub use repo::Repo;
pub use write::AcceptSide;

/// This crate's version, from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
