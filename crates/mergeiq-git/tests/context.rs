mod support;
use mergeiq_git::RepoPath;
use support::Scenario;

fn a() -> RepoPath {
    RepoPath::from_bytes(b"a.txt".to_vec())
}

#[test]
fn merge_context() {
    let s = Scenario::new();
    s.write_commit("a.txt", "1\n2\n3\n4\n5\n6\n7\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "1\nT1\n3\n4\n5\n6\n7\n", "theirs one");
    s.write_commit("a.txt", "1\nT1\n3\n4\n5\n6\nT2\n", "theirs two");
    s.write_commit("other.txt", "x\n", "theirs unrelated");
    s.checkout("main");
    s.write_commit("a.txt", "1\nO1\n3\n4\n5\n6\n7\n", "ours one");
    s.write_commit("a.txt", "1\nO1\n3\n4\n5\n6\nO2\n", "ours two");
    s.write_commit("other2.txt", "x\n", "ours unrelated");
    s.expect_conflict(&["merge", "feature"]);

    let repo = s.repo();
    let ctx = repo.file_context(&a(), &repo.operation().unwrap()).unwrap();
    let subjects =
        |v: &[mergeiq_git::CommitSummary]| v.iter().map(|c| c.subject.clone()).collect::<Vec<_>>();
    assert_eq!(subjects(&ctx.ours), ["ours two", "ours one"]);
    assert_eq!(subjects(&ctx.theirs), ["theirs two", "theirs one"]);
    assert_eq!(ctx.ours[0].author, "Test User");
    assert_eq!(ctx.ours[0].sha.len(), 40);
    assert!(!ctx.ours[0].date.is_empty());
}

#[test]
fn rebase_theirs_is_single_replayed_commit() {
    let s = Scenario::new();
    s.rebase_conflict();
    let repo = s.repo();
    let ctx = repo.file_context(&a(), &repo.operation().unwrap()).unwrap();
    assert_eq!(ctx.theirs.len(), 1);
    assert_eq!(ctx.theirs[0].subject, "feature change");
    assert_eq!(ctx.ours[0].subject, "main change");
}

#[test]
fn cherry_pick_theirs_is_single_commit() {
    let s = Scenario::new();
    let sha = s.cherry_pick_conflict();
    let repo = s.repo();
    let ctx = repo.file_context(&a(), &repo.operation().unwrap()).unwrap();
    assert_eq!(ctx.theirs.len(), 1);
    assert_eq!(ctx.theirs[0].sha, sha);
}

#[test]
fn context_is_capped_at_50_per_side() {
    let s = Scenario::new();
    s.write_commit("a.txt", "base\n", "base");
    s.checkout_new("feature");
    s.write_commit("a.txt", "theirs\n", "t");
    s.checkout("main");
    for i in 0..55 {
        s.write_commit("a.txt", &format!("ours {i}\n"), &format!("o{i}"));
    }
    s.expect_conflict(&["merge", "feature"]);
    let repo = s.repo();
    let ctx = repo.file_context(&a(), &repo.operation().unwrap()).unwrap();
    assert_eq!(ctx.ours.len(), 50);
    assert_eq!(ctx.ours[0].subject, "o54");
}

#[test]
fn commit_bodies_are_loaded_in_one_call() {
    let s = Scenario::new();
    s.write_commit("a.txt", "1\n2\n3\n", "base");
    s.checkout_new("feature");
    s.write("a.txt", "1\nT\n3\n");
    s.git(&[
        "commit",
        "-qam",
        "Change two\n\nBecause the old value was wrong.\n\nRefs #12",
    ]);
    s.checkout("main");
    s.write_commit("a.txt", "1\nO\n3\n", "ours subject only");
    s.expect_conflict(&["merge", "feature"]);

    let repo = s.repo();
    let ctx = repo.file_context(&a(), &repo.operation().unwrap()).unwrap();
    let shas: Vec<String> = ctx
        .ours
        .iter()
        .chain(&ctx.theirs)
        .map(|c| c.sha.clone())
        .collect();
    let bodies = repo.commit_bodies(&shas).unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "subject-only commits have no body: {bodies:?}"
    );
    assert_eq!(
        bodies[&ctx.theirs[0].sha],
        "Because the old value was wrong.\n\nRefs #12"
    );
    assert!(repo.commit_bodies(&[]).unwrap().is_empty());
    assert!(repo
        .commit_bodies(&["not-a-sha; rm -rf".to_string(), "deadbeef".to_string()])
        .unwrap()
        .is_empty());
}
