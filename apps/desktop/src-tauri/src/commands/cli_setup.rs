//! IPC commands behind Settings › Command line.

use std::path::{Path, PathBuf};

use mergeiq_git::GitExec;
use serde::Serialize;

use crate::cli::git_config::{self, mergetool_commands};
use crate::cli::install::{self, InstallDir, InstallOutcome};
use crate::ipc::IpcError;
use crate::settings;

/// What the Command line settings page needs to render.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CliSetupInfo {
    /// `"macos"`, `"windows"` or `"linux"`.
    pub platform: String,
    /// Folders offered in the picker (on Windows: the install folder only).
    pub dirs: Vec<InstallDir>,
    /// Where the `mergeiq` command is already installed, if anywhere we looked.
    pub installed_at: Option<String>,
    /// git's global config already registers MergeIQ as the merge tool.
    pub mergetool_configured: bool,
}

fn exe() -> Result<PathBuf, IpcError> {
    std::env::current_exe().map_err(|e| IpcError::Request(e.to_string()))
}

fn home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_default()
}

fn git() -> Option<GitExec> {
    let configured = settings::load()
        .extra
        .get("gitPath")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    GitExec::locate(configured.as_deref()).ok()
}

fn dirs(exe: &Path) -> Vec<InstallDir> {
    if cfg!(windows) {
        let dir = exe.parent().unwrap_or(exe);
        return vec![InstallDir {
            path: dir.display().to_string(),
            label: dir.display().to_string(),
            needs_admin: false,
        }];
    }
    install::default_dirs(&home())
}

#[tauri::command]
#[specta::specta]
pub async fn cli_setup_info() -> Result<CliSetupInfo, IpcError> {
    let exe = exe()?;
    let dirs = dirs(&exe);
    let installed_at = if cfg!(windows) {
        None
    } else {
        dirs.iter()
            .find_map(|d| install::installed_link(Path::new(&d.path), &exe))
            .map(|p| p.display().to_string())
    };
    Ok(CliSetupInfo {
        platform: std::env::consts::OS.to_string(),
        dirs,
        installed_at,
        mergetool_configured: git().is_some_and(|g| git_config::is_configured(&g)),
    })
}

/// Installs the command into `dir` (macOS/Linux: a symlink; Windows: nothing to copy, the
/// install folder just needs to be on PATH) and reports whether `dir` is on PATH.
#[tauri::command]
#[specta::specta]
pub async fn cli_install(dir: String, admin: bool) -> Result<InstallOutcome, IpcError> {
    let exe = exe()?;
    let dir = PathBuf::from(dir);
    let link = if cfg!(windows) {
        exe.clone()
    } else if admin {
        install::install_link_as_admin(&exe, &dir).map_err(IpcError::Request)?
    } else {
        install::install_link(&exe, &dir).map_err(IpcError::Request)?
    };
    let on_path = {
        let mut found = install::user_path().is_some_and(|p| install::is_on_path(&dir, &p));
        if !found {
            found = install::login_shell_path().is_some_and(|p| install::is_on_path(&dir, &p));
        }
        found
    };
    let path_hint = (!on_path && !cfg!(windows))
        .then(|| install::path_hint(&dir, &home(), std::env::var("SHELL").ok().as_deref()));
    Ok(InstallOutcome {
        link_path: link.display().to_string(),
        on_path,
        path_hint,
    })
}

/// Windows: adds `dir` to the user PATH.
#[tauri::command]
#[specta::specta]
pub async fn cli_add_to_path(dir: String) -> Result<(), IpcError> {
    install::add_to_user_path(Path::new(&dir)).map_err(IpcError::Request)
}

/// The `git config --global` commands, as displayed.
#[tauri::command]
#[specta::specta]
pub async fn git_mergetool_commands(no_backup: bool) -> Vec<String> {
    mergetool_commands(no_backup)
        .iter()
        .map(git_config::ConfigCommand::display)
        .collect()
}

/// Runs the commands. Only invoked after the user confirms in the UI.
#[tauri::command]
#[specta::specta]
pub async fn git_mergetool_configure(no_backup: bool) -> Result<(), IpcError> {
    let exec = git().ok_or(IpcError::Git(mergeiq_git::GitError::GitNotFound))?;
    git_config::apply(&exec, &mergetool_commands(no_backup)).map_err(IpcError::Request)
}
