mod support;
use mergeiq_git::{ConflictType, GitError, RepoPath};
use support::Scenario;

fn only_conflict(s: &Scenario) -> mergeiq_git::ConflictEntry {
    let mut list = s.repo().list_conflicts().unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    list.remove(0)
}

fn base_commit(s: &Scenario) {
    s.write_commit("keep.txt", "k\n", "base");
}

#[test]
fn both_modified() {
    let s = Scenario::new();
    s.merge_conflict();
    let e = only_conflict(&s);
    assert_eq!(e.conflict_type, ConflictType::BothModified);
    assert_eq!(e.display, "a.txt");
    assert_eq!(
        e.stages.iter().map(|x| x.stage).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert!(e.stages.iter().all(|x| x.mode == "100644"));
    assert!(!e.has_symlink && !e.has_gitlink);
}

#[test]
fn both_added() {
    let s = Scenario::new();
    base_commit(&s);
    s.checkout_new("feature");
    s.write_commit("n.txt", "theirs\n", "feature add");
    s.checkout("main");
    s.write_commit("n.txt", "ours\n", "main add");
    s.expect_conflict(&["merge", "feature"]);
    assert_eq!(only_conflict(&s).conflict_type, ConflictType::BothAdded);
}

#[test]
fn modify_delete_conflict() {
    // ours deleted a.txt, theirs modified it
    let s = Scenario::new();
    s.write_commit("a.txt", "one\ntwo\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "one\nTWO\n", "feature modify");
    s.checkout("main");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("main delete");
    s.expect_conflict(&["merge", "feature"]);
    let e = only_conflict(&s);
    assert_eq!(e.display, "a.txt");
    assert_eq!(e.conflict_type, ConflictType::DeletedByUs);
}

#[test]
fn deleted_by_them() {
    let s = Scenario::new();
    s.write_commit("a.txt", "one\ntwo\n", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("feature delete");
    s.checkout("main");
    s.write_commit("a.txt", "one\nTWO\n", "main modify");
    s.expect_conflict(&["merge", "feature"]);
    assert_eq!(only_conflict(&s).conflict_type, ConflictType::DeletedByThem);
}

#[test]
fn added_by_us_and_them_and_both_deleted() {
    // AddedByThem / AddedByUs arise from index states; script them with update-index.
    let s = Scenario::new();
    s.merge_conflict();
    let oid = s.git(&["rev-parse", ":2:a.txt"]);
    let set_stages = |stages: &[(u8, &str)]| {
        s.git(&["update-index", "--force-remove", "a.txt"]);
        let input: String = stages
            .iter()
            .map(|(n, o)| format!("100644 {o} {n}\ta.txt\n"))
            .collect();
        let mut child = std::process::Command::new("git")
            .current_dir(&s.root)
            .args(["update-index", "--index-info"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    };
    set_stages(&[(2, &oid)]);
    assert_eq!(only_conflict(&s).conflict_type, ConflictType::AddedByUs);
    set_stages(&[(3, &oid)]);
    assert_eq!(only_conflict(&s).conflict_type, ConflictType::AddedByThem);
    set_stages(&[(1, &oid)]);
    assert_eq!(only_conflict(&s).conflict_type, ConflictType::BothDeleted);
}

#[test]
fn symlink_and_gitlink_flags() {
    let s = Scenario::new();
    s.merge_conflict();
    let oid = s.git(&["rev-parse", ":2:a.txt"]);
    s.git(&["update-index", "--force-remove", "a.txt"]);
    let mut child = std::process::Command::new("git")
        .current_dir(&s.root)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("120000 {oid} 2\ta.txt\n160000 {oid} 3\ta.txt\n").as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
    let e = only_conflict(&s);
    assert!(e.has_symlink && e.has_gitlink);
    assert_eq!(e.conflict_type, ConflictType::BothAdded);
}

#[test]
#[cfg(unix)]
fn non_utf8_path() {
    // Some filesystems (APFS) refuse non-UTF-8 names, so script the conflict in the index.
    use std::io::Write;
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "base");
    let blob = |c: &str| {
        s.write("tmp.blob", c);
        s.git(&["hash-object", "-w", "tmp.blob"])
    };
    let (b, o, t) = (
        blob("one\ntwo\n"),
        blob("one\nOURS\n"),
        blob("one\nTHEIRS\n"),
    );
    let mut input = Vec::new();
    for (n, oid) in [(1, &b), (2, &o), (3, &t)] {
        input.extend_from_slice(format!("100644 {oid} {n}\t").as_bytes());
        input.extend_from_slice(b"bad\xFF.txt\n");
    }
    let mut child = std::process::Command::new("git")
        .current_dir(&s.root)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&input).unwrap();
    assert!(child.wait().unwrap().success());

    let repo = s.repo();
    let e = only_conflict(&s);
    assert!(e.display.contains('\u{FFFD}'));
    // The opaque token round-trips into other calls.
    let path = e.path.decode().unwrap();
    assert_eq!(path.as_bytes(), b"bad\xFF.txt");
    let blobs = repo.read_blobs(&path).unwrap();
    assert_eq!(blobs.ours.unwrap(), b"one\nOURS\n");
}

#[test]
fn read_three_stages() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    let path = RepoPath::from_bytes(b"a.txt".to_vec());
    let blobs = repo.read_blobs(&path).unwrap();
    for (n, got) in [(1, &blobs.base), (2, &blobs.ours), (3, &blobs.theirs)] {
        let expected = s.git_raw(&["show", &format!(":{n}:a.txt")]).stdout;
        assert_eq!(got.as_deref(), Some(expected.as_slice()), "stage {n}");
    }
    assert_eq!(blobs.working.unwrap(), s.read("a.txt"));
}

#[test]
fn absent_stage_is_none_and_missing_conflict_errors() {
    let s = Scenario::new();
    s.write_commit("a.txt", "one\ntwo\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "one\nTWO\n", "feature modify");
    s.checkout("main");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("main delete");
    s.expect_conflict(&["merge", "feature"]);
    let repo = s.repo();
    let blobs = repo
        .read_blobs(&RepoPath::from_bytes(b"a.txt".to_vec()))
        .unwrap();
    assert!(blobs.ours.is_none() && blobs.base.is_some() && blobs.theirs.is_some());
    let err = repo
        .read_blobs(&RepoPath::from_bytes(b"keep.txt".to_vec()))
        .unwrap_err();
    assert!(matches!(err, GitError::NoSuchConflict { .. }));
}
