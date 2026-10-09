//! IPC commands wrapping `mergeiq-git`. One repository is open at a time.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergeiq_core::{Analysis, EncodingInfo, Options, WhitespacePolicy};
use mergeiq_git::{
    AcceptSide, ConflictLoad, ControlOutcome, GitExec, PathToken, Repo, RepoStatus, RepoWatcher,
};
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::ipc::IpcError;
use crate::settings;

/// Emitted (as `repo-changed`) when the index or operation state changes outside the app.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
pub struct RepoChanged;

struct Session {
    repo: Repo,
    _watcher: RepoWatcher,
}

/// The currently open repository and its watcher.
#[derive(Default)]
pub struct GitState {
    session: Mutex<Option<Session>>,
}

impl GitState {
    fn repo(&self) -> Result<Repo, IpcError> {
        self.session
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.repo.clone()))
            .ok_or(IpcError::NoRepo)
    }
}

fn configured_git() -> Option<PathBuf> {
    let settings = settings::load();
    settings
        .extra
        .get("gitPath")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}

fn open_session(app: &AppHandle, path: &Path) -> Result<Session, IpcError> {
    let exec = GitExec::locate(configured_git().as_deref())?;
    let repo = Repo::open(exec, path)?;
    let handle = app.clone();
    let watcher = repo.watch(move || {
        if let Err(err) = RepoChanged.emit(&handle) {
            tracing::warn!(error = %err, "failed to emit repo-changed");
        }
    })?;
    Ok(Session {
        repo,
        _watcher: watcher,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn repo_open(
    app: AppHandle,
    state: State<'_, GitState>,
    path: String,
) -> Result<RepoStatus, IpcError> {
    let session = open_session(&app, Path::new(&path))?;
    let status = session.repo.status()?;
    if let Ok(mut slot) = state.session.lock() {
        *slot = Some(session);
    }
    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub async fn repo_status(state: State<'_, GitState>) -> Result<RepoStatus, IpcError> {
    Ok(state.repo()?.status()?)
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_load(
    state: State<'_, GitState>,
    path: PathToken,
) -> Result<ConflictLoad, IpcError> {
    Ok(state.repo()?.load_conflict(&path, &Options::default())?)
}

/// Re-runs the merge analysis for one conflicted file with another whitespace policy
/// (the merge editor's whitespace selector).
#[tauri::command]
#[specta::specta]
pub async fn conflict_analyze(
    state: State<'_, GitState>,
    path: PathToken,
    whitespace: WhitespacePolicy,
) -> Result<Analysis, IpcError> {
    let opts = Options {
        whitespace,
        ..Options::default()
    };
    let load = state.repo()?.load_conflict(&path, &opts)?;
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
    state: State<'_, GitState>,
    path: PathToken,
    text: String,
    encoding: EncodingInfo,
    stage: bool,
) -> Result<(), IpcError> {
    let repo = state.repo()?;
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
    state: State<'_, GitState>,
    path: PathToken,
    side: AcceptSide,
) -> Result<(), IpcError> {
    Ok(state.repo()?.accept_side(&path.decode()?, side)?)
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_restore(state: State<'_, GitState>, path: PathToken) -> Result<(), IpcError> {
    Ok(state.repo()?.restore_conflict(&path.decode()?)?)
}

#[tauri::command]
#[specta::specta]
pub async fn op_continue(state: State<'_, GitState>) -> Result<ControlOutcome, IpcError> {
    Ok(state.repo()?.op_continue()?)
}

#[tauri::command]
#[specta::specta]
pub async fn op_abort(state: State<'_, GitState>) -> Result<ControlOutcome, IpcError> {
    Ok(state.repo()?.op_abort()?)
}

#[tauri::command]
#[specta::specta]
pub async fn op_skip(state: State<'_, GitState>) -> Result<ControlOutcome, IpcError> {
    Ok(state.repo()?.op_skip()?)
}
