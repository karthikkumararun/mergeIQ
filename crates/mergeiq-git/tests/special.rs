//! Resolving special conflicts (binary, LFS, symlink, submodule, rename, modify/delete)
//! against real repositories.

mod support;

use mergeiq_git::{
    AcceptSide, ConflictClass, ConflictType, RenameSide, RepoPath, SubmoduleRelation,
};
use support::Scenario;

fn rp(s: &str) -> RepoPath {
    RepoPath::from_bytes(s.as_bytes().to_vec())
}

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

fn staged(s: &Scenario, rel: &str) -> (String, String, String) {
    // (stage, mode, oid) of the single index entry
    let out = s.git(&["ls-files", "-s", "--", rel]);
    let mut lines = out.lines();
    let first = lines.next().expect("path is in the index");
    assert!(lines.next().is_none(), "still conflicted: {out}");
    let mut parts = first.split_whitespace();
    let mode = parts.next().unwrap().to_string();
    let oid = parts.next().unwrap().to_string();
    let stage = parts.next().unwrap().to_string();
    (stage, mode, oid)
}

fn blob_oid(s: &Scenario, bytes: &[u8]) -> String {
    let tmp = s.path("blob.tmp");
    std::fs::write(&tmp, bytes).unwrap();
    let oid = s.git(&["hash-object", "--no-filters", "blob.tmp"]);
    std::fs::remove_file(tmp).unwrap();
    oid
}

const PNG_BASE: &[u8] = b"\x89PNG\r\n\x1a\n\0\0base\r\n";
const PNG_OURS: &[u8] = b"\x89PNG\r\n\x1a\n\0\0ours!\r\n";
const PNG_THEIRS: &[u8] = b"\x89PNG\r\n\x1a\n\0\0theirs\r\n";

#[test]
fn use_right_writes_the_right_blob_bytes_exactly() {
    let s = Scenario::new();
    // CRLF conversion must not touch binary content.
    s.git(&["config", "core.autocrlf", "true"]);
    both_modify(&s, "logo.png", PNG_BASE, PNG_OURS, PNG_THEIRS);
    let repo = s.repo();
    repo.use_side_exact(&rp("logo.png"), AcceptSide::Theirs)
        .unwrap();
    assert_eq!(s.read("logo.png"), PNG_THEIRS);
    let (stage, mode, oid) = staged(&s, "logo.png");
    assert_eq!((stage.as_str(), mode.as_str()), ("0", "100644"));
    assert_eq!(oid, blob_oid(&s, PNG_THEIRS));
    assert!(repo.list_conflicts().unwrap().is_empty());
}

#[test]
fn use_left_keeps_the_left_blob() {
    let s = Scenario::new();
    both_modify(&s, "logo.png", PNG_BASE, PNG_OURS, PNG_THEIRS);
    s.repo()
        .use_side_exact(&rp("logo.png"), AcceptSide::Ours)
        .unwrap();
    assert_eq!(s.read("logo.png"), PNG_OURS);
    assert_eq!(staged(&s, "logo.png").2, blob_oid(&s, PNG_OURS));
}

