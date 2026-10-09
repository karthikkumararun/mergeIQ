//! IPC for special conflicts (`special-conflicts`): binary, image, symlink, submodule, LFS,
//! oversized, rename, `go.sum` and lockfile resolution.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use mergeiq_core::EncodingInfo;
use mergeiq_git::{
    AcceptSide, ConflictDetails, GoSumMerge, LockfileKind, ModifyDeleteView, OutputStream,
    PathToken, RegenerateResult, RenameOutcome, StageBlob, SubmoduleDetails,
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

use crate::commands::git::repo_of;
use crate::ipc::IpcError;
use crate::repos::Repos;
use crate::settings;

/// The kinds that have a regeneration command, in display order.
const REGENERABLE: [LockfileKind; 6] = [
    LockfileKind::Npm,
    LockfileKind::Pnpm,
    LockfileKind::Yarn,
    LockfileKind::Poetry,
    LockfileKind::Cargo,
    LockfileKind::Gradle,
];

fn key(kind: LockfileKind) -> String {
    format!("{kind:?}")
}

/// Running lockfile commands, so they can be cancelled.
#[derive(Default)]
pub struct LockfileRuns {
    next: AtomicU32,
    cancels: Mutex<HashMap<u32, Arc<AtomicBool>>>,
}

impl LockfileRuns {
    fn start(&self) -> (u32, Arc<AtomicBool>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut map) = self.cancels.lock() {
            map.insert(id, flag.clone());
        }
        (id, flag)
    }

    fn finish(&self, id: u32) {
        if let Ok(mut map) = self.cancels.lock() {
            map.remove(&id);
        }
    }

    fn cancel(&self, id: u32) {
        if let Some(flag) = self.cancels.lock().ok().and_then(|m| m.get(&id).cloned()) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

/// One line of a running lockfile command (emitted as `lockfile-output`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct LockfileOutput {
    pub run: u32,
    pub stream: OutputStream,
    pub line: String,
}

/// A lockfile command ended (emitted as `lockfile-finished`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct LockfileFinished {
    pub run: u32,
    /// Set unless the run could not even start (see `error`).
    pub result: Option<RegenerateResult>,
    pub error: Option<String>,
}

/// A lockfile kind with its default and effective regeneration command.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LockfileCommand {
    pub kind: LockfileKind,
    pub default_command: String,
    /// The command that will be proposed (the default unless customised).
    pub command: String,
    pub custom: bool,
}

/// Current text of a working-tree file, for the plain editor.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkingText {
    pub text: String,
    pub encoding: EncodingInfo,
}

/// The proposed command for every regenerable lockfile, given the user's customisations.
fn lockfile_commands_with(
    custom: &std::collections::BTreeMap<String, String>,
) -> Vec<LockfileCommand> {
    REGENERABLE
        .iter()
        .filter_map(|&kind| {
            let default_command = mergeiq_git::default_command(kind)?.to_string();
            let own = custom.get(&key(kind)).filter(|c| !c.trim().is_empty());
            Some(LockfileCommand {
                kind,
                command: own.cloned().unwrap_or_else(|| default_command.clone()),
                custom: own.is_some(),
                default_command,
            })
        })
        .collect()
}

