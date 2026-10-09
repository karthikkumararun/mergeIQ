mod support;
use mergeiq_git::Operation;
use support::Scenario;

#[test]
fn no_operation() {
    let s = Scenario::new();
    s.write_commit("a.txt", "x\n", "init");
    assert_eq!(s.repo().operation().unwrap(), Operation::None);
}

#[test]
fn merge_in_progress() {
    let s = Scenario::new();
    s.merge_conflict();
    assert_eq!(s.repo().operation().unwrap(), Operation::Merge);
}

#[test]
fn interactive_rebase_step() {
    let s = Scenario::new();
    s.write_commit("a.txt", "one\ntwo\nthree\n", "base");
    s.checkout_new("feature");
    for i in 1..=2 {
        s.write_commit(&format!("f{i}.txt"), "x\n", &format!("feature {i}"));
    }
    s.write_commit("a.txt", "one\nTHEIRS\nthree\n", "feature 3 (conflicts)");
    for i in 4..=7 {
        s.write_commit(&format!("f{i}.txt"), "x\n", &format!("feature {i}"));
    }
    s.checkout("main");
    s.write_commit("a.txt", "one\nOURS\nthree\n", "main change");
    let onto = s.git(&["rev-parse", "main"]);
    s.checkout("feature");
    s.expect_conflict(&["rebase", "main"]);
    match s.repo().operation().unwrap() {
        Operation::Rebase {
            step,
            total,
            onto: got,
        } => {
            assert_eq!((step, total), (3, 7));
            assert_eq!(got, onto);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn cherry_pick_in_progress() {
    let s = Scenario::new();
    s.cherry_pick_conflict();
    assert_eq!(s.repo().operation().unwrap(), Operation::CherryPick);
}

#[test]
fn revert_in_progress() {
    let s = Scenario::new();
    s.revert_conflict();
    assert_eq!(s.repo().operation().unwrap(), Operation::Revert);
}

#[test]
fn am_in_progress() {
    let s = Scenario::new();
    s.diverge(
        "one\ntwo\nthree\n",
        "one\nOURS\nthree\n",
        "one\nTHEIRS\nthree\n",
    );
    let patch = s.git(&["format-patch", "--stdout", "-1", "feature"]);
    let patch_file = s.root.parent().unwrap().join("x.patch");
    std::fs::write(&patch_file, patch + "\n").unwrap();
    let out = s.git_raw(&["am", "--3way", patch_file.to_str().unwrap()]);
    let git_dir = s.root.join(".git");
    let listing: Vec<_> = std::fs::read_dir(&git_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    eprintln!("DIAG am status={:?}", out.status);
    eprintln!("DIAG am stdout={}", String::from_utf8_lossy(&out.stdout));
    eprintln!("DIAG am stderr={}", String::from_utf8_lossy(&out.stderr));
    eprintln!("DIAG .git={listing:?}");
    eprintln!(
        "DIAG patch={:?}",
        std::fs::read_to_string(&patch_file).unwrap()
    );
    assert!(!out.status.success());
    assert_eq!(s.repo().operation().unwrap(), Operation::Am);
}

#[test]
fn conflicts_without_state_file_are_unknown() {
    let s = Scenario::new();
    s.diverge(
        "one\ntwo\nthree\n",
        "one\nOURS\nthree\n",
        "one\nTHEIRS\nthree\n",
    );
    s.checkout("feature");
    s.write("a.txt", "one\nWIP\nthree\n");
    s.git(&["stash", "-q"]);
    s.checkout("main");
    s.expect_conflict(&["stash", "pop"]);
    assert_eq!(s.repo().operation().unwrap(), Operation::Unknown);
}
