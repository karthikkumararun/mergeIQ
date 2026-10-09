mod support;
use mergeiq_git::{GitError, Operation, RepoPath};
use support::Scenario;

fn a() -> RepoPath {
    RepoPath::from_bytes(b"a.txt".to_vec())
}

#[test]
fn continue_blocked() {
    let s = Scenario::new();
    s.merge_conflict();
    let head_before = s.git(&["rev-parse", "HEAD"]);
    let err = s.repo().op_continue().unwrap_err();
    assert!(
        matches!(err, GitError::UnresolvedPaths { count: 1 }),
        "{err:?}"
    );
    // git was not invoked: merge still in progress, HEAD unchanged.
    assert_eq!(s.repo().operation().unwrap(), Operation::Merge);
    assert_eq!(s.git(&["rev-parse", "HEAD"]), head_before);
}

#[test]
fn merge_continue() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    repo.save_resolved(&a(), b"one\nBOTH\nthree\n").unwrap();
    let outcome = repo.op_continue().unwrap();
    assert!(outcome.finished);
    assert_eq!(
        s.git(&["log", "-1", "--format=%s"]),
        "Merge branch 'feature'"
    );
    assert_eq!(
        s.git(&["rev-list", "--parents", "-1", "HEAD"])
            .split(' ')
            .count(),
        3
    );
    assert_eq!(repo.operation().unwrap(), Operation::None);
}

#[test]
fn rebase_continue() {
    let s = Scenario::new();
    s.rebase_conflict();
    let repo = s.repo();
    repo.save_resolved(&a(), b"one\nREBASED\nthree\n").unwrap();
    let outcome = repo.op_continue().unwrap();
    assert!(outcome.finished, "{}", outcome.message);
    assert_eq!(s.git(&["log", "-1", "--format=%s"]), "feature change");
    assert_eq!(
        s.git(&["rev-parse", "HEAD~1"]),
        s.git(&["rev-parse", "main"])
    );
    assert_eq!(s.git(&["symbolic-ref", "--short", "HEAD"]), "feature");
}

#[test]
fn rebase_continue_reports_next_conflict() {
    let s = Scenario::new();
    s.write_commit("a.txt", "1\n2\n3\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "1\nF1\n3\n", "f1");
    s.write_commit("a.txt", "1\nF2\n3\n", "f2");
    s.checkout("main");
    s.write_commit("a.txt", "1\nM\n3\n", "m");
    s.checkout("feature");
    s.expect_conflict(&["rebase", "main"]);
    let repo = s.repo();
    repo.save_resolved(&a(), b"1\nR1\n3\n").unwrap();
    let outcome = repo.op_continue().unwrap();
    assert!(!outcome.finished);
    assert!(matches!(
        repo.operation().unwrap(),
        Operation::Rebase {
            step: 2,
            total: 2,
            ..
        }
    ));
}

#[test]
fn rebase_skip() {
    let s = Scenario::new();
    s.rebase_conflict();
    let outcome = s.repo().op_skip().unwrap();
    assert!(outcome.finished);
    assert_eq!(s.git(&["rev-parse", "HEAD"]), s.git(&["rev-parse", "main"]));
}

#[test]
fn skip_is_rebase_only() {
    let s = Scenario::new();
    s.merge_conflict();
    assert!(matches!(
        s.repo().op_skip().unwrap_err(),
        GitError::Unsupported { .. }
    ));
}

#[test]
fn abort_each_operation() {
    let s = Scenario::new();
    s.merge_conflict();
    let before = s.git(&["rev-parse", "HEAD"]);
    assert!(s.repo().op_abort().unwrap().finished);
    assert_eq!(s.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(s.git(&["status", "--porcelain"]), "");

    let s = Scenario::new();
    s.rebase_conflict();
    assert!(s.repo().op_abort().unwrap().finished);
    assert_eq!(s.git(&["symbolic-ref", "--short", "HEAD"]), "feature");

    let s = Scenario::new();
    s.cherry_pick_conflict();
    assert!(s.repo().op_abort().unwrap().finished);

    let s = Scenario::new();
    s.revert_conflict();
    assert!(s.repo().op_abort().unwrap().finished);
}

#[test]
fn no_operation_errors() {
    let s = Scenario::new();
    s.write_commit("a.txt", "x\n", "init");
    assert!(matches!(
        s.repo().op_abort().unwrap_err(),
        GitError::NoOperation
    ));
    assert!(matches!(
        s.repo().op_continue().unwrap_err(),
        GitError::NoOperation
    ));
}
