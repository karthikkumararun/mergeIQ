//! Opening repositories (one window each) and the recent-repositories list.

use std::path::{Path, PathBuf};

use mergeiq_git::GitExec;
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_specta::Event;

use crate::commands::git::RepoChanged;
use crate::ipc::IpcError;
use crate::recents::{self, RecentRepo};
use crate::repos::Repos;
use crate::settings;

/// Window label for a repository.
pub fn window_label(id: u32) -> String {
    format!("repo-{id}")
}

/// The id encoded in a repository window label (`repo-3`).
pub fn repo_id(label: &str) -> Option<u32> {
    label.strip_prefix("repo-")?.parse().ok()
}

fn locate_git() -> Result<GitExec, IpcError> {
    let configured = settings::load()
        .extra
        .get("gitPath")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    Ok(GitExec::locate(configured.as_deref())?)
}

/// Opens the repository containing `path` in its own window, or focuses the window it
/// already has. Records the repository in the recents list.
pub fn open_repo_window(app: &AppHandle, repos: &Repos, path: &Path) -> Result<u32, IpcError> {
    let emitter = app.clone();
    let opened = repos.open(locate_git()?, path, move |id, repo| {
        let label = window_label(id);
        repo.watch(move || {
            if let Err(err) = RepoChanged.emit_to(&emitter, label.as_str()) {
                tracing::warn!(error = %err, "failed to emit repo-changed");
            }
        })
    })?;
    let repo = repos.get(opened.id).ok_or(IpcError::NoRepo)?;
    let root = repo.root().display().to_string();
    let label = window_label(opened.id);

    let focus = |app: &AppHandle| {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
            true
        } else {
            false
        }
    };
    if opened.created || !focus(app) {
        let title = format!("{} — MergeIQ", recents::display_name(&root));
        let built = WebviewWindowBuilder::new(
            app,
            &label,
            WebviewUrl::App(format!("repo/{}", opened.id).into()),
        )
        .title(title)
        .inner_size(1360.0, 860.0)
        .min_inner_size(1024.0, 640.0)
        .build();
        if let Err(err) = built {
            repos.remove(opened.id);
            return Err(IpcError::Request(err.to_string()));
        }
    }
    record_recent(&root);
    Ok(opened.id)
}

fn record_recent(root: &str) {
    let mut current = settings::load();
    recents::push(&mut current.recent_repos, root, recents::now());
    if let Err(err) = settings::save(&current) {
        tracing::warn!(error = %err, "failed to save recent repositories");
    }
}

/// Opens (or focuses) a repository window.
#[tauri::command]
#[specta::specta]
pub async fn repo_open(
    app: AppHandle,
    repos: State<'_, Repos>,
    path: String,
) -> Result<u32, IpcError> {
    open_repo_window(&app, &repos, Path::new(&path))
}

/// The repository a window was opened for.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name: String,
    pub path: String,
}

#[tauri::command]
#[specta::specta]
pub async fn repo_info(repos: State<'_, Repos>, repo: u32) -> Result<RepoInfo, IpcError> {
    let repo = repos.get(repo).ok_or(IpcError::NoRepo)?;
    let path = repo.root().display().to_string();
    Ok(RepoInfo {
        name: recents::display_name(&path),
        path,
    })
}

/// A remembered repository, as the home view lists it.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepoDto {
    pub name: String,
    pub path: String,
    pub opened_at: u32,
    /// `false` when the folder no longer exists.
    pub exists: bool,
}

fn to_dto(r: &RecentRepo) -> RecentRepoDto {
    RecentRepoDto {
        name: recents::display_name(&r.path),
        path: r.path.clone(),
        opened_at: r.opened_at,
        exists: Path::new(&r.path).is_dir(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn recents_list() -> Vec<RecentRepoDto> {
    settings::load().recent_repos.iter().map(to_dto).collect()
}

#[tauri::command]
#[specta::specta]
pub async fn recents_remove(path: String) -> Result<Vec<RecentRepoDto>, IpcError> {
    let mut current = settings::load();
    recents::remove(&mut current.recent_repos, &path);
    settings::save(&current).map_err(|e| IpcError::Settings(e.to_string()))?;
    Ok(current.recent_repos.iter().map(to_dto).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_labels_round_trip() {
        assert_eq!(repo_id(&window_label(7)), Some(7));
        assert_eq!(repo_id("merge-7"), None);
        assert_eq!(repo_id("repo-x"), None);
    }
}