#[test]
fn an_executable_keeps_its_mode() {
    let s = Scenario::new();
    let commit_exec = |content: &str, message: &str| {
        s.write("run.bin", content);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(s.path("run.bin"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        s.git(&["add", "run.bin"]);
        s.git(&["update-index", "--chmod=+x", "run.bin"]);
        s.git(&["commit", "-q", "-m", message]);
    };
    commit_exec("\0base", "base");
    s.checkout_new("feature");
    commit_exec("\0theirs", "feature");
    s.checkout("main");
    commit_exec("\0ours", "main");
    s.expect_conflict(&["merge", "feature"]);
    s.repo()
        .use_side_exact(&rp("run.bin"), AcceptSide::Theirs)
        .unwrap();
    assert_eq!(staged(&s, "run.bin").1, "100755");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(s.path("run.bin"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }
}

#[test]
fn lfs_pick_stages_the_pointer_exactly() {
    let s = Scenario::new();
    let pointer = |hex: char, size: u32| {
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {size}\n",
            hex.to_string().repeat(64)
        )
    };
    both_modify(
        &s,
        "model.bin",
        pointer('a', 1).as_bytes(),
        pointer('b', 2).as_bytes(),
        pointer('c', 3).as_bytes(),
    );
    let repo = s.repo();
    let details = repo.conflict_details(&rp("model.bin").token()).unwrap();
    assert_eq!(details.entry.class, ConflictClass::LfsPointer);
    let ours = details.stages.iter().find(|m| m.stage == 2).unwrap();
    assert_eq!(ours.lfs.as_ref().unwrap().size, 2);
    assert!(ours.lfs.as_ref().unwrap().oid.starts_with("sha256:bbbb"));
    repo.use_side_exact(&rp("model.bin"), AcceptSide::Ours)
        .unwrap();
    assert_eq!(s.read("model.bin"), pointer('b', 2).as_bytes());
    assert_eq!(
        staged(&s, "model.bin").2,
        blob_oid(&s, pointer('b', 2).as_bytes())
    );
}

#[cfg(unix)]
fn symlink_conflict(s: &Scenario) {
    use std::os::unix::fs::symlink;
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
}

#[cfg(unix)]
#[test]
fn symlink_targets_differ_and_use_right_points_at_the_right_target() {
    let s = Scenario::new();
    symlink_conflict(&s);
    let repo = s.repo();
    let details = repo.conflict_details(&rp("link").token()).unwrap();
    let target = |stage: u8| {
        details
            .stages
            .iter()
            .find(|m| m.stage == stage)
            .and_then(|m| m.symlink_target.clone())
    };
    assert_eq!(target(1).as_deref(), Some("../base"));
    assert_eq!(target(2).as_deref(), Some("../a"));
    assert_eq!(target(3).as_deref(), Some("../b"));

    repo.use_side_exact(&rp("link"), AcceptSide::Theirs)
        .unwrap();
    assert_eq!(
        std::fs::read_link(s.path("link")).unwrap().to_str(),
        Some("../b")
    );
    let (stage, mode, _) = staged(&s, "link");
    assert_eq!((stage.as_str(), mode.as_str()), ("0", "120000"));
}

#[cfg(unix)]
#[test]
fn without_symlink_support_the_target_becomes_file_content() {
    let s = Scenario::new();
    symlink_conflict(&s);
    s.git(&["config", "core.symlinks", "false"]);
    s.repo()
        .use_side_exact(&rp("link"), AcceptSide::Theirs)
        .unwrap();
    let meta = std::fs::symlink_metadata(s.path("link")).unwrap();
    assert!(meta.is_file() && !meta.file_type().is_symlink());
    assert_eq!(s.read("link"), b"../b");
    // Still recorded as a symlink in the index.
    assert_eq!(staged(&s, "link").1, "120000");
}

#[test]
fn modify_delete_view_shows_the_surviving_side_against_the_base() {
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "keep");
    s.write_commit("a.txt", "one\ntwo\nthree\n", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.write_commit("a.txt", "one\nTWO\nthree\nfour\n", "main modifies");
    s.expect_conflict(&["merge", "feature"]);
    let repo = s.repo();
    let list = repo.list_conflicts().unwrap();
    assert_eq!(list[0].conflict_type, ConflictType::DeletedByThem);
    let view = repo.modify_delete_view(&rp("a.txt").token()).unwrap();
    assert_eq!(view.deleted_by, RenameSide::Theirs);
    assert_eq!(view.base_text.as_deref(), Some("one\ntwo\nthree\n"));
    assert_eq!(
        view.survivor_text.as_deref(),
        Some("one\nTWO\nthree\nfour\n")
    );
    assert_eq!(view.hunks.len(), 2);
    assert!(view.note.is_none());
}

#[test]
fn modify_delete_view_reports_binary_and_rejects_other_types() {
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "keep");
    s.write_commit("img.png", "\0base", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "img.png"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.write_commit("img.png", "\0changed", "main modifies");
    s.expect_conflict(&["merge", "feature"]);
    let view = s.repo().modify_delete_view(&rp("img.png").token()).unwrap();
    assert_eq!(view.note.as_deref(), Some("the file is binary"));
    assert!(view.hunks.is_empty());

    let t = Scenario::new();
    t.merge_conflict();
    assert!(t.repo().modify_delete_view(&rp("a.txt").token()).is_err());
}

#[test]
fn delete_chosen_removes_the_file_from_tree_and_index() {
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "keep");
    s.write_commit("a.txt", "one\n", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.write_commit("a.txt", "two\n", "main modifies");
    s.expect_conflict(&["merge", "feature"]);
    let repo = s.repo();
    repo.delete_resolved(&rp("a.txt")).unwrap();
    assert!(!s.path("a.txt").exists());
    assert!(repo.list_conflicts().unwrap().is_empty());
    assert!(s.git(&["ls-files", "--", "a.txt"]).is_empty());
}

#[test]
fn keep_and_edit_writes_the_survivor_without_staging() {
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "keep");
    s.write_commit("a.txt", "one\n", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.write_commit("a.txt", "two\n", "main modifies");
    s.expect_conflict(&["merge", "feature"]);
    let repo = s.repo();
    // Simulate the file having been removed from the working tree.
    std::fs::remove_file(s.path("a.txt")).ok();
    repo.write_side_unstaged(&rp("a.txt"), AcceptSide::Ours)
        .unwrap();
    assert_eq!(s.read("a.txt"), b"two\n");
    // Still conflicted.
    assert_eq!(repo.list_conflicts().unwrap().len(), 1);
    // Taking the deleting side's stage fails: there is nothing to write.
    assert!(repo
        .write_side_unstaged(&rp("a.txt"), AcceptSide::Theirs)
        .is_err());
}

#[test]
fn choosing_the_deleting_side_removes_the_file() {
    let s = Scenario::new();
    s.write_commit("keep.txt", "k\n", "keep");
    s.write_commit("a.txt", "one\n", "base");
    s.checkout_new("feature");
    s.git(&["rm", "-q", "a.txt"]);
    s.commit_all("feature deletes");
    s.checkout("main");
    s.write_commit("a.txt", "two\n", "main modifies");
    s.expect_conflict(&["merge", "feature"]);
    s.repo()
        .use_side_exact(&rp("a.txt"), AcceptSide::Theirs)
        .unwrap();
    assert!(!s.path("a.txt").exists());
    assert!(s.repo().list_conflicts().unwrap().is_empty());
}

#[test]
fn stage_blob_returns_base64_and_mime() {
    use base64::Engine;
    let s = Scenario::new();
    both_modify(&s, "logo.png", PNG_BASE, PNG_OURS, PNG_THEIRS);
    let blob = s.repo().stage_blob(&rp("logo.png").token(), 3).unwrap();
    assert_eq!(blob.mime, "image/png");
    assert_eq!(blob.size, PNG_THEIRS.len() as u64);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&blob.base64)
        .unwrap();
    assert_eq!(bytes, PNG_THEIRS);
    assert!(s.repo().stage_blob(&rp("logo.png").token(), 4).is_err());
}

#[test]
fn stage_blob_refuses_files_over_the_preview_cap() {
    let s = Scenario::new();
    let big = |tag: &str| {
        let mut v = format!("{tag}\0").into_bytes();
        v.extend(std::iter::repeat_n(b'x', 21 * 1024 * 1024));
        v
    };
    both_modify(&s, "huge.png", &big("base"), &big("ours"), &big("theirs"));
    let err = s.repo().stage_blob(&rp("huge.png").token(), 2).unwrap_err();
    assert!(err.to_string().contains("larger than"), "{err}");
}

// ---- renames ----

const BODY: &str = "line one\nline two\nline three\nline four\nline five\nline six\n";

fn rename_rename(s: &Scenario, ours_edit: &str, theirs_edit: &str) {
    s.write_commit("a.ts", BODY, "base");
    s.checkout_new("feature");
    s.git(&["mv", "a.ts", "c.ts"]);
    s.write("c.ts", format!("{BODY}{theirs_edit}"));
    s.commit_all("feature renames");
    s.checkout("main");
    s.git(&["mv", "a.ts", "b.ts"]);
    s.write("b.ts", format!("{BODY}{ours_edit}"));
    s.commit_all("main renames");
    s.expect_conflict(&["merge", "feature"]);
}

#[test]
fn rename_rename_same_content_keeps_only_the_chosen_path_staged() {
    let s = Scenario::new();
    rename_rename(&s, "", "");
    let repo = s.repo();
    let pair = repo.rename_pair_of(&rp("b.ts")).unwrap().unwrap();
    assert!(!pair.contents_differ);
    let out = repo.choose_rename_path(&rp("c.ts")).unwrap();
    assert!(!out.needs_merge);
    assert!(s.path("c.ts").exists());
    assert!(!s.path("b.ts").exists() && !s.path("a.ts").exists());
    assert!(repo.list_conflicts().unwrap().is_empty());
    assert_eq!(s.git(&["ls-files"]), "c.ts");
    assert_eq!(s.read("c.ts"), BODY.as_bytes());
}

#[test]
fn rename_rename_with_different_content_becomes_a_text_conflict() {
    let s = Scenario::new();
    rename_rename(&s, "OURS edit\n", "THEIRS edit\n");
    let repo = s.repo();
    assert!(
        repo.rename_pair_of(&rp("c.ts"))
            .unwrap()
            .unwrap()
            .contents_differ
    );
    let out = repo.choose_rename_path(&rp("c.ts")).unwrap();
    assert!(out.needs_merge);
    let list = repo.list_conflicts().unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0].display, "c.ts");
    assert_eq!(list[0].conflict_type, ConflictType::BothModified);
    assert_eq!(list[0].class, ConflictClass::Text);
    assert!(!s.path("b.ts").exists() && !s.path("a.ts").exists());
    // The merge editor gets real ours/theirs/base content.
    let load = repo
        .load_conflict(&rp("c.ts").token(), &mergeiq_core::Options::default())
        .unwrap();
    let a = load.analysis.unwrap();
    assert!(a.ours.text.ends_with("OURS edit\n"), "{}", a.ours.text);
    assert!(a.theirs.text.ends_with("THEIRS edit\n"));
    assert_eq!(a.base.text, BODY);
}

