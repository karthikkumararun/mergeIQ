//! Locating and running the `git` executable.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::error::{GitError, Result};

/// A parsed `git --version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Patch version (0 if absent).
    pub patch: u32,
}

/// Oldest supported git.
pub const MIN_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 30,
    patch: 0,
};

/// Parses output like `git version 2.39.2`, `git version 2.54.0 (Apple Git-157)` or
/// `git version 2.45.1.windows.1`.
pub fn parse_version(text: &str) -> Option<GitVersion> {
    let token = text
        .trim()
        .strip_prefix("git version ")?
        .split_whitespace()
        .next()?;
    let mut parts = token.split('.').map(|p| {
        let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u32>().ok()
    });
    let major = parts.next()??;
    let minor = parts.next()??;
    let patch = parts.next().flatten().unwrap_or(0);
    Some(GitVersion {
        major,
        minor,
        patch,
    })
}

/// A verified git executable. Every invocation uses an argument array (no shell).
#[derive(Debug, Clone)]
pub struct GitExec {
    path: PathBuf,
    version: GitVersion,
    env: Vec<(OsString, OsString)>,
}

impl GitExec {
    /// Uses `configured` if given, otherwise searches `PATH`.
    pub fn locate(configured: Option<&Path>) -> Result<Self> {
        Self::locate_in(configured, None)
    }

    /// Like [`GitExec::locate`] but searches `search_path` instead of `PATH` (for tests).
    pub fn locate_in(configured: Option<&Path>, search_path: Option<&OsStr>) -> Result<Self> {
        let path = match configured.filter(|p| !p.as_os_str().is_empty()) {
            Some(path) => path.to_path_buf(),
            None => {
                let cwd = std::env::current_dir()?;
                let found = match search_path {
                    Some(paths) => which::which_in("git", Some(paths), cwd),
                    None => which::which("git"),
                };
                found.map_err(|_| GitError::GitNotFound)?
            }
        };
        Self::with_binary(path)
    }

    /// Verifies the binary at `path` and checks its version.
    pub fn with_binary(path: PathBuf) -> Result<Self> {
        let output = Command::new(&path)
            .arg("--version")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .output()
            .map_err(|_| GitError::GitNotFound)?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let Some(version) = parse_version(&text).filter(|_| output.status.success()) else {
            return Err(GitError::GitTooOld {
                found: text.trim().to_string(),
            });
        };
        if version < MIN_VERSION {
            return Err(GitError::GitTooOld {
                found: format!("{}.{}.{}", version.major, version.minor, version.patch),
            });
        }
        Ok(Self {
            path,
            version,
            env: Vec::new(),
        })
    }

    /// Sets an extra environment variable for every invocation (e.g. `GIT_CONFIG_GLOBAL`
    /// to point git at a scratch config).
    #[must_use]
    pub fn with_env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// The detected version.
    pub fn version(&self) -> GitVersion {
        self.version
    }

    /// Path of the executable.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn command<I, S>(&self, cwd: &Path, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new(&self.path);
        cmd.current_dir(cwd)
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_LITERAL_PATHSPECS", "1")
            .env("GIT_EDITOR", "true")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        cmd
    }

    /// Runs git and returns its raw output regardless of exit status.
    pub fn run<I, S>(&self, cwd: &Path, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Ok(self.command(cwd, args).output()?)
    }

    /// Runs git and returns stdout, or [`GitError::CommandFailed`] on non-zero exit.
    pub fn run_ok<I, S>(&self, cwd: &Path, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<OsString> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
        let output = self.run(cwd, &args)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(command_failed(&args, &output))
        }
    }
}

/// Builds a [`GitError::CommandFailed`] from a failed invocation.
pub fn command_failed(args: &[OsString], output: &Output) -> GitError {
    // Skip leading `-c key=value` pairs so the subcommand is reported.
    let mut iter = args.iter();
    let mut command = String::new();
    while let Some(arg) = iter.next() {
        if arg == "-c" {
            iter.next();
            continue;
        }
        command = arg.to_string_lossy().into_owned();
        break;
    }
    let mut stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        stderr = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }
    GitError::CommandFailed { command, stderr }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        let v = parse_version("git version 2.54.0 (Apple Git-157)\n").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (2, 54, 0));
        let v = parse_version("git version 2.45.1.windows.1").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (2, 45, 1));
        assert!(parse_version("hello").is_none());
    }

    #[test]
    fn git_missing() {
        let empty = tempfile::tempdir().unwrap();
        let err = GitExec::locate_in(None, Some(empty.path().as_os_str())).unwrap_err();
        assert!(matches!(err, GitError::GitNotFound));
        let err = GitExec::locate(Some(&empty.path().join("nope"))).unwrap_err();
        assert!(matches!(err, GitError::GitNotFound));
    }

    #[cfg(unix)]
    fn fake_git(dir: &Path, version_line: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-git");
        std::fs::write(&path, format!("#!/bin/sh\necho '{version_line}'\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn git_too_old() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_git(dir.path(), "git version 2.29.3");
        let err = GitExec::locate(Some(&fake)).unwrap_err();
        match err {
            GitError::GitTooOld { found } => assert_eq!(found, "2.29.3"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn configured_path_is_used_when_new_enough() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_git(dir.path(), "git version 2.30.0");
        let exec = GitExec::locate(Some(&fake)).unwrap();
        assert_eq!(exec.path(), fake);
        assert_eq!(exec.version().minor, 30);
    }
}
