//! Debounced change notifications for the index and operation state files.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{GitError, Result};
use crate::repo::Repo;

/// Quiet period after the last filesystem event before notifying.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

const STATE_NAMES: &[&str] = &[
    "index",
    "HEAD",
    "MERGE_HEAD",
    "MERGE_MSG",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "REBASE_HEAD",
    "rebase-merge",
    "rebase-apply",
];

impl From<notify::Error> for GitError {
    fn from(err: notify::Error) -> Self {
        GitError::Io {
            message: err.to_string(),
        }
    }
}

fn is_relevant(path: &Path, git_dir: &Path) -> bool {
    let rel = path.strip_prefix(git_dir).unwrap_or(path);
    rel.components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .is_some_and(|name| STATE_NAMES.contains(&name))
}

/// Keeps a watch alive; dropping it stops notifications.
pub struct RepoWatcher {
    _watcher: RecommendedWatcher,
}

impl Repo {
    /// Calls `on_change` (debounced 250 ms) when the index or operation state files
    /// change. Changes caused by this adapter's own mutations are suppressed.
    pub fn watch<F>(&self, on_change: F) -> Result<RepoWatcher>
    where
        F: Fn() + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<()>();
        let git_dir = self.git_dir.clone();
        let canonical = dunce::canonicalize(&git_dir).unwrap_or_else(|_| git_dir.clone());
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    if event
                        .paths
                        .iter()
                        .any(|p| is_relevant(p, &git_dir) || is_relevant(p, &canonical))
                    {
                        let _ = tx.send(());
                    }
                }
            })?;
        watcher.watch(&self.git_dir, RecursiveMode::Recursive)?;

        let repo = self.clone();
        std::thread::spawn(move || {
            while rx.recv().is_ok() {
                loop {
                    match rx.recv_timeout(DEBOUNCE) {
                        Ok(()) => continue,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if !repo.is_quiet() {
                    on_change();
                }
            }
        });
        Ok(RepoWatcher { _watcher: watcher })
    }
}
