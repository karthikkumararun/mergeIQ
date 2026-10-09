//! Scripts throwaway git repositories (via the real `git` CLI) for integration tests.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use mergeiq_git::{GitExec, Repo};
use tempfile::TempDir;

pub struct Scenario {
    _dir: TempDir,
    pub root: PathBuf,
}

impl Scenario {
    /// New repo on branch `main` with an isolated local config.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap().join("repo");
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
            ("rerere.enabled", "false"),
            ("advice.detachedHead", "false"),
        ] {
            s.git(&["config", k, v]);
        }
        s
    }

    pub fn exec(&self) -> GitExec {
        GitExec::locate(None).expect("git on PATH")
    }

    pub fn repo(&self) -> Repo {
        Repo::open(self.exec(), &self.root).unwrap()
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(cwd)
            .args(args)
            .env("GIT_EDITOR", "true")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("LC_ALL", "C");
        cmd
    }

    /// Runs git in the repo root; panics on failure; returns trimmed stdout.
    pub fn git(&self, args: &[&str]) -> String {
        self.git_in(&self.root, args)
    }

    pub fn git_in(&self, cwd: &Path, args: &[&str]) -> String {
        let out = self.command(cwd, args).output().unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Runs git, returning the raw output regardless of status.
    pub fn git_raw(&self, args: &[&str]) -> Output {
        self.command(&self.root, args).output().unwrap()
    }

    pub fn write(&self, rel: &str, contents: impl AsRef<[u8]>) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    pub fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.path(rel)).unwrap()
    }

    /// Stages everything and commits; returns the new commit sha.
    pub fn commit_all(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn write_commit(&self, rel: &str, contents: &str, message: &str) -> String {
        self.write(rel, contents);
        self.commit_all(message)
    }

    pub fn checkout(&self, branch: &str) {
        self.git(&["checkout", "-q", branch]);
    }

    pub fn checkout_new(&self, branch: &str) {
        self.git(&["checkout", "-q", "-b", branch]);
    }

    /// `main` gets `ours`, `feature` gets `theirs`, both from a common `base`, then
    /// `main` is checked out. Single file `a.txt`.
    pub fn diverge(&self, base: &str, ours: &str, theirs: &str) {
        self.write_commit("a.txt", base, "base");
        self.checkout_new("feature");
        self.write_commit("a.txt", theirs, "feature change");
        self.checkout("main");
        self.write_commit("a.txt", ours, "main change");
    }

    /// Runs a command expected to stop with conflicts (non-zero exit).
    pub fn expect_conflict(&self, args: &[&str]) {
        let out = self.git_raw(args);
        assert!(
            !out.status.success(),
            "git {args:?} unexpectedly succeeded: {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    /// `main` vs `feature` both editing `a.txt`; leaves a conflicted `git merge feature`.
    pub fn merge_conflict(&self) {
        self.diverge(
            "one\ntwo\nthree\n",
            "one\nOURS\nthree\n",
            "one\nTHEIRS\nthree\n",
        );
        self.expect_conflict(&["merge", "feature"]);
    }

    /// Same divergence, but `feature` is rebased onto `main`; leaves a conflicted rebase.
    pub fn rebase_conflict(&self) {
        self.diverge(
            "one\ntwo\nthree\n",
            "one\nOURS\nthree\n",
            "one\nTHEIRS\nthree\n",
        );
        self.checkout("feature");
        self.expect_conflict(&["rebase", "main"]);
    }

    pub fn cherry_pick_conflict(&self) -> String {
        self.diverge(
            "one\ntwo\nthree\n",
            "one\nOURS\nthree\n",
            "one\nTHEIRS\nthree\n",
        );
        let sha = self.git(&["rev-parse", "feature"]);
        self.expect_conflict(&["cherry-pick", &sha]);
        sha
    }

    pub fn revert_conflict(&self) -> String {
        self.write_commit("a.txt", "one\ntwo\nthree\n", "base");
        let target = self.write_commit("a.txt", "one\nMIDDLE\nthree\n", "to be reverted");
        self.write_commit("a.txt", "one\nLATER\nthree\n", "later change");
        self.expect_conflict(&["revert", &target]);
        target
    }
}
