//! Repository-level operations shared by IPC commands and tests.

use mergeiq_git::{AcceptSide, GitError, PathToken, Repo};
use serde::Serialize;

/// One path a batch action could not apply to.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchFailure {
    pub path: PathToken,
    pub message: String,
}

/// Outcome of a multi-file action: what was applied and what failed (the rest still ran).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub done: Vec<PathToken>,
    pub failed: Vec<BatchFailure>,
}

fn decode(token: &PathToken) -> Result<mergeiq_git::RepoPath, GitError> {
    token.decode()
}

/// Accepts one side for every path: written, staged (or deleted when that side deleted it).
pub fn accept_many(repo: &Repo, paths: &[PathToken], side: AcceptSide) -> BatchResult {
    let mut result = BatchResult {
        done: Vec::new(),
        failed: Vec::new(),
    };
    for token in paths {
        match decode(token).and_then(|p| repo.accept_side(&p, side)) {
            Ok(()) => result.done.push(token.clone()),
            Err(err) => result.failed.push(BatchFailure {
                path: token.clone(),
                message: err.to_string(),
            }),
        }
    }
    result
}
