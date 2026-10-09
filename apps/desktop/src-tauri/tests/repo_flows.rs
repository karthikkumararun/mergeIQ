#![cfg(unix)]
//! Repository-window flows end to end at the backend: a scripted repository, opened through
//! the same `RepoRegistry` the app uses, resolved with the same service calls the IPC
//! commands make, then continued. (The webview itself is covered by the Playwright suite.)

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use mergeiq_desktop_lib::repo_service::accept_many;
use mergeiq_desktop_lib::repos::RepoRegistry;
use mergeiq_git::{AcceptSide, ConflictType, GitExec, Operation, PathToken, Repo, RepoPath};

struct Scripted {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Scripted {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let s = Self { _dir: dir, root };
        s.git(&["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "Test User"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", "no-hooks"),
            ("merge.conflictStyle", "merge"),
        ] {
            s.git(&["config", k, v]);
        }
        s
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(&self.root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_EDITOR", "true")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        assert!(
            out.status.success()
                || args
                    .first()
                    .is_some_and(|a| ["merge", "rebase"].contains(a)),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn commit(&self, files: &[(&str, &str)], message: &str) {
        for (path, text) in files {
            let full = self.root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, text).unwrap();
        }
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    fn open(&self) -> Repo {
        let registry = RepoRegistry::default();
        let exec = GitExec::locate(None).unwrap();
        let opened = registry
            .open(exec, &self.root, |_, repo| repo.watch(|| {}))
            .unwrap();
        registry.get(opened.id).unwrap()
    }
}

fn token(path: &str) -> PathToken {
    RepoPath::from_bytes(path.as_bytes().to_vec()).token()
}

fn displays(repo: &Repo) -> Vec<String> {
    repo.list_conflicts()
        .unwrap()
        .into_iter()
        .map(|c| c.display)
        .collect()
}

/// main and feature both edit three files.
fn merge_with_three_conflicts(s: &Scripted) {
    s.commit(
        &[
            ("a.txt", "1\n2\n3\n"),
            ("b.txt", "1\n2\n3\n"),
            ("web/yarn.lock", "1\n2\n3\n"),
        ],
        "base",
    );
    s.git(&["checkout", "-q", "-b", "feature"]);
    s.commit(
        &[
            ("a.txt", "1\nTHEIRS\n3\n"),
            ("b.txt", "1\nTHEIRS\n3\n"),
            ("web/yarn.lock", "1\nTHEIRS\n3\n"),
        ],
        "feature change",
    );
    s.git(&["checkout", "-q", "main"]);
    s.commit(
        &[
            ("a.txt", "1\nOURS\n3\n"),
            ("b.txt", "1\nOURS\n3\n"),
            ("web/yarn.lock", "1\nOURS\n3\n"),
        ],
        "main change",
    );
    s.git(&["merge", "feature"]);
}

#[test]
fn merge_with_three_conflicts_resolve_batch_accept_continue() {
    let s = Scripted::new();
    merge_with_three_conflicts(&s);
    let repo = s.open();
    let status = repo.status().unwrap();
    assert_eq!(status.operation, Operation::Merge);
    assert_eq!(status.branch.as_deref(), Some("main"));
    assert_eq!(status.conflicts.len(), 3);
    assert!(status
        .conflicts
        .iter()
        .all(|c| c.conflict_type == ConflictType::BothModified));

    // Resolve one through the editor path (save + stage).
    repo.save_resolved(&RepoPath::from_bytes(b"a.txt".to_vec()), b"1\nBOTH\n3\n")
        .unwrap();
    assert_eq!(displays(&repo), ["b.txt", "web/yarn.lock"]);
    // Continue is blocked while conflicts remain.
    assert!(matches!(
        repo.op_continue().unwrap_err(),
        mergeiq_git::GitError::UnresolvedPaths { count: 2 }
    ));

    // Batch accept the generated file and the other one from the right side.
    let result = accept_many(
        &repo,
        &[token("b.txt"), token("web/yarn.lock")],
        AcceptSide::Theirs,
    );
    assert_eq!(result.done.len(), 2);
    assert!(result.failed.is_empty());
    assert_eq!(
        std::fs::read_to_string(s.root.join("b.txt")).unwrap(),
        "1\nTHEIRS\n3\n"
    );
    assert!(repo.list_conflicts().unwrap().is_empty());

    let outcome = repo.op_continue().unwrap();
    assert!(outcome.finished, "{}", outcome.message);
    assert_eq!(repo.status().unwrap().operation, Operation::None);
    // A merge commit with two parents exists and holds the resolutions.
    assert_eq!(
        s.git(&["rev-list", "--parents", "-1", "HEAD"])
            .split(' ')
            .count(),
        3
    );
    assert_eq!(s.git(&["show", "HEAD:a.txt"]), "1\nBOTH\n3");
    assert_eq!(s.git(&["show", "HEAD:web/yarn.lock"]), "1\nTHEIRS\n3");
}

#[test]
fn batch_accept_reports_a_failing_path_and_still_applies_the_rest() {
    let s = Scripted::new();
    merge_with_three_conflicts(&s);
    let repo = s.open();
    let result = accept_many(
        &repo,
        &[token("a.txt"), token("not-a-conflict.txt"), token("b.txt")],
        AcceptSide::Ours,
    );
    assert_eq!(result.done, [token("a.txt"), token("b.txt")]);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].path, token("not-a-conflict.txt"));
    assert_eq!(displays(&repo), ["web/yarn.lock"]);
}