#[test]
fn rename_choice_rejects_other_paths() {
    let s = Scenario::new();
    rename_rename(&s, "", "");
    assert!(s.repo().choose_rename_path(&rp("a.ts")).is_err());
    assert!(s.repo().choose_rename_path(&rp("nope.ts")).is_err());
}

// ---- submodules ----

/// A submodule `lib` (commits c1, c2, c3 in a sibling repo `sublib`) added in a base commit.
struct Sub {
    c: [String; 3],
}

fn submodule_host(s: &Scenario) -> Sub {
    let sub = s.root.parent().unwrap().join("sublib");
    std::fs::create_dir_all(&sub).unwrap();
    s.git_in(&sub, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "Sub"), ("user.email", "sub@example.com")] {
        s.git_in(&sub, &["config", k, v]);
    }
    let mut c = Vec::new();
    for (i, text) in ["1", "2", "3"].iter().enumerate() {
        std::fs::write(sub.join("f"), text).unwrap();
        s.git_in(&sub, &["add", "-A"]);
        s.git_in(
            &sub,
            &["commit", "-q", "-m", &format!("sub commit {}", i + 1)],
        );
        c.push(s.git_in(&sub, &["rev-parse", "HEAD"]));
        if i == 0 {
            s.write("README", "host");
            s.commit_all("host base");
            s.git(&[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                sub.to_str().unwrap(),
                "lib",
            ]);
            s.commit_all("add submodule");
        }
    }
    s.git_in(&s.path("lib"), &["fetch", "-q", "origin"]);
    Sub {
        c: [c[0].clone(), c[1].clone(), c[2].clone()],
    }
}

