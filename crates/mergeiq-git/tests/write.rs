mod support;
use mergeiq_git::{AcceptSide, RepoPath};
use support::Scenario;

fn a() -> RepoPath {
    RepoPath::from_bytes(b"a.txt".to_vec())
}

fn stages(s: &Scenario) -> String {
    s.git(&["ls-files", "-s", "--", "a.txt"])
}

#[test]
fn save_resolved() {
    let s = Scenario::new();
    s.merge_conflict();
    s.repo()
        .save_resolved(&a(), b"one\nMERGED\nthree\n")
        .unwrap();
    assert_eq!(s.read("a.txt"), b"one\nMERGED\nthree\n");
    let ls = stages(&s);
    assert_eq!(ls.lines().count(), 1, "{ls}");
    assert!(ls.contains(" 0\t"), "{ls}");
    assert!(!s.path("a.txt.mergeiq.tmp").exists());
}

#[test]
fn save_unresolved_does_not_stage() {
    let s = Scenario::new();
    s.merge_conflict();
    s.repo().save_unresolved(&a(), b"partial\n").unwrap();
    assert_eq!(s.read("a.txt"), b"partial\n");
    assert_eq!(stages(&s).lines().count(), 3);
}

#[cfg(unix)]
#[test]
fn write_preserves_exec_bit() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scenario::new();
    s.merge_conflict();
    std::fs::set_permissions(s.path("a.txt"), std::fs::Permissions::from_mode(0o755)).unwrap();
    s.repo().save_unresolved(&a(), b"x\n").unwrap();
    let mode = std::fs::metadata(s.path("a.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111);
}

#[test]
fn autocrlf_true_writes_crlf_but_stages_lf() {
    let s = Scenario::new();
    s.merge_conflict();
    s.git(&["config", "core.autocrlf", "true"]);
    s.repo()
        .save_resolved(&a(), b"one\nMERGED\nthree\n")
        .unwrap();
    assert_eq!(s.read("a.txt"), b"one\r\nMERGED\r\nthree\r\n");
    assert_eq!(
        s.git_raw(&["show", ":a.txt"]).stdout,
        b"one\nMERGED\nthree\n"
    );
}

#[test]
fn eol_attribute_overrides_autocrlf() {
    let s = Scenario::new();
    s.write_commit(".gitattributes", "*.txt eol=lf\nb.bin -text\n", "attrs");
    s.git(&["config", "core.autocrlf", "true"]);
    s.merge_conflict();
    let repo = s.repo();
    repo.save_unresolved(&a(), b"x\ny\n").unwrap();
    assert_eq!(s.read("a.txt"), b"x\ny\n");
    repo.save_unresolved(&RepoPath::from_bytes(b"b.bin".to_vec()), b"x\ny\n")
        .unwrap();
    assert_eq!(s.read("b.bin"), b"x\ny\n");
}

#[test]
fn accept_side_takes_stage_and_stages_it() {
    let s = Scenario::new();
    s.merge_conflict();
    s.repo().accept_side(&a(), AcceptSide::Theirs).unwrap();
    assert_eq!(s.read("a.txt"), b"one\nTHEIRS\nthree\n");
    assert_eq!(stages(&s).lines().count(), 1);

    let s = Scenario::new();
    s.merge_conflict();
    s.repo().accept_side(&a(), AcceptSide::Ours).unwrap();
    assert_eq!(s.read("a.txt"), b"one\nOURS\nthree\n");
}

#[test]
fn accept_deleted_side() {
    let s = Scenario::new();
    s.write_commit("a.txt", "one\ntwo\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "one\nTWO\n", "feature modify");
    s.checkout("main");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("main delete");
    s.expect_conflict(&["merge", "feature"]);
    s.repo().accept_side(&a(), AcceptSide::Ours).unwrap();
    assert!(!s.path("a.txt").exists());
    assert!(stages(&s).is_empty());
}

#[test]
fn restore_conflict() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    repo.save_resolved(&a(), b"resolved\n").unwrap();
    assert_eq!(stages(&s).lines().count(), 1);
    repo.restore_conflict(&a()).unwrap();
    assert_eq!(stages(&s).lines().count(), 3);
    let text = String::from_utf8(s.read("a.txt")).unwrap();
    assert!(
        text.contains("<<<<<<<") && text.contains(">>>>>>>"),
        "{text}"
    );
}

#[test]
#[cfg(unix)] // `*` is not a valid file name character on Windows
fn paths_with_leading_dashes_and_globs_are_literal() {
    let s = Scenario::new();
    s.write_commit("-weird*.txt", "one\ntwo\n", "base");
    s.write_commit("other.txt", "o\n", "other");
    s.write("-weird*.txt", "changed\n");
    let p = RepoPath::from_bytes(b"-weird*.txt".to_vec());
    s.repo().save_resolved(&p, b"resolved\n").unwrap();
    assert_eq!(s.git(&["diff", "--cached", "--name-only"]), "-weird*.txt");
}

#[test]
fn delete_resolved_removes_file_and_stages_deletion() {
    let s = Scenario::new();
    s.merge_conflict();
    s.repo().delete_resolved(&a()).unwrap();
    assert!(!s.path("a.txt").exists());
    assert!(stages(&s).is_empty());
}

#[test]
fn delete_resolved_requires_a_conflict() {
    let s = Scenario::new();
    s.write_commit("b.txt", "x\n", "init");
    let err = s
        .repo()
        .delete_resolved(&RepoPath::from_bytes(b"b.txt".to_vec()))
        .unwrap_err();
    assert!(matches!(err, mergeiq_git::GitError::NoSuchConflict { .. }));
    assert!(s.path("b.txt").exists());
}
