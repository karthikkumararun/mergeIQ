//! Opening a repository and running git inside it.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::classify::BlobFacts;
use crate::error::{GitError, Result};
use crate::exec::GitExec;
use crate::renames::Rename;

/// Renames per `(ours, theirs)` commit ids.
pub(crate) type RenamesCache = Arc<Mutex<HashMap<(String, String), Arc<Vec<Rename>>>>>;

/// How long watcher events are suppressed after our own mutations.
pub const SELF_WRITE_QUIET: Duration = Duration::from_millis(500);

/// An opened git working tree.
#[derive(Debug, Clone)]
pub struct Repo {
    pub(crate) exec: GitExec,
    pub(crate) root: PathBuf,
    pub(crate) git_dir: PathBuf,
    pub(crate) common_dir: PathBuf,
    pub(crate) quiet_until: Arc<Mutex<Option<Instant>>>,
    /// Blob facts by object id (content-addressed, so never stale).
    pub(crate) facts_cache: Arc<Mutex<HashMap<String, Arc<BlobFacts>>>>,
    /// Renames for the current operation head: `(ours, theirs)` commit ids.
    pub(crate) renames_cache: RenamesCache,
}

impl Repo {
    /// Opens the working tree containing `path` (any depth, worktree or submodule).
    /// Bare repositories are rejected with [`GitError::Bare`].
    pub fn open(exec: GitExec, path: &Path) -> Result<Self> {
        let not_a_repo = || GitError::NotARepo {
            path: path.display().to_string(),
        };
        let bare = exec.run(path, ["rev-parse", "--is-bare-repository"])?;
        if !bare.status.success() {
            return Err(not_a_repo());
        }
        if String::from_utf8_lossy(&bare.stdout).trim() == "true" {
            return Err(GitError::Bare);
        }
        let out = exec.run(
            path,
            [
                "rev-parse",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
            ],
        )?;
        if !out.status.success() {
            return Err(not_a_repo());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        let (Some(root), Some(git_dir), Some(common)) = (lines.next(), lines.next(), lines.next())
        else {
            return Err(not_a_repo());
        };
        let cwd = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let common_dir = {
            let p = PathBuf::from(common);
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        };
        Ok(Self {
            exec,
            root: PathBuf::from(root),
            git_dir: PathBuf::from(git_dir),
            common_dir,
            quiet_until: Arc::new(Mutex::new(None)),
            facts_cache: Arc::new(Mutex::new(HashMap::new())),
            renames_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Worktree root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// This worktree's private git dir (`.git`, or `.git/worktrees/<name>`).
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// The shared git dir (same as [`Repo::git_dir`] except in linked worktrees).
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// The git executable in use.
    pub fn exec(&self) -> &GitExec {
        &self.exec
    }

    /// Runs git at the worktree root, returning stdout on success.
    pub(crate) fn git<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec.run_ok(&self.root, args)
    }

    /// Runs git at the worktree root with `input` on stdin.
    pub(crate) fn git_with_stdin<I, S>(&self, args: I, input: &[u8]) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        use std::io::Write;
        let args: Vec<std::ffi::OsString> =
            args.into_iter().map(|a| a.as_ref().to_owned()).collect();
        let mut child = self
            .exec
            .command(&self.root, &args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input)?;
        }
        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(crate::exec::command_failed(&args, &output))
        }
    }

    /// Runs git and returns trimmed UTF-8 stdout, or `None` when git exits non-zero or prints nothing.
    pub(crate) fn git_text_opt<I, S>(&self, args: I) -> Option<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let out = self.exec.run(&self.root, args).ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    /// Records that we are mutating, so the watcher ignores the resulting events.
    pub(crate) fn note_mutation(&self) {
        if let Ok(mut guard) = self.quiet_until.lock() {
            *guard = Some(Instant::now() + SELF_WRITE_QUIET);
        }
    }

    pub(crate) fn is_quiet(&self) -> bool {
        self.quiet_until
            .lock()
            .ok()
            .and_then(|g| *g)
            .is_some_and(|until| Instant::now() < until)
    }
}