/// Replaces `path`'s index entries by gitlink stages 1, 2, 3 (what a submodule conflict looks like).
fn fabricate_gitlink_conflict(s: &Scenario, shas: [&str; 3]) {
    s.git(&["update-index", "--force-remove", "lib"]);
    let input: String = shas
        .iter()
        .enumerate()
        .map(|(i, sha)| format!("160000 {sha} {}\tlib\n", i + 1))
        .collect();
    let mut child = std::process::Command::new("git")
        .current_dir(&s.root)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), input.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn diverged_submodules_conflict_and_are_reported_as_such() {
    let s = Scenario::new();
    let sub = submodule_host(&s);
    // Two commits on different lines of history in the submodule.
    let lib = s.path("lib");
    s.git_in(&lib, &["checkout", "-q", &sub.c[0]]);
    std::fs::write(lib.join("f"), "side-a").unwrap();
    s.git_in(
        &lib,
        &[
            "-c",
            "user.name=A",
            "-c",
            "user.email=a@x",
            "commit",
            "-qam",
            "side a",
        ],
    );
    let a = s.git_in(&lib, &["rev-parse", "HEAD"]);
    s.git_in(&lib, &["checkout", "-q", &sub.c[0]]);
    std::fs::write(lib.join("f"), "side-b").unwrap();
    s.git_in(
        &lib,
        &[
            "-c",
            "user.name=B",
            "-c",
            "user.email=b@x",
            "commit",
            "-qam",
            "side b",
        ],
    );
    let b = s.git_in(&lib, &["rev-parse", "HEAD"]);
    s.checkout_new("feature");
    s.git_in(&lib, &["checkout", "-q", &b]);
    s.git(&["add", "lib"]);
    s.commit_all("feature");
    s.checkout("main");
    s.git_in(&lib, &["checkout", "-q", &a]);
    s.git(&["add", "lib"]);
    s.commit_all("main");
    s.expect_conflict(&["merge", "feature"]);

    let repo = s.repo();
    let entry = repo.list_conflicts().unwrap().remove(0);
    assert_eq!(entry.class, ConflictClass::Submodule);
    let d = repo.submodule_details(&rp("lib").token()).unwrap();
    assert!(d.checked_out);
    assert_eq!(d.relation, SubmoduleRelation::Diverged);
    assert_eq!(d.left.as_ref().unwrap().subject.as_deref(), Some("side a"));
    assert_eq!(d.right.as_ref().unwrap().subject.as_deref(), Some("side b"));
    assert_eq!(d.base.as_ref().unwrap().sha, sub.c[0]);
}

