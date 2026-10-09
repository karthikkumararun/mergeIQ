//! The `git config --global` commands that register MergeIQ as `git mergetool`.

use mergeiq_git::GitExec;

/// The tool name registered in git's config.
pub const TOOL: &str = "mergeiq";

/// One `git config --global <key> <value>` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigCommand {
    pub key: String,
    pub value: String,
}

impl ConfigCommand {
    fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    /// The command as a user would type it (value single-quoted when needed).
    pub fn display(&self) -> String {
        let plain = !self.value.is_empty()
            && self
                .value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c));
        let value = if plain {
            self.value.clone()
        } else {
            format!("'{}'", self.value.replace('\'', "'\\''"))
        };
        format!("git config --global {} {}", self.key, value)
    }
}

/// The commands to run. `no_backup` adds `mergetool.keepBackup false` (no `.orig` files).
pub fn mergetool_commands(no_backup: bool) -> Vec<ConfigCommand> {
    let mut commands = vec![
        ConfigCommand::new("merge.tool", TOOL),
        ConfigCommand::new(
            "mergetool.mergeiq.cmd",
            r#"mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED""#,
        ),
        ConfigCommand::new("mergetool.mergeiq.trustExitCode", "true"),
    ];
    if no_backup {
        commands.push(ConfigCommand::new("mergetool.keepBackup", "false"));
    }
    commands
}

/// Runs `commands` with argument arrays (no shell), stopping at the first failure.
pub fn apply(exec: &GitExec, commands: &[ConfigCommand]) -> Result<(), String> {
    let cwd = std::env::temp_dir();
    for command in commands {
        exec.run_ok(&cwd, ["config", "--global", &command.key, &command.value])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// True when global git config already registers MergeIQ as the merge tool.
pub fn is_configured(exec: &GitExec) -> bool {
    let get = |key: &str| {
        exec.run_ok(&std::env::temp_dir(), ["config", "--global", "--get", key])
            .ok()
            .map(|out| String::from_utf8_lossy(&out).trim().to_string())
    };
    get("merge.tool").as_deref() == Some(TOOL) && get("mergetool.mergeiq.cmd").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_exec(dir: &std::path::Path) -> GitExec {
        GitExec::locate(None)
            .expect("git on PATH")
            .with_env("GIT_CONFIG_GLOBAL", dir.join("gitconfig"))
            .with_env("GIT_CONFIG_NOSYSTEM", "1")
    }

    #[test]
    fn commands_display_exactly_what_the_ui_shows() {
        let shown: Vec<String> = mergetool_commands(true)
            .iter()
            .map(ConfigCommand::display)
            .collect();
        assert_eq!(
            shown,
            [
                "git config --global merge.tool mergeiq",
                r#"git config --global mergetool.mergeiq.cmd 'mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED"'"#,
                "git config --global mergetool.mergeiq.trustExitCode true",
                "git config --global mergetool.keepBackup false",
            ]
        );
        assert_eq!(mergetool_commands(false).len(), 3);
    }

    #[test]
    fn confirmed_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let exec = scratch_exec(dir.path());
        assert!(!is_configured(&exec));
        apply(&exec, &mergetool_commands(true)).unwrap();

        let get = |key: &str| {
            String::from_utf8(
                exec.run_ok(dir.path(), ["config", "--global", key])
                    .unwrap(),
            )
            .unwrap()
            .trim()
            .to_string()
        };
        assert_eq!(get("merge.tool"), "mergeiq");
        assert_eq!(
            get("mergetool.mergeiq.cmd"),
            r#"mergeiq merge "$BASE" "$LOCAL" "$REMOTE" "$MERGED""#
        );
        assert_eq!(get("mergetool.mergeiq.trustExitCode"), "true");
        assert_eq!(get("mergetool.keepBackup"), "false");
        assert!(is_configured(&exec));
    }

    #[test]
    fn nothing_is_written_until_apply_runs() {
        let dir = tempfile::tempdir().unwrap();
        let exec = scratch_exec(dir.path());
        let _ = mergetool_commands(true);
        assert!(!dir.path().join("gitconfig").exists());
        assert!(!is_configured(&exec));
    }
}
