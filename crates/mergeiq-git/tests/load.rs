mod support;
use mergeiq_core::{ChunkKind, Options};
use mergeiq_git::{ConflictType, Operation, RepoPath};
use support::Scenario;

#[test]
fn conflict_load_returns_analysis_labels_and_context() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    let token = RepoPath::from_bytes(b"a.txt".to_vec()).token();
    let load = repo.load_conflict(&token, &Options::default()).unwrap();

    assert_eq!(load.entry.conflict_type, ConflictType::BothModified);
    assert_eq!(load.labels.ours.ref_name.as_deref(), Some("main"));
    assert_eq!(load.labels.theirs.ref_name.as_deref(), Some("feature"));
    assert_eq!(load.context.ours[0].subject, "main change");
    assert_eq!(load.context.theirs[0].subject, "feature change");
    assert!(load.analysis_error.is_none());
    let analysis = load.analysis.unwrap();
    assert_eq!(analysis.base.text, "one\ntwo\nthree\n");
    assert_eq!(analysis.ours.text, "one\nOURS\nthree\n");
    assert_eq!(analysis.theirs.text, "one\nTHEIRS\nthree\n");
    assert_eq!(analysis.chunks.len(), 1);
    assert_eq!(analysis.chunks[0].kind, ChunkKind::Conflict);
}

#[test]
fn conflict_load_with_absent_stage_uses_empty_text() {
    let s = Scenario::new();
    s.write_commit("a.txt", "one\ntwo\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "one\nTWO\n", "feature modify");
    s.checkout("main");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("main delete");
    s.expect_conflict(&["merge", "feature"]);
    let token = RepoPath::from_bytes(b"a.txt".to_vec()).token();
    let load = s.repo().load_conflict(&token, &Options::default()).unwrap();
    assert_eq!(load.entry.conflict_type, ConflictType::DeletedByUs);
    assert_eq!(load.analysis.unwrap().ours.text, "");
}

#[test]
fn conflict_load_binary_reports_error_not_panic() {
    let s = Scenario::new();
    s.write_commit("a.txt", "one\n", "base");
    s.checkout_new("feature");
    s.write("a.txt", b"\0\x01theirs".as_slice());
    s.commit_all("feature");
    s.checkout("main");
    s.write("a.txt", b"\0\x02ours".as_slice());
    s.commit_all("main");
    s.expect_conflict(&["merge", "feature"]);
    let token = RepoPath::from_bytes(b"a.txt".to_vec()).token();
    let load = s.repo().load_conflict(&token, &Options::default()).unwrap();
    assert!(load.analysis.is_none());
    assert!(load.analysis_error.unwrap().contains("binary"));
}

#[test]
fn conflict_load_rejects_bad_token() {
    let s = Scenario::new();
    s.merge_conflict();
    let bad = mergeiq_git::PathToken("Li4vZXRj".into()); // "../etc"
    assert!(s.repo().load_conflict(&bad, &Options::default()).is_err());
}

#[test]
fn repo_status_summarises_state() {
    let s = Scenario::new();
    s.rebase_conflict();
    let status = s.repo().status().unwrap();
    assert!(matches!(status.operation, Operation::Rebase { .. }));
    assert_eq!(status.conflicts.len(), 1);
    assert!(status.labels.ours.role.contains("main"));
}
