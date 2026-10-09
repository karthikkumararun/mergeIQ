//! IPC commands wrapping `mergeiq-git`. One repository is open at a time.

use mergeiq_core::{Analysis, EncodingInfo, Options, WhitespacePolicy};
use mergeiq_git::{AcceptSide, ConflictLoad, ControlOutcome, PathToken, Repo, RepoStatus};
use tauri::State;

use crate::ipc::IpcError;
use crate::repo_service::{self, BatchResult};
use crate::repos::Repos;

/// Emitted (as `repo-changed`) to a repository's window when its index or operation state changes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
pub struct RepoChanged;

fn repo_of(repos: &Repos, id: u32) -> Result<Repo, IpcError> {
    repos.get(id).ok_or(IpcError::NoRepo)
}

#[tauri::command]
#[specta::specta]
pub async fn repo_status(repos: State<'_, Repos>, repo: u32) -> Result<RepoStatus, IpcError> {
    Ok(repo_of(&repos, repo)?.status()?)
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_load(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<ConflictLoad, IpcError> {
    Ok(repo_of(&repos, repo)?.load_conflict(&path, &Options::default())?)
}

/// Re-runs the merge analysis for one conflicted file with another whitespace policy
/// (the merge editor's whitespace selector).
#[tauri::command]
#[specta::specta]
pub async fn conflict_analyze(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
    whitespace: WhitespacePolicy,
) -> Result<Analysis, IpcError> {
    let opts = Options {
        whitespace,
        ..Options::default()
    };
    let load = repo_of(&repos, repo)?.load_conflict(&path, &opts)?;
    load.analysis.ok_or_else(|| {
        IpcError::Git(mergeiq_git::GitError::Unsupported {
            what: load
                .analysis_error
                .unwrap_or_else(|| "this file cannot be analysed as text".to_string()),
        })
    })
}

/// Saves editor text. With `stage` the path is also `git add`ed (resolved).
#[tauri::command]
#[specta::specta]
pub async fn conflict_save(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
    text: String,
    encoding: EncodingInfo,
    stage: bool,
) -> Result<(), IpcError> {
    let repo = repo_of(&repos, repo)?;
    let path = path.decode()?;
    let bytes = mergeiq_core::encode(&text, &encoding);
    if stage {
        repo.save_resolved(&path, &bytes)?;
    } else {
        repo.save_unresolved(&path, &bytes)?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_accept_side(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
    side: AcceptSide,
) -> Result<(), IpcError> {
    Ok(repo_of(&repos, repo)?.accept_side(&path.decode()?, side)?)
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_restore(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<(), IpcError> {
    Ok(repo_of(&repos, repo)?.restore_conflict(&path.decode()?)?)
}

#[tauri::command]
#[specta::specta]
pub async fn op_continue(repos: State<'_, Repos>, repo: u32) -> Result<ControlOutcome, IpcError> {
    Ok(repo_of(&repos, repo)?.op_continue()?)
}

#[tauri::command]
#[specta::specta]
pub async fn op_abort(repos: State<'_, Repos>, repo: u32) -> Result<ControlOutcome, IpcError> {
    Ok(repo_of(&repos, repo)?.op_abort()?)
}

#[tauri::command]
#[specta::specta]
pub async fn op_skip(repos: State<'_, Repos>, repo: u32) -> Result<ControlOutcome, IpcError> {
    Ok(repo_of(&repos, repo)?.op_skip()?)
}

/// Accepts one side for several files. Every path is tried; failures are reported per path.
#[tauri::command]
#[specta::specta]
pub async fn conflict_accept_many(
    repos: State<'_, Repos>,
    repo: u32,
    paths: Vec<PathToken>,
    side: AcceptSide,
) -> Result<BatchResult, IpcError> {
    Ok(repo_service::accept_many(
        &repo_of(&repos, repo)?,
        &paths,
        side,
    ))
}

/// Resolves a conflict by deleting the file.
#[tauri::command]
#[specta::specta]
pub async fn conflict_delete(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<(), IpcError> {
    Ok(repo_of(&repos, repo)?.delete_resolved(&path.decode()?)?)
}