#[test]
fn rebase_with_two_conflicting_steps_progresses_and_completes() {
    let s = Scripted::new();
    s.commit(&[("a.txt", "1\n2\n3\n"), ("b.txt", "1\n2\n3\n")], "base");
    s.git(&["checkout", "-q", "-b", "feature"]);
    s.commit(&[("a.txt", "1\nF1\n3\n")], "feature one");
    s.commit(&[("b.txt", "1\nF2\n3\n")], "feature two");
    s.git(&["checkout", "-q", "main"]);
    s.commit(
        &[("a.txt", "1\nM1\n3\n"), ("b.txt", "1\nM2\n3\n")],
        "main change",
    );
    s.git(&["checkout", "-q", "feature"]);
    s.git(&["rebase", "main"]);

    let repo = s.open();
    let first = repo.status().unwrap();
    assert!(matches!(
        first.operation,
        Operation::Rebase {
            step: 1,
            total: 2,
            ..
        }
    ));
    assert_eq!(first.branch, None);
    assert_eq!(first.labels.theirs.subject.as_deref(), Some("feature one"));
    assert_eq!(displays(&repo), ["a.txt"]);

    repo.save_resolved(&RepoPath::from_bytes(b"a.txt".to_vec()), b"1\nR1\n3\n")
        .unwrap();
    // Continue stops at the next commit, which also conflicts.
    let stopped = repo.op_continue().unwrap();
    assert!(!stopped.finished, "{}", stopped.message);
    let second = repo.status().unwrap();
    assert!(matches!(
        second.operation,
        Operation::Rebase {
            step: 2,
            total: 2,
            ..
        }
    ));
    assert_eq!(second.labels.theirs.subject.as_deref(), Some("feature two"));
    assert_eq!(displays(&repo), ["b.txt"]);

    let result = accept_many(&repo, &[token("b.txt")], AcceptSide::Theirs);
    assert_eq!(result.done.len(), 1);
    let done = repo.op_continue().unwrap();
    assert!(done.finished, "{}", done.message);
    let after = repo.status().unwrap();
    assert_eq!(after.operation, Operation::None);
    assert_eq!(after.branch.as_deref(), Some("feature"));
    assert_eq!(
        s.git(&["log", "--format=%s", "-2"]),
        "feature two\nfeature one"
    );
    assert_eq!(
        s.git(&["rev-parse", "HEAD~2"]),
        s.git(&["rev-parse", "main"])
    );
}

#[test]
fn reopen_restores_the_original_stages() {
    let s = Scripted::new();
    merge_with_three_conflicts(&s);
    let repo = s.open();
    let before = repo.list_conflicts().unwrap();
    let path = RepoPath::from_bytes(b"a.txt".to_vec());
    repo.save_resolved(&path, b"resolved\n").unwrap();
    assert_eq!(repo.list_conflicts().unwrap().len(), 2);
    repo.restore_conflict(&path).unwrap();
    let after = repo.list_conflicts().unwrap();
    assert_eq!(after, before);
}

#[test]
fn external_resolution_reaches_the_watcher_and_the_list() {
    let s = Scripted::new();
    merge_with_three_conflicts(&s);
    let registry = RepoRegistry::default();
    let (tx, rx) = mpsc::channel();
    let opened = registry
        .open(GitExec::locate(None).unwrap(), &s.root, move |_, repo| {
            repo.watch(move || {
                let _ = tx.send(());
            })
        })
        .unwrap();
    let repo = registry.get(opened.id).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    while rx.try_recv().is_ok() {}

    s.git(&["checkout", "--theirs", "a.txt"]);
    s.git(&["add", "a.txt"]);
    rx.recv_timeout(Duration::from_secs(1))
        .expect("repo-changed within 1s");
    assert_eq!(displays(&repo), ["b.txt", "web/yarn.lock"]);
}

#[test]
fn modify_delete_is_listed_and_accept_right_deletes() {
    let s = Scripted::new();
    s.commit(&[("a.txt", "1\n2\n"), ("keep.txt", "k\n")], "base");
    s.git(&["checkout", "-q", "-b", "feature"]);
    s.git(&["rm", "-q", "a.txt"]);
    s.git(&["commit", "-q", "-m", "remove a"]);
    s.git(&["checkout", "-q", "main"]);
    s.commit(&[("a.txt", "1\nTWO\n")], "change a");
    s.git(&["merge", "feature"]);
    let repo = s.open();
    let conflicts = repo.list_conflicts().unwrap();
    assert_eq!(conflicts[0].conflict_type, ConflictType::DeletedByThem);
    let result = accept_many(&repo, &[conflicts[0].path.clone()], AcceptSide::Theirs);
    assert_eq!(result.done.len(), 1);
    assert!(!s.root.join("a.txt").exists());
}
