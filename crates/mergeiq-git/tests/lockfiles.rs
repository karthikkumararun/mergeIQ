//! go.sum union and lockfile regeneration against real repositories.

mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use mergeiq_git::{AcceptSide, ConflictClass, LockfileKind, OutputStream, RepoPath};
use support::Scenario;

fn rp(s: &str) -> RepoPath {
    RepoPath::from_bytes(s.as_bytes().to_vec())
}

fn both_modify(s: &Scenario, rel: &str, base: &str, ours: &str, theirs: &str) {
    s.write_commit(rel, base, "base");
    s.checkout_new("feature");
    s.write_commit(rel, theirs, "feature change");
    s.checkout("main");
    s.write_commit(rel, ours, "main change");
    s.expect_conflict(&["merge", "feature"]);
}

const A: &str = "github.com/a/a v1.0.0 h1:aaa=\n";
const B: &str = "github.com/b/b v1.2.0 h1:bbb=\n";
const C: &str = "github.com/c/c v0.1.0 h1:ccc=\n";

#[test]
fn go_sum_union_contains_both_sorted_and_is_staged() {
    let s = Scenario::new();
    both_modify(&s, "go.sum", A, &format!("{A}{C}"), &format!("{A}{B}"));
    let repo = s.repo();
    let entry = repo.list_conflicts().unwrap().remove(0);
    assert_eq!(
        entry.class,
        ConflictClass::Lockfile {
            kind: LockfileKind::GoSum
        }
    );

    let preview = repo.go_sum_preview(&rp("go.sum").token()).unwrap();
    assert_eq!(preview.result, format!("{A}{B}{C}"));

    repo.go_sum_union(&rp("go.sum").token()).unwrap();
    assert_eq!(s.read("go.sum"), format!("{A}{B}{C}").as_bytes());
    assert!(repo.list_conflicts().unwrap().is_empty());
    assert_eq!(
        s.git(&["ls-files", "-s", "go.sum"])
            .split_whitespace()
            .nth(2),
        Some("0")
    );
}

fn pnpm_conflict(s: &Scenario) {
    both_modify(
        s,
        "web/pnpm-lock.yaml",
        "lockfileVersion: 9\nv: base\n",
        "lockfileVersion: 9\nv: ours\n",
        "lockfileVersion: 9\nv: theirs\n",
    );
}

fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn regeneration_runs_in_the_lockfile_directory_and_stages_on_success() {
    let s = Scenario::new();
    pnpm_conflict(&s);
    let repo = s.repo();
    let out: Arc<Mutex<Vec<(OutputStream, String)>>> = Arc::default();
    let sink = out.clone();
    // `git rev-parse --show-prefix` prints the directory the command runs in.
    let result = repo
        .regenerate_lockfile(
            &rp("web/pnpm-lock.yaml").token(),
            AcceptSide::Theirs,
            "git rev-parse --show-prefix",
            &no_cancel(),
            move |stream, line| sink.lock().unwrap().push((stream, line.to_string())),
        )
        .unwrap();
    assert!(result.run.success(), "{result:?}");
    assert!(result.staged);
    assert_eq!(
        out.lock().unwrap().as_slice(),
        [(OutputStream::Stdout, "web/".to_string())]
    );
    // The right side was taken, then staged.
    assert_eq!(
        s.read("web/pnpm-lock.yaml"),
        b"lockfileVersion: 9\nv: theirs\n"
    );
    assert!(repo.list_conflicts().unwrap().is_empty());
}

#[test]
fn a_failing_command_shows_output_and_leaves_the_conflict() {
    let s = Scenario::new();
    pnpm_conflict(&s);
    let repo = s.repo();
    let out: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = out.clone();
    let result = repo
        .regenerate_lockfile(
            &rp("web/pnpm-lock.yaml").token(),
            AcceptSide::Ours,
            "git definitely-not-a-git-command",
            &no_cancel(),
            move |stream, line| {
                if stream == OutputStream::Stderr {
                    sink.lock().unwrap().push(line.to_string());
                }
            },
        )
        .unwrap();
    assert!(!result.run.success());
    assert_ne!(result.run.exit_code, Some(0));
    assert!(!result.staged);
    assert!(out
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.contains("not a git command")));
    // Still listed as a conflict; the chosen side's bytes are in the working tree.
    assert_eq!(repo.list_conflicts().unwrap().len(), 1);
    assert_eq!(
        s.read("web/pnpm-lock.yaml"),
        b"lockfileVersion: 9\nv: ours\n"
    );
}

#[test]
fn a_missing_program_is_reported_without_touching_anything() {
    let s = Scenario::new();
    pnpm_conflict(&s);
    let before = s.read("web/pnpm-lock.yaml");
    let err = s
        .repo()
        .regenerate_lockfile(
            &rp("web/pnpm-lock.yaml").token(),
            AcceptSide::Ours,
            "definitely-not-an-installed-program --flag",
            &no_cancel(),
            |_, _| {},
        )
        .unwrap_err();
    assert!(err.to_string().contains("was not found"), "{err}");
    assert_eq!(s.repo().list_conflicts().unwrap().len(), 1);
    assert_eq!(s.read("web/pnpm-lock.yaml"), before);
}

#[test]
fn an_invalid_command_never_touches_the_working_tree() {
    let s = Scenario::new();
    pnpm_conflict(&s);
    let before = s.read("web/pnpm-lock.yaml");
    assert!(s
        .repo()
        .regenerate_lockfile(
            &rp("web/pnpm-lock.yaml").token(),
            AcceptSide::Theirs,
            "unterminated 'quote",
            &no_cancel(),
            |_, _| {}
        )
        .is_err());
    assert_eq!(s.read("web/pnpm-lock.yaml"), before);
}

#[cfg(unix)]
#[test]
fn cancelling_stops_the_command_and_does_not_stage() {
    let s = Scenario::new();
    pnpm_conflict(&s);
    let repo = s.repo();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        flag.store(true, Ordering::Relaxed);
    });
    let started = std::time::Instant::now();
    let result = repo
        .regenerate_lockfile(
            &rp("web/pnpm-lock.yaml").token(),
            AcceptSide::Theirs,
            "sleep 30",
            &cancel,
            |_, _| {},
        )
        .unwrap();
    handle.join().unwrap();
    assert!(result.run.cancelled);
    assert!(!result.staged);
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(repo.list_conflicts().unwrap().len(), 1);
}

#[test]
fn a_side_that_deleted_the_lockfile_cannot_be_regenerated_from() {
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "keep");
    s.write_commit("pnpm-lock.yaml", "a: 1\n", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "pnpm-lock.yaml"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.write_commit("pnpm-lock.yaml", "a: 2\n", "main modifies");
    s.expect_conflict(&["merge", "feature"]);
    assert!(s
        .repo()
        .regenerate_lockfile(
            &rp("pnpm-lock.yaml").token(),
            AcceptSide::Theirs,
            "git --version",
            &no_cancel(),
            |_, _| {}
        )
        .is_err());
}
