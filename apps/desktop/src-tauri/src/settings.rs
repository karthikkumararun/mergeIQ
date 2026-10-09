use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_theme() -> String {
    "system".to_string()
}

fn default_true() -> bool {
    true
}

/// Merge editor preferences (`merge-editor-ui`), stored under `mergeEditor` in the
/// settings file and exposed over IPC as-is (every field has a default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct MergeEditorSettings {
    /// Apply non-conflicting chunks automatically when a file opens.
    pub auto_apply_non_conflicting: bool,
    /// Show the read-only base pane.
    pub show_base: bool,
    /// Fold long unchanged regions.
    pub collapse_unchanged: bool,
    /// Keep the panes scroll-aligned.
    #[serde(default = "default_true")]
    pub sync_scroll: bool,
    /// Whitespace policy used when analysing a file.
    pub whitespace_policy: mergeiq_core::WhitespacePolicy,
    /// In a repository window, open the next unresolved file after a resolved save.
    #[serde(default = "default_true")]
    pub auto_advance_after_save: bool,
}

impl Default for MergeEditorSettings {
    fn default() -> Self {
        Self {
            auto_apply_non_conflicting: false,
            show_base: false,
            collapse_unchanged: false,
            sync_scroll: true,
            whitespace_policy: mergeiq_core::WhitespacePolicy::Exact,
            auto_advance_after_save: true,
        }
    }
}

/// On-disk settings model. Not exported via specta: `extra` holds unknown keys as
/// arbitrary JSON for forward-compatibility, which specta's TypeScript exporter can't
/// type losslessly. IPC uses [`crate::ipc::SettingsDto`] instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// `"system"` | `"light"` | `"dark"`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Merge editor preferences.
    #[serde(default)]
    pub merge_editor: MergeEditorSettings,
    /// Recently opened repositories, most recent first.
    #[serde(default)]
    pub recent_repos: Vec<crate::recents::RecentRepo>,
    /// Lockfile regeneration commands the user customised, by lockfile kind (`"Pnpm"`, ...).
    #[serde(default)]
    pub lockfile_commands: BTreeMap<String, String>,
    /// AI assistance settings (never contains credentials).
    #[serde(default)]
    pub ai: mergeiq_ai::service::AiSettings,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            merge_editor: MergeEditorSettings::default(),
            recent_repos: Vec::new(),
            lockfile_commands: BTreeMap::new(),
            ai: mergeiq_ai::service::AiSettings::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("could not determine config directory")]
    NoConfigDir,
    #[error("failed to serialize settings: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// `<OS config dir>/MergeIQ/settings.json`.
pub fn settings_path() -> Result<PathBuf, SettingsError> {
    let base = directories::BaseDirs::new().ok_or(SettingsError::NoConfigDir)?;
    Ok(base.config_dir().join("MergeIQ").join("settings.json"))
}

pub fn load() -> Settings {
    match settings_path() {
        Ok(path) => load_from(&path),
        Err(err) => {
            tracing::warn!(error = %err, "could not determine settings path, using defaults");
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) -> Result<(), SettingsError> {
    let path = settings_path()?;
    save_to(&path, settings)
}

/// Loads settings from `path`, creating it with defaults if missing, and backing up
/// then replacing a corrupt file with defaults.
fn load_from(path: &Path) -> Settings {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => {
            let defaults = Settings::default();
            if let Err(err) = save_to(path, &defaults) {
                tracing::warn!(error = %err, "failed to write default settings file");
            }
            return defaults;
        }
    };

    match serde_json::from_str::<Settings>(&content) {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!(error = %err, path = %path.display(), "settings file corrupt; backing up and resetting to defaults");
            let backup = path.with_extension("json.bak");
            if let Err(err) = fs::rename(path, &backup) {
                tracing::warn!(error = %err, "failed to back up corrupt settings file");
            }
            let defaults = Settings::default();
            if let Err(err) = save_to(path, &defaults) {
                tracing::warn!(error = %err, "failed to write default settings file");
            }
            defaults
        }
    }
}

