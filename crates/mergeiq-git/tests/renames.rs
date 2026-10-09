//! Rename detection for conflicted paths against real repositories.

mod support;

use mergeiq_git::{RenameSide, RepoPath};
use support::Scenario;

const BODY: &str = "line one\nline two\nline three\nline four\nline five\nline six\n";

fn rp(s: &str) -> RepoPath {
    RepoPath::from_bytes(s.as_bytes().to_vec())
}

#[test]
fn rename_rename_to_different_paths() {
    let s = Scenario::new();
    s.write_commit("a.ts", BODY, "base");
    s.checkout_new("feature");
    s.git(&["mv", "a.ts", "c.ts"]);
    s.commit_all("feature renames");
    s.checkout("main");
    s.git(&["mv", "a.ts", "b.ts"]);
    s.commit_all("main renames");
    s.expect_conflict(&["merge", "feature"]);

    let repo = s.repo();
    let paths: Vec<String> = repo
        .list_conflicts()
        .unwrap()
        .into_iter()
        .map(|e| e.display)
        .collect();
    assert!(paths.contains(&"b.ts".to_string()), "{paths:?}");
    assert!(paths.contains(&"c.ts".to_string()), "{paths:?}");

    let pair = repo.rename_pair_of(&rp("b.ts")).unwrap().expect("a pair");
    assert_eq!(pair.from, "a.ts");
    assert_eq!(pair.ours.to, "b.ts");
    assert_eq!(pair.ours.side, RenameSide::Ours);
    assert_eq!(pair.theirs.to, "c.ts");
    assert_eq!(pair.theirs.side, RenameSide::Theirs);
    // The same pair is found from the other destination.
    assert_eq!(repo.rename_pair_of(&rp("c.ts")).unwrap(), Some(pair));
}

#[test]
fn rename_delete_reports_the_rename() {
    let s = Scenario::new();
    s.write_commit("a.ts", BODY, "base");
    s.write_commit("keep.txt", "k\n", "keep");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "a.ts"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.git(&["mv", "a.ts", "b.ts"]);
    s.commit_all("main renames");
    s.expect_conflict(&["merge", "feature"]);

    let repo = s.repo();
    let infos = repo.renames_of(&rp("b.ts")).unwrap();
    assert_eq!(infos.len(), 1, "{infos:?}");
    assert_eq!(infos[0].side, RenameSide::Ours);
    assert_eq!(
        (infos[0].from.as_str(), infos[0].to.as_str()),
        ("a.ts", "b.ts")
    );
    assert_eq!(repo.rename_pair_of(&rp("b.ts")).unwrap(), None);
}

#[test]
fn unrelated_paths_have_no_renames() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    assert!(repo.renames_of(&rp("a.txt")).unwrap().is_empty());
    assert_eq!(repo.rename_pair_of(&rp("a.txt")).unwrap(), None);
}

#[test]
fn renames_are_cached_per_operation_head() {
    let s = Scenario::new();
    s.write_commit("a.ts", BODY, "base");
    s.checkout_new("feature");
    s.git(&["mv", "a.ts", "c.ts"]);
    s.commit_all("feature renames");
    s.checkout("main");
    s.git(&["mv", "a.ts", "b.ts"]);
    s.commit_all("main renames");
    s.expect_conflict(&["merge", "feature"]);
    let repo = s.repo();
    let first = repo.renames_of(&rp("b.ts")).unwrap();
    let second = repo.renames_of(&rp("b.ts")).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 1);
}

#[test]
fn rename_in_a_rebase_uses_the_replayed_commit() {
    let s = Scenario::new();
    s.write_commit("a.ts", BODY, "base");
    s.checkout_new("feature");
    s.git(&["mv", "a.ts", "c.ts"]);
    s.commit_all("feature renames");
    s.checkout("main");
    s.git(&["mv", "a.ts", "b.ts"]);
    s.commit_all("main renames");
    s.checkout("feature");
    s.expect_conflict(&["rebase", "main"]);
    let repo = s.repo();
    // During a rebase "ours" is the branch being rebased onto (main: b.ts), "theirs" the commit.
    let pair = repo.rename_pair_of(&rp("b.ts")).unwrap().expect("a pair");
    assert_eq!(pair.ours.to, "b.ts");
    assert_eq!(pair.theirs.to, "c.ts");
}
