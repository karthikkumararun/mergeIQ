use crate::commands::{ai, cli_setup, git, repos, requests, special, structural};
use crate::settings::{self, MergeEditorSettings, Settings};

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
}

#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "MergeIQ".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
    }
}

#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(tag = "kind", content = "message")]
pub enum IpcError {
    #[error("failed to save settings: {0}")]
    Settings(String),
    #[error("{0}")]
    Git(#[from] mergeiq_git::GitError),
    #[error("no repository is open")]
    NoRepo,
    #[error("{0}")]
    Request(String),
}

/// Typed IPC view of [`Settings`]; unknown keys in the settings file are preserved on
/// disk but not exposed over IPC.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub theme: String,
}

impl From<Settings> for SettingsDto {
    fn from(settings: Settings) -> Self {
        Self {
            theme: settings.theme,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_settings() -> SettingsDto {
    settings::load().into()
}

/// Native window chrome (title bar, frame) theme for a saved theme mode; `None` follows the OS.
pub fn native_theme(mode: &str) -> Option<tauri::Theme> {
    match mode {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    }
}

#[tauri::command]
#[specta::specta]
pub fn update_settings(
    app: tauri::AppHandle,
    settings: SettingsDto,
) -> Result<SettingsDto, IpcError> {
    let mut current = settings::load();
    current.theme = settings.theme;
    settings::save(&current).map_err(|err| IpcError::Settings(err.to_string()))?;
    app.set_theme(native_theme(&current.theme));
    Ok(current.into())
}

#[tauri::command]
#[specta::specta]
pub fn get_merge_editor_settings() -> MergeEditorSettings {
    settings::load().merge_editor
}

#[tauri::command]
#[specta::specta]
pub fn update_merge_editor_settings(
    settings: MergeEditorSettings,
) -> Result<MergeEditorSettings, IpcError> {
    let mut current = settings::load();
    current.merge_editor = settings;
    settings::save(&current).map_err(|err| IpcError::Settings(err.to_string()))?;
    Ok(current.merge_editor)
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            app_info,
            get_settings,
            update_settings,
            get_merge_editor_settings,
            update_merge_editor_settings,
            repos::repo_open,
            repos::repo_info,
            repos::recents_list,
            repos::recents_remove,
            git::repo_status,
            git::conflict_load,
            git::conflict_analyze,
            git::conflict_save,
            git::conflict_accept_side,
            git::conflict_accept_many,
            git::conflict_delete,
            git::conflict_restore,
            git::op_continue,
            git::op_abort,
            git::op_skip,
            requests::merge_request_load,
            requests::merge_request_analyze,
            requests::merge_request_save,
            requests::request_close,
            structural::structural_resolve,
            special::conflict_details,
            special::conflict_stage_blob,
            special::conflict_modify_delete_view,
            special::conflict_use_side,
            special::conflict_keep_and_edit,
            special::conflict_working_text,
            special::submodule_details,
            special::rename_choose,
            special::go_sum_preview,
            special::go_sum_union,
            special::get_lockfile_commands,
            special::set_lockfile_command,
            special::lockfile_regenerate,
            special::lockfile_cancel,
            special::open_working_file,
            ai::ai_get_settings,
            ai::ai_update_settings,
            ai::ai_set_repo_decision,
            ai::ai_key_info,
            ai::ai_set_key,
            ai::ai_delete_key,
            ai::ai_test_connection,
            ai::ai_usage,
            ai::ai_reset_usage,
            ai::ai_status,
            ai::ai_preview,
            ai::ai_estimate,
            ai::ai_explain,
            ai::ai_suggest,
            ai::ai_cancel,
            ai::ai_open_settings,
            ai::take_pending_settings,
            cli_setup::cli_setup_info,
            cli_setup::cli_install,
            cli_setup::cli_add_to_path,
            cli_setup::git_mergetool_commands,
            cli_setup::git_mergetool_configure,
        ])
        .events(tauri_specta::collect_events![
            git::RepoChanged,
            special::LockfileOutput,
            special::LockfileFinished,
            ai::AiDelta,
            ai::OpenSettings
        ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use specta_typescript::Typescript;

    /// Fails if `apps/desktop/src/ipc/bindings.ts` is out of date. Regenerate it with
    /// `MERGEIQ_UPDATE_BINDINGS=1 cargo test -p mergeiq-desktop bindings_are_up_to_date` or by
    /// running the app in debug mode (`cargo run -p mergeiq-desktop` or `pnpm tauri dev`).
    #[test]
    fn bindings_are_up_to_date() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("bindings.ts");
        specta_builder()
            .export(Typescript::default(), &out)
            .expect("failed to export typescript bindings");

        let generated = std::fs::read_to_string(&out).expect("read generated bindings");
        let committed_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts");
        if std::env::var_os("MERGEIQ_UPDATE_BINDINGS").is_some() {
            std::fs::write(&committed_path, &generated).expect("write bindings");
            return;
        }
        let committed = std::fs::read_to_string(&committed_path).expect(
            "failed to read apps/desktop/src/ipc/bindings.ts — generate it first in debug mode",
        );

        assert_eq!(
            generated, committed,
            "apps/desktop/src/ipc/bindings.ts is stale; regenerate it in debug mode"
        );
    }
}

#[cfg(test)]
mod native_theme_tests {
    use super::native_theme;
    use tauri::Theme;

    #[test]
    fn dark_and_light_modes_force_the_native_chrome() {
        assert_eq!(native_theme("dark"), Some(Theme::Dark));
        assert_eq!(native_theme("light"), Some(Theme::Light));
    }

    #[test]
    fn system_and_unknown_modes_follow_the_os() {
        assert_eq!(native_theme("system"), None);
        assert_eq!(native_theme("anything-else"), None);
    }
}