fn lockfile_commands_now() -> Vec<LockfileCommand> {
    lockfile_commands_with(&settings::load().lockfile_commands)
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_details(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<ConflictDetails, IpcError> {
    Ok(repo_of(&repos, repo)?.conflict_details(&path)?)
}

/// A stage's bytes for an image preview (capped at 20 MB).
#[tauri::command]
#[specta::specta]
pub async fn conflict_stage_blob(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
    stage: u8,
) -> Result<StageBlob, IpcError> {
    Ok(repo_of(&repos, repo)?.stage_blob(&path, stage)?)
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_modify_delete_view(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<ModifyDeleteView, IpcError> {
    Ok(repo_of(&repos, repo)?.modify_delete_view(&path)?)
}

/// Takes one side byte for byte and stages it (deleting the path if that side deleted it).
#[tauri::command]
#[specta::specta]
pub async fn conflict_use_side(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
    side: AcceptSide,
) -> Result<(), IpcError> {
    Ok(repo_of(&repos, repo)?.use_side_exact(&path.decode()?, side)?)
}

/// "Keep and edit": writes the surviving side unstaged and returns its text.
#[tauri::command]
#[specta::specta]
pub async fn conflict_keep_and_edit(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
    side: AcceptSide,
) -> Result<WorkingText, IpcError> {
    let repo = repo_of(&repos, repo)?;
    let decoded = path.decode()?;
    repo.write_side_unstaged(&decoded, side)?;
    working_text(&repo, &decoded)
}

/// The working-tree text of a file (for the plain editor tab).
#[tauri::command]
#[specta::specta]
pub async fn conflict_working_text(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<WorkingText, IpcError> {
    working_text(&repo_of(&repos, repo)?, &path.decode()?)
}

fn working_text(
    repo: &mergeiq_git::Repo,
    path: &mergeiq_git::RepoPath,
) -> Result<WorkingText, IpcError> {
    let bytes = repo
        .read_working(path)?
        .ok_or_else(|| IpcError::Request("the file does not exist".into()))?;
    let (text, encoding) = mergeiq_core::decode(&bytes, mergeiq_core::Side::Ours)
        .map_err(|_| IpcError::Request("the file is binary".into()))?;
    Ok(WorkingText { text, encoding })
}

#[tauri::command]
#[specta::specta]
pub async fn submodule_details(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<SubmoduleDetails, IpcError> {
    Ok(repo_of(&repos, repo)?.submodule_details(&path)?)
}

/// Chooses the final path of a rename/rename conflict.
#[tauri::command]
#[specta::specta]
pub async fn rename_choose(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<RenameOutcome, IpcError> {
    Ok(repo_of(&repos, repo)?.choose_rename_path(&path.decode()?)?)
}

#[tauri::command]
#[specta::specta]
pub async fn go_sum_preview(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<GoSumMerge, IpcError> {
    Ok(repo_of(&repos, repo)?.go_sum_preview(&path)?)
}

/// Writes the `go.sum` union merge and stages it.
#[tauri::command]
#[specta::specta]
pub async fn go_sum_union(
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<(), IpcError> {
    Ok(repo_of(&repos, repo)?.go_sum_union(&path)?)
}

#[tauri::command]
#[specta::specta]
pub fn get_lockfile_commands() -> Vec<LockfileCommand> {
    lockfile_commands_now()
}

/// Sets (or, with `None`, resets) the regeneration command for a lockfile kind.
#[tauri::command]
#[specta::specta]
pub fn set_lockfile_command(
    kind: LockfileKind,
    command: Option<String>,
) -> Result<Vec<LockfileCommand>, IpcError> {
    let mut current = settings::load();
    match command
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
    {
        Some(command) => {
            mergeiq_git::split_command(&command)?;
            current.lockfile_commands.insert(key(kind), command);
        }
        None => {
            current.lockfile_commands.remove(&key(kind));
        }
    }
    settings::save(&current).map_err(|e| IpcError::Settings(e.to_string()))?;
    Ok(lockfile_commands_now())
}

/// Takes `side`'s lockfile, then runs `command` (already confirmed by the user) in the
/// lockfile's directory. Returns the run id at once; output arrives as `lockfile-output`
/// events and the end as `lockfile-finished`, both to the calling window.
#[tauri::command]
#[specta::specta]
pub async fn lockfile_regenerate(
    window: tauri::WebviewWindow,
    repos: State<'_, Repos>,
    runs: State<'_, Arc<LockfileRuns>>,
    repo: u32,
    path: PathToken,
    side: AcceptSide,
    command: String,
) -> Result<u32, IpcError> {
    let repo = repo_of(&repos, repo)?;
    let decoded = path.decode()?;
    let file: PathBuf = decoded.in_root(repo.root())?;
    let dir = file.parent().unwrap_or(repo.root());
    mergeiq_git::check_command(&command, dir)?;

    let runs = runs.inner().clone();
    let (run, cancel) = runs.start();
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    std::thread::spawn(move || {
        let outcome = repo.regenerate_lockfile(&path, side, &command, &cancel, |stream, line| {
            let _ = LockfileOutput {
                run,
                stream,
                line: line.to_string(),
            }
            .emit_to(&app, label.as_str());
        });
        runs.finish(run);
        let finished = match outcome {
            Ok(result) => LockfileFinished {
                run,
                result: Some(result),
                error: None,
            },
            Err(err) => LockfileFinished {
                run,
                result: None,
                error: Some(err.to_string()),
            },
        };
        let _ = finished.emit_to(&app, label.as_str());
    });
    Ok(run)
}

/// Stops a running lockfile command (the lockfile stays unstaged).
#[tauri::command]
#[specta::specta]
pub fn lockfile_cancel(runs: State<'_, Arc<LockfileRuns>>, run: u32) {
    runs.cancel(run);
}

/// Opens the working-tree file in the system's default application.
#[tauri::command]
#[specta::specta]
pub async fn open_working_file(
    app: tauri::AppHandle,
    repos: State<'_, Repos>,
    repo: u32,
    path: PathToken,
) -> Result<(), IpcError> {
    let repo = repo_of(&repos, repo)?;
    let file = path.decode()?.in_root(repo.root())?;
    if !file.exists() {
        return Err(IpcError::Request(
            "the file is not in the working tree".into(),
        ));
    }
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| IpcError::Request(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn defaults_cover_every_regenerable_lockfile() {
        let all = lockfile_commands_with(&BTreeMap::new());
        let by_kind: BTreeMap<String, &LockfileCommand> =
            all.iter().map(|c| (key(c.kind), c)).collect();
        assert_eq!(by_kind.len(), 6);
        assert_eq!(by_kind["Pnpm"].command, "pnpm install --lockfile-only");
        assert_eq!(by_kind["Cargo"].command, "cargo update --workspace");
        assert!(all
            .iter()
            .all(|c| !c.custom && c.command == c.default_command));
        assert!(!by_kind.contains_key("GoSum"), "go.sum has no command");
    }

    #[test]
    fn a_customised_command_replaces_only_its_kind() {
        let custom = BTreeMap::from([
            (
                "Pnpm".to_string(),
                "pnpm install --lockfile-only --offline".to_string(),
            ),
            ("Npm".to_string(), "   ".to_string()),
        ]);
        let all = lockfile_commands_with(&custom);
        let pnpm = all.iter().find(|c| c.kind == LockfileKind::Pnpm).unwrap();
        assert!(pnpm.custom);
        assert_eq!(pnpm.command, "pnpm install --lockfile-only --offline");
        assert_eq!(pnpm.default_command, "pnpm install --lockfile-only");
        // A blank override is ignored.
        let npm = all.iter().find(|c| c.kind == LockfileKind::Npm).unwrap();
        assert!(!npm.custom);
    }

    #[test]
    fn lockfile_runs_can_be_cancelled_by_id() {
        let runs = LockfileRuns::default();
        let (a, flag_a) = runs.start();
        let (b, flag_b) = runs.start();
        assert_ne!(a, b);
        runs.cancel(a);
        assert!(flag_a.load(Ordering::Relaxed));
        assert!(!flag_b.load(Ordering::Relaxed));
        runs.finish(a);
        runs.cancel(a); // unknown ids are ignored
    }
}
