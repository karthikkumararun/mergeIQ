use crate::commands::git;
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

#[tauri::command]
#[specta::specta]
pub fn update_settings(settings: SettingsDto) -> Result<SettingsDto, IpcError> {
    let mut current = settings::load();
    current.theme = settings.theme;
    settings::save(&current).map_err(|err| IpcError::Settings(err.to_string()))?;
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
            git::repo_open,
            git::repo_status,
            git::conflict_load,
            git::conflict_analyze,
            git::conflict_save,
            git::conflict_accept_side,
            git::conflict_restore,
            git::op_continue,
            git::op_abort,
            git::op_skip,
        ])
        .events(tauri_specta::collect_events![git::RepoChanged])
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
