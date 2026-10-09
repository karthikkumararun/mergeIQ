//! The set of open repositories: canonical worktree root → id (and so window).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mergeiq_git::{GitError, GitExec, Repo, RepoWatcher};

/// An open repository and the watcher feeding its window.
pub struct RepoEntry {
    pub id: u32,
    pub repo: Repo,
    _watcher: RepoWatcher,
}

/// Result of [`RepoRegistry::open`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opened {
    pub id: u32,
    /// `false` when the repository was already open (its window should be focused).
    pub created: bool,
}

#[derive(Default)]
struct Inner {
    next: u32,
    by_root: HashMap<PathBuf, u32>,
    by_id: HashMap<u32, Arc<RepoEntry>>,
}

/// One entry per repository, however it was reached (nested path, `open` request, recents).
#[derive(Default)]
pub struct RepoRegistry {
    inner: Mutex<Inner>,
}

impl RepoRegistry {
    /// Opens the repository containing `path`, or finds it if already open. `watch` creates
    /// the change watcher for a newly opened repository (it receives the new id).
    pub fn open(
        &self,
        exec: GitExec,
        path: &Path,
        watch: impl FnOnce(u32, &Repo) -> Result<RepoWatcher, GitError>,
    ) -> Result<Opened, GitError> {
        let repo = Repo::open(exec, path)?;
        let key = dunce::canonicalize(repo.root()).unwrap_or_else(|_| repo.root().to_path_buf());
        let mut inner = self.inner.lock().expect("repo registry lock");
        if let Some(&id) = inner.by_root.get(&key) {
            return Ok(Opened { id, created: false });
        }
        inner.next += 1;
        let id = inner.next;
        let watcher = watch(id, &repo)?;
        inner.by_root.insert(key, id);
        inner.by_id.insert(
            id,
            Arc::new(RepoEntry {
                id,
                repo,
                _watcher: watcher,
            }),
        );
        Ok(Opened { id, created: true })
    }

    /// The repository for window `id`.
    pub fn get(&self, id: u32) -> Option<Repo> {
        self.inner
            .lock()
            .expect("repo registry lock")
            .by_id
            .get(&id)
            .map(|e| e.repo.clone())
    }

    /// Forgets `id` (its window closed); dropping the entry stops its watcher.
    pub fn remove(&self, id: u32) -> bool {
        let mut inner = self.inner.lock().expect("repo registry lock");
        let removed = inner.by_id.remove(&id);
        inner.by_root.retain(|_, v| *v != id);
        removed.is_some()
    }

    /// Number of open repositories.
    pub fn len(&self) -> usize {
        self.inner.lock().expect("repo registry lock").by_id.len()
    }

    /// True when no repository is open.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Shared handle used as Tauri managed state.
pub type Repos = Arc<RepoRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    fn git_repo(dir: &Path) {
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .current_dir(dir)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success());
        };
        run(&["init", "-q", "-b", "main"]);
    }

    fn open(reg: &RepoRegistry, path: &Path) -> Result<Opened, GitError> {
        let exec = GitExec::locate(None).unwrap();
        reg.open(exec, path, |_, repo| repo.watch(|| {}))
    }

    #[test]
    fn open_via_cli_focuses_the_existing_window() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        git_repo(&root);
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        let reg = RepoRegistry::default();
        let first = open(&reg, &root).unwrap();
        assert!(first.created);
        // A nested path of the same repository is the same repository.
        let second = open(&reg, &root.join("src/deep")).unwrap();
        assert_eq!(
            second,
            Opened {
                id: first.id,
                created: false
            }
        );
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn different_repositories_get_different_ids_and_removal_forgets_them() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        git_repo(a.path());
        git_repo(b.path());
        let reg = RepoRegistry::default();
        let ia = open(&reg, a.path()).unwrap().id;
        let ib = open(&reg, b.path()).unwrap().id;
        assert_ne!(ia, ib);
        assert!(reg.get(ia).is_some());
        assert!(reg.remove(ia));
        assert!(reg.get(ia).is_none());
        // Reopening after the window closed creates a fresh entry.
        assert!(open(&reg, a.path()).unwrap().created);
    }

    #[test]
    fn not_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = RepoRegistry::default();
        let err = open(&reg, tmp.path()).unwrap_err();
        assert!(matches!(err, GitError::NotARepo { .. }), "{err:?}");
        assert_eq!(reg.len(), 0);
    }
}
