//! Classification of conflicted paths against real repositories.

mod support;

use mergeiq_git::{ConflictClass, LockfileKind, OVERSIZED_BYTES};
use support::Scenario;

/// Both branches rewrite `rel` differently from a common base; leaves a conflicted merge.
fn both_modify(s: &Scenario, rel: &str, base: &[u8], ours: &[u8], theirs: &[u8]) {
    s.write(rel, base);
    s.commit_all("base");
    s.checkout_new("feature");
    s.write(rel, theirs);
    s.commit_all("feature change");
    s.checkout("main");
    s.write(rel, ours);
    s.commit_all("main change");
    s.expect_conflict(&["merge", "feature"]);
}

fn class_of(s: &Scenario, rel: &str) -> ConflictClass {
    let repo = s.repo();
    let list = repo.list_conflicts().unwrap();
    list.iter()
        .find(|e| e.display == rel)
        .unwrap_or_else(|| panic!("{rel} not conflicted: {list:?}"))
        .class
}

#[test]
fn text_is_text() {
    let s = Scenario::new();
    s.merge_conflict();
    assert_eq!(class_of(&s, "a.txt"), ConflictClass::Text);
}

#[test]
fn png_conflict_is_a_binary_image() {
    let s = Scenario::new();
    both_modify(
        &s,
        "logo.png",
        b"\x89PNG\r\n\x1a\n\0\0base",
        b"\x89PNG\r\n\x1a\n\0\0ours!",
        b"\x89PNG\r\n\x1a\n\0\0theirs",
    );
    assert_eq!(
        class_of(&s, "logo.png"),
        ConflictClass::Binary { is_image: true }
    );
}

#[test]
fn other_binary_is_not_an_image() {
    let s = Scenario::new();
    both_modify(&s, "data.bin", b"\0base", b"\0ours", b"\0theirs");
    assert_eq!(
        class_of(&s, "data.bin"),
        ConflictClass::Binary { is_image: false }
    );
}

#[test]
fn pnpm_lockfile_is_a_lockfile() {
    let s = Scenario::new();
    both_modify(
        &s,
        "web/pnpm-lock.yaml",
        b"lockfileVersion: 9\na: 1\n",
        b"lockfileVersion: 9\na: 2\n",
        b"lockfileVersion: 9\na: 3\n",
    );
    assert_eq!(
        class_of(&s, "web/pnpm-lock.yaml"),
        ConflictClass::Lockfile {
            kind: LockfileKind::Pnpm
        }
    );
}

#[test]
fn lfs_pointer_conflict() {
    let s = Scenario::new();
    let pointer = |hex: char, size: u32| {
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {size}\n",
            hex.to_string().repeat(64)
        )
    };
    both_modify(
        &s,
        "assets/model.bin",
        pointer('a', 10).as_bytes(),
        pointer('b', 20).as_bytes(),
        pointer('c', 30).as_bytes(),
    );
    assert_eq!(class_of(&s, "assets/model.bin"), ConflictClass::LfsPointer);
}

#[cfg(unix)]
#[test]
fn symlink_conflict() {
    use std::os::unix::fs::symlink;
    let s = Scenario::new();
    s.write("README", "x");
    let link = s.path("link");
    symlink("../base", &link).unwrap();
    s.commit_all("base");
    s.checkout_new("feature");
    std::fs::remove_file(&link).unwrap();
    symlink("../b", &link).unwrap();
    s.commit_all("feature link");
    s.checkout("main");
    std::fs::remove_file(&link).unwrap();
    symlink("../a", &link).unwrap();
    s.commit_all("main link");
    s.expect_conflict(&["merge", "feature"]);
    assert_eq!(class_of(&s, "link"), ConflictClass::Symlink);
}

#[test]
fn oversized_text_conflict() {
    let s = Scenario::new();
    // ~21 MB of distinct-enough lines; compresses well, so the repo stays small.
    let big = |tag: &str| {
        let line = format!("{tag}: some repeated csv row,1,2,3,4,5\n");
        line.repeat((OVERSIZED_BYTES as usize + 1024 * 1024) / line.len() + 1)
    };
    both_modify(
        &s,
        "data.csv",
        big("base").as_bytes(),
        big("ours").as_bytes(),
        big("theirs").as_bytes(),
    );
    assert_eq!(class_of(&s, "data.csv"), ConflictClass::Oversized);
}

#[test]
fn classification_is_cached_by_object_id() {
    let s = Scenario::new();
    s.merge_conflict();
    let repo = s.repo();
    let first = repo.list_conflicts().unwrap();
    let second = repo.list_conflicts().unwrap();
    assert_eq!(first, second);
}

#[test]
fn many_files_are_classified_in_one_listing() {
    let s = Scenario::new();
    for i in 0..30 {
        s.write(&format!("f{i:02}.txt"), "base\n");
    }
    s.commit_all("base");
    s.checkout_new("feature");
    for i in 0..30 {
        s.write(&format!("f{i:02}.txt"), "theirs\n");
    }
    s.commit_all("feature");
    s.checkout("main");
    for i in 0..30 {
        s.write(&format!("f{i:02}.txt"), "ours\n");
    }
    s.commit_all("main");
    s.expect_conflict(&["merge", "feature"]);
    let list = s.repo().list_conflicts().unwrap();
    assert_eq!(list.len(), 30);
    assert!(list.iter().all(|e| e.class == ConflictClass::Text));
}