#[test]
fn fast_forwardable_submodule_recommends_the_descendant() {
    let s = Scenario::new();
    let sub = submodule_host(&s);
    fabricate_gitlink_conflict(&s, [&sub.c[0], &sub.c[1], &sub.c[2]]);
    let repo = s.repo();
    let d = repo.submodule_details(&rp("lib").token()).unwrap();
    assert!(d.checked_out);
    assert_eq!(d.relation, SubmoduleRelation::LeftAncestorOfRight);
    assert_eq!(
        d.right.as_ref().unwrap().subject.as_deref(),
        Some("sub commit 3")
    );
    assert!(d.right.as_ref().unwrap().date.is_some());

    fabricate_gitlink_conflict(&s, [&sub.c[0], &sub.c[2], &sub.c[1]]);
    assert_eq!(
        s.repo()
            .submodule_details(&rp("lib").token())
            .unwrap()
            .relation,
        SubmoduleRelation::RightAncestorOfLeft
    );
}

#[test]
fn use_right_updates_the_gitlink_with_update_index() {
    let s = Scenario::new();
    let sub = submodule_host(&s);
    fabricate_gitlink_conflict(&s, [&sub.c[0], &sub.c[1], &sub.c[2]]);
    s.repo()
        .use_side_exact(&rp("lib"), AcceptSide::Theirs)
        .unwrap();
    let (stage, mode, oid) = staged(&s, "lib");
    assert_eq!(
        (stage.as_str(), mode.as_str(), oid.as_str()),
        ("0", "160000", sub.c[2].as_str())
    );
    assert!(s.repo().list_conflicts().unwrap().is_empty());
    assert!(
        s.path("lib").is_dir(),
        "the submodule working tree is untouched"
    );
}

#[test]
fn a_submodule_that_is_not_checked_out_has_unknown_ancestry() {
    let s = Scenario::new();
    let sub = submodule_host(&s);
    fabricate_gitlink_conflict(&s, [&sub.c[0], &sub.c[1], &sub.c[2]]);
    std::fs::remove_dir_all(s.path("lib")).unwrap();
    std::fs::create_dir(s.path("lib")).unwrap();
    let d = s.repo().submodule_details(&rp("lib").token()).unwrap();
    assert!(!d.checked_out);
    assert_eq!(d.relation, SubmoduleRelation::Unknown);
    // SHAs are still known, subjects are not.
    assert_eq!(d.left.as_ref().unwrap().sha, sub.c[1]);
    assert_eq!(d.left.as_ref().unwrap().subject, None);
}
