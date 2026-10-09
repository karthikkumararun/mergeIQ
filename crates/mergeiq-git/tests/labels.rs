mod support;
use support::Scenario;

#[test]
fn merge_labels() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    let labels = repo.side_labels(&repo.operation().unwrap()).unwrap();
    assert_eq!(labels.ours.ref_name.as_deref(), Some("main"));
    assert_eq!(labels.theirs.ref_name.as_deref(), Some("feature"));
    assert_eq!(labels.ours.role, "Your branch");
    assert_eq!(labels.theirs.role, "Incoming: feature");
    assert_eq!(labels.theirs.subject.as_deref(), Some("feature change"));
    assert_eq!(labels.ours.git_term, "ours");
    assert_eq!(labels.theirs.git_term, "theirs");
}

#[test]
fn rebase_labels_are_swapped_correctly() {
    let s = Scenario::new();
    s.rebase_conflict();
    let repo = s.repo();
    let labels = repo.side_labels(&repo.operation().unwrap()).unwrap();
    // Stage 2 is the upstream we are rebasing onto; stage 3 is the replayed feature commit.
    assert!(labels.ours.role.contains("main"), "{:?}", labels.ours.role);
    assert!(labels.ours.role.starts_with("Upstream"));
    assert_eq!(labels.ours.subject.as_deref(), Some("main change"));
    assert_eq!(labels.theirs.subject.as_deref(), Some("feature change"));
    assert_eq!(labels.theirs.role, "Your commit being replayed");
    assert_eq!(labels.theirs.ref_name.as_deref(), Some("feature"));
}

#[test]
fn cherry_pick_and_revert_labels() {
    let s = Scenario::new();
    s.cherry_pick_conflict();
    let repo = s.repo();
    let labels = repo.side_labels(&repo.operation().unwrap()).unwrap();
    assert_eq!(labels.ours.role, "Current branch");
    assert_eq!(labels.ours.ref_name.as_deref(), Some("main"));
    assert_eq!(labels.theirs.role, "Cherry-picked commit");
    assert_eq!(labels.theirs.subject.as_deref(), Some("feature change"));

    let s = Scenario::new();
    let target = s.revert_conflict();
    let repo = s.repo();
    let labels = repo.side_labels(&repo.operation().unwrap()).unwrap();
    assert_eq!(labels.theirs.role, format!("Revert of {}", &target[..7]));
}
