use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_theme() -> String {
    "system".to_string()
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
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
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
}
