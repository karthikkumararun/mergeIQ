mod support;
use mergeiq_git::{GitError, Operation, Repo};
use support::Scenario;

#[test]
fn nested_path() {
    let s = Scenario::new();
    s.write_commit("src/deep/dir/f.txt", "x\n", "init");
    let repo = Repo::open(s.exec(), &s.path("src/deep/dir")).unwrap();
    assert_eq!(repo.root(), s.root.as_path());
    assert_eq!(repo.git_dir(), s.root.join(".git"));
}

#[test]
fn linked_worktree() {
    let s = Scenario::new();
    s.merge_conflict();
    s.git(&["merge", "--abort"]);
    let wt = s.root.parent().unwrap().join("wt");
    s.git(&["worktree", "add", "-q", "-b", "other", wt.to_str().unwrap()]);
    // Conflict inside the linked worktree only.
    s.git_in(&wt, &["checkout", "-q", "-b", "wt-side"]);
    std::fs::write(wt.join("a.txt"), "one\nWT\nthree\n").unwrap();
    s.git_in(&wt, &["commit", "-qam", "wt change"]);
    let out = std::process::Command::new("git")
        .current_dir(&wt)
        .args(["merge", "feature"])
        .output()
        .unwrap();
    assert!(!out.status.success());

    let repo = Repo::open(s.exec(), &wt.join(".")).unwrap();
    assert_eq!(repo.root(), dunce::canonicalize(&wt).unwrap());
    assert!(repo
        .git_dir()
        .components()
        .any(|c| c.as_os_str() == "worktrees"));
    assert_eq!(repo.operation().unwrap(), Operation::Merge);
    // The main worktree is unaffected.
    assert_eq!(s.repo().operation().unwrap(), Operation::None);
}

#[test]
fn submodule() {
    let s = Scenario::new();
    let sub = Scenario::new();
    sub.write_commit("s.txt", "sub\n", "sub init");
    s.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "-q",
        sub.root.to_str().unwrap(),
        "libs/sub",
    ]);
    let repo = Repo::open(s.exec(), &s.path("libs/sub")).unwrap();
    assert_eq!(repo.root(), s.path("libs/sub"));
    assert!(repo
        .git_dir()
        .components()
        .any(|c| c.as_os_str() == "modules"));
}

#[test]
fn bare_repository_rejected() {
    let s = Scenario::new();
    s.write_commit("a", "a\n", "init");
    let bare = s.root.parent().unwrap().join("bare.git");
    s.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    let err = Repo::open(s.exec(), &bare).unwrap_err();
    assert!(matches!(err, GitError::Bare), "{err:?}");
}

#[test]
fn outside_repository() {
    let dir = tempfile::tempdir().unwrap();
    let err = Repo::open(Scenario::new().exec(), dir.path()).unwrap_err();
    assert!(matches!(err, GitError::NotARepo { .. }), "{err:?}");
}