/// Atomic write: write to a temp file in the same directory, then rename.
fn save_to(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let data = serde_json::to_string_pretty(settings)?;
    fs::write(&tmp_path, data)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_creates_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let settings = load_from(&path);

        assert_eq!(settings, Settings::default());
        assert!(path.exists());
    }

    #[test]
    fn corrupt_file_is_backed_up_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not valid json").unwrap();

        let settings = load_from(&path);

        assert_eq!(settings, Settings::default());
        let backup = path.with_extension("json.bak");
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), "{ not valid json");
        assert_eq!(
            serde_json::from_str::<Settings>(&fs::read_to_string(&path).unwrap()).unwrap(),
            Settings::default()
        );
    }

    #[test]
    fn unknown_keys_are_preserved_across_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark","futureFeature":true}"#).unwrap();

        let settings = load_from(&path);

        assert_eq!(settings.theme, "dark");
        assert_eq!(
            settings.extra.get("futureFeature"),
            Some(&Value::Bool(true))
        );

        save_to(&path, &settings).unwrap();
        let reloaded = load_from(&path);
        assert_eq!(reloaded, settings);
    }

    #[test]
    fn recent_repos_persist_and_default_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
        let mut settings = load_from(&path);
        assert!(settings.recent_repos.is_empty());
        assert!(settings.merge_editor.auto_advance_after_save);
        crate::recents::push(&mut settings.recent_repos, "/code/a", 5);
        save_to(&path, &settings).unwrap();
        assert_eq!(load_from(&path).recent_repos.len(), 1);
    }

    #[test]
    fn merge_editor_settings_default_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();

        let settings = load_from(&path);

        assert_eq!(settings.merge_editor, MergeEditorSettings::default());
        assert!(settings.merge_editor.sync_scroll);
        assert!(!settings.merge_editor.auto_apply_non_conflicting);
    }

    #[test]
    fn merge_editor_settings_persist_across_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark","futureFeature":1}"#).unwrap();

        let mut settings = load_from(&path);
        settings.merge_editor.show_base = true;
        settings.merge_editor.whitespace_policy = mergeiq_core::WhitespacePolicy::IgnoreAll;
        save_to(&path, &settings).unwrap();

        let reloaded = load_from(&path);
        assert!(reloaded.merge_editor.show_base);
        assert_eq!(
            reloaded.merge_editor.whitespace_policy,
            mergeiq_core::WhitespacePolicy::IgnoreAll
        );
        assert_eq!(reloaded.extra.get("futureFeature"), Some(&Value::from(1)));
    }

    #[test]
    fn ai_settings_persist_and_a_stored_key_never_reaches_the_file() {
        use mergeiq_ai::privacy::RepoDecision;
        use mergeiq_ai::secrets::{MemoryStore, SecretStore};
        use mergeiq_ai::ProviderKind;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let key = "sk-ant-api03-very-secret-key-a9F2";
        let store = MemoryStore::default();
        store.set(ProviderKind::Anthropic.id(), key).unwrap();

        let mut settings = load_from(&path);
        settings.ai.confirmed = true;
        settings.ai.notice_accepted = true;
        settings.ai.provider = ProviderKind::Ollama;
        settings
            .ai
            .privacy
            .repos
            .insert("/code/shop-web".into(), RepoDecision::Allowed);
        settings.ai.privacy.exclude_globs.push("vendor/**".into());
        settings.ai.context.surrounding_lines = 12;
        save_to(&path, &settings).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains(key) && !text.contains("very-secret") && !text.contains("a9F2"));
        assert!(!text.to_lowercase().contains("apikey") && !text.contains("api_key"));
        let reloaded = load_from(&path);
        assert_eq!(reloaded.ai, settings.ai);
        assert_eq!(reloaded.ai.provider, ProviderKind::Ollama);
        assert_eq!(reloaded.ai.context.surrounding_lines, 12);
        assert_eq!(
            reloaded.ai.privacy.repos["/code/shop-web"],
            RepoDecision::Allowed
        );
        // A settings file from before AI existed still loads with the defaults.
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(
            load_from(&path).ai,
            mergeiq_ai::service::AiSettings::default()
        );
    }
}
