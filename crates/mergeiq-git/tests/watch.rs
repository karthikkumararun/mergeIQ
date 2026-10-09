mod support;
use std::sync::mpsc;
use std::time::Duration;

use mergeiq_git::RepoPath;
use support::Scenario;

#[test]
fn external_resolution() {
    let s = Scenario::new();
    s.merge_conflict();
    let (tx, rx) = mpsc::channel();
    let _watch = s
        .repo()
        .watch(move || {
            let _ = tx.send(());
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(600));
    while rx.try_recv().is_ok() {}

    s.write("a.txt", "one\nexternal\nthree\n");
    s.git(&["add", "a.txt"]);
    rx.recv_timeout(Duration::from_secs(1))
        .expect("RepoChanged within 1s of git add");
}

#[test]
fn own_writes_are_suppressed() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    let (tx, rx) = mpsc::channel();
    let _watch = repo
        .watch(move || {
            let _ = tx.send(());
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(600));
    while rx.try_recv().is_ok() {}

    repo.save_resolved(&RepoPath::from_bytes(b"a.txt".to_vec()), b"mine\n")
        .unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(1200)).is_err());
}
