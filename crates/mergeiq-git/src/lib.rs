//! Git adapter: conflict discovery, stage reads, resolve/stage.
#![warn(missing_docs)]
#![allow(dead_code)] // removed once all modules land

mod error;
mod exec;
mod paths;
mod repo;

pub use error::{GitError, Result};
pub use exec::{parse_version, GitExec, GitVersion, MIN_VERSION};
pub use paths::{PathToken, RepoPath};
pub use repo::Repo;

/// This crate's version, from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
