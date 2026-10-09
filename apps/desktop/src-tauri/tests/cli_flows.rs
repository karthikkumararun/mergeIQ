//! End-to-end CLI flows: request preparation, exit codes, single-instance routing and a
//! real `git mergetool` run against the built `mergeiq` binary. The merge window is replaced
//! by a test host that "clicks" Apply or Cancel through the same code the IPC commands use.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use mergeiq_core::{Analysis, ChunkKind};
use mergeiq_desktop_lib::cli::args::{Request, RequestKind};
use mergeiq_desktop_lib::cli::ipc_socket::{serve, Endpoint, Handler};
use mergeiq_desktop_lib::cli::prepare::{prepare, RequestPrepared, SaveMode};
use mergeiq_desktop_lib::cli::requests::{Dispatcher, Registry, WindowHost, WindowKind};

struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let repo = Self { _dir: dir, root };
        repo.git(&["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "Test User"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", "no-hooks"),
            ("merge.conflictStyle", "merge"),
        ] {
            repo.git(&["config", k, v]);
        }
        repo
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_EDITOR", "true")
            .env("GIT_TERMINAL_PROMPT", "0");
        cmd
    }

    fn git(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(&self, rel: &str, text: &str) {
        std::fs::write(self.root.join(rel), text).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root.join(rel)).unwrap()
    }

    fn commit(&self, rel: &str, text: &str, message: &str) {
        self.write(rel, text);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    /// main and feature both edit a.txt.
    fn diverge(&self) {
        self.commit("a.txt", "one\ntwo\nthree\n", "base");
        self.git(&["checkout", "-q", "-b", "feature"]);
        self.commit("a.txt", "one\nTHEIRS\nthree\n", "feature change");
        self.git(&["checkout", "-q", "main"]);
        self.commit("a.txt", "one\nOURS\nthree\n", "main change");
    }

    fn merge_conflict(&self) {
        self.diverge();
        assert!(!self
            .cmd(&["merge", "feature"])
            .output()
            .unwrap()
            .status
            .success());
    }

    fn rebase_conflict(&self) {
        self.diverge();
        self.git(&["checkout", "-q", "feature"]);
        assert!(!self
            .cmd(&["rebase", "main"])
            .output()
            .unwrap()
            .status
            .success());
    }

    fn unmerged(&self) -> String {
        self.git(&["ls-files", "-u"])
    }
}

fn request(kind: RequestKind, cwd: &Path) -> Request {
    Request {
        kind,
        cwd: cwd.to_path_buf(),
    }
}

fn merge_files(
    dir: &Path,
    base: Option<&str>,
    local: &str,
    remote: &str,
    merged: &str,
) -> RequestKind {
    let write = |name: &str, text: &str| {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        p
    };
    RequestKind::Merge {
        base: match base {
            Some(text) => write("BASE", text),
            None => dir.join("BASE-missing"),
        },
        local: write("LOCAL", local),
        remote: write("REMOTE", remote),
        merged: write("MERGED", merged),
    }
}

fn analysis_of(p: &RequestPrepared) -> &Analysis {
    &p.merge_doc().expect("merge document").analysis
}

// ---- request preparation -------------------------------------------------------------

#[test]
fn add_add_conflict_without_base() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = merge_files(tmp.path(), None, "ours\n", "theirs\n", "marker junk\n");
    let prepared = prepare(&request(kind, tmp.path())).unwrap();
    let analysis = analysis_of(&prepared);
    assert_eq!(analysis.base.text, "");
    assert_eq!(analysis.chunks.len(), 1);
    assert_eq!(analysis.chunks[0].kind, ChunkKind::Conflict);
    // Outside a repository the labels fall back to Local / Remote with file names.
    let labels = &prepared.merge_doc().unwrap().labels;
    assert_eq!(labels.left.role, "Local");
    assert_eq!(labels.right.role, "Remote");
    assert_eq!(labels.left.subject.as_deref(), Some("LOCAL"));
    assert_eq!(labels.right.subject.as_deref(), Some("REMOTE"));
}

#[test]
fn empty_base_file_is_an_empty_base() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = merge_files(tmp.path(), Some(""), "a\n", "b\n", "");
    let prepared = prepare(&request(kind, tmp.path())).unwrap();
    assert_eq!(analysis_of(&prepared).base.text, "");
}

#[test]
fn mergetool_during_rebase_uses_git_labels() {
    let repo = Repo::new();
    repo.rebase_conflict();
    let tmp = tempfile::tempdir().unwrap();
    let stage = |n: u8| repo.git(&["show", &format!(":{n}:a.txt")]) + "\n";
    let kind = {
        let k = merge_files(tmp.path(), Some(&stage(1)), &stage(2), &stage(3), "");
        match k {
            RequestKind::Merge {
                base,
                local,
                remote,
                ..
            } => RequestKind::Merge {
                base,
                local,
                remote,
                merged: repo.root.join("a.txt"),
            },
            other => other,
        }
    };
    let prepared = prepare(&request(kind, &repo.root)).unwrap();
    let doc = prepared.merge_doc().unwrap();
    assert!(
        doc.labels.left.role.starts_with("Upstream"),
        "{:?}",
        doc.labels.left.role
    );
    assert!(doc.labels.left.role.contains("main"));
    assert_eq!(doc.labels.right.subject.as_deref(), Some("feature change"));
    assert_eq!(doc.display_path, "a.txt");
    assert!(doc.context.is_some());
}

#[test]
fn unreadable_local_exits_2() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = RequestKind::Merge {
        base: tmp.path().join("b"),
        local: tmp.path().join("missing"),
        remote: tmp.path().join("r"),
        merged: tmp.path().join("m"),
    };
    let err = prepare(&request(kind, tmp.path())).err().unwrap();
    assert_eq!(err.exit_code, 2);
    assert!(err.message.unwrap().contains("LOCAL"));
}

// ---- exit codes ----------------------------------------------------------------------

#[test]
fn resolved_writes_merged_without_staging_and_exits_0() {
    let repo = Repo::new();
    repo.merge_conflict();
    let kind = RequestKind::Merge {
        base: repo.root.join("a.txt"),
        local: repo.root.join("a.txt"),
        remote: repo.root.join("a.txt"),
        merged: repo.root.join("a.txt"),
    };
    let prepared = prepare(&request(kind, &repo.root)).unwrap();
    let doc = prepared.merge_doc().unwrap();
    let code = prepared
        .save(
            "one\nBOTH\nthree\n",
            &doc.analysis.encoding,
            SaveMode::Resolved,
        )
        .unwrap();
    assert_eq!(code, 0);
    assert_eq!(repo.read("a.txt"), "one\nBOTH\nthree\n");
    // MergeIQ does not stage; git mergetool does that after exit 0.
    assert!(!repo.unmerged().is_empty());
}

#[test]
fn saving_with_markers_exits_1() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = merge_files(tmp.path(), Some("x\n"), "a\n", "b\n", "");
    let prepared = prepare(&request(kind, tmp.path())).unwrap();
    let enc = analysis_of(&prepared).encoding;
    let code = prepared
        .save(
            "<<<<<<< Local\na\n=======\nb\n>>>>>>> Remote\n",
            &enc,
            SaveMode::Markers,
        )
        .unwrap();
    assert_eq!(code, 1);
}

#[test]
fn cancel_leaves_merged_unchanged_and_exits_1() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = merge_files(tmp.path(), Some("x\n"), "a\n", "b\n", "original\n");
    let registry: Registry<RequestPrepared> = Registry::default();
    let prepared = prepare(&request(kind, tmp.path())).unwrap();
    let (id, rx) = registry.register(prepared, WindowKind::Merge);
    assert_eq!(registry.close(id), Some(1));
    assert_eq!(rx.recv().unwrap(), 1);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("MERGED")).unwrap(),
        "original\n"
    );
}

// ---- resolve -------------------------------------------------------------------------

#[test]
fn resolve_unmerged_index_entry_stages_on_save() {
    let repo = Repo::new();
    repo.merge_conflict();
    let prepared = prepare(&request(
        RequestKind::Resolve {
            path: repo.root.join("a.txt"),
        },
        &repo.root,
    ))
    .unwrap();
    let doc = prepared.merge_doc().unwrap();
    assert_eq!(doc.labels.left.ref_name.as_deref(), Some("main"));
    assert_eq!(doc.labels.right.ref_name.as_deref(), Some("feature"));
    prepared
        .save(
            "one\nBOTH\nthree\n",
            &doc.analysis.encoding,
            SaveMode::Resolved,
        )
        .unwrap();
    assert_eq!(repo.unmerged(), "");
    assert_eq!(repo.read("a.txt"), "one\nBOTH\nthree\n");
}

#[test]
fn resolve_with_markers_saved_unresolved_does_not_stage() {
    let repo = Repo::new();
    repo.merge_conflict();
    let prepared = prepare(&request(
        RequestKind::Resolve {
            path: repo.root.join("a.txt"),
        },
        &repo.root,
    ))
    .unwrap();
    let enc = analysis_of(&prepared).encoding;
    let code = prepared
        .save(
            "<<<<<<< a\nx\n=======\ny\n>>>>>>> b\n",
            &enc,
            SaveMode::Markers,
        )
        .unwrap();
    assert_eq!(code, 1);
    assert!(!repo.unmerged().is_empty());
}

#[test]
fn file_with_markers_outside_a_conflicted_index() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("notes.txt");
    std::fs::write(
        &path,
        "intro\n<<<<<<< mine\nours\n||||||| base\nold\n=======\ntheirs\n>>>>>>> yours\noutro\n",
    )
    .unwrap();
    let prepared = prepare(&request(
        RequestKind::Resolve { path: path.clone() },
        tmp.path(),
    ))
    .unwrap();
    let doc = prepared.merge_doc().unwrap();
    assert_eq!(doc.analysis.base.text, "intro\nold\noutro\n");
    assert_eq!(doc.analysis.ours.text, "intro\nours\noutro\n");
    assert_eq!(doc.analysis.theirs.text, "intro\ntheirs\noutro\n");
    assert_eq!(doc.labels.left.ref_name.as_deref(), Some("mine"));
    prepared
        .save(
            "intro\nresolved\noutro\n",
            &doc.analysis.encoding,
            SaveMode::Resolved,
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "intro\nresolved\noutro\n"
    );
}

#[test]
fn resolve_a_file_without_conflicts_exits_2() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("plain.txt");
    std::fs::write(&path, "nothing to see\n").unwrap();
    let err = prepare(&request(
        RequestKind::Resolve { path: path.clone() },
        tmp.path(),
    ))
    .err()
    .unwrap();
    assert_eq!(err.exit_code, 2);
    assert_eq!(
        err.message.unwrap(),
        format!("no conflicts in {}", path.display())
    );
}

// ---- routing + git mergetool ---------------------------------------------------------

/// Stands in for the webview: when a window "opens" it applies a fixed outcome.
struct ScriptedHost {
    registry: Arc<Registry<RequestPrepared>>,
    /// `Some(text)`: resolve with this text; `None`: cancel.
    resolution: Option<String>,
}

impl WindowHost for ScriptedHost {
    fn open(&self, id: u32, _kind: WindowKind, _title: &str) -> Result<(), String> {
        let registry = Arc::clone(&self.registry);
        let resolution = self.resolution.clone();
        std::thread::spawn(move || {
            if let (Some(text), Some(prepared)) = (resolution, registry.get(id)) {
                let enc = prepared.merge_doc().unwrap().analysis.encoding;
                prepared.save(&text, &enc, SaveMode::Resolved).unwrap();
            }
            registry.close(id);
        });
        Ok(())
    }
}

struct Instance {
    _dir: tempfile::TempDir,
    socket_dir: PathBuf,
}

fn start_instance(resolution: Option<&str>) -> Instance {
    let dir = tempfile::Builder::new()
        .prefix("mq")
        .tempdir_in("/tmp")
        .unwrap();
    let socket_dir = dir.path().join("s");
    let endpoint = Endpoint::in_dir(socket_dir.clone());
    let registry = Arc::new(Registry::default());
    let dispatcher = Arc::new(Dispatcher {
        registry: Arc::clone(&registry),
        host: Arc::new(ScriptedHost {
            registry,
            resolution: resolution.map(str::to_string),
        }),
        prepare: Arc::new(prepare),
    });
    serve(endpoint.bind().unwrap(), dispatcher as Arc<dyn Handler>);
    Instance {
        _dir: dir,
        socket_dir,
    }
}

fn mergeiq() -> &'static str {
    env!("CARGO_BIN_EXE_mergeiq")
}

fn configure_mergetool(repo: &Repo) {
    repo.git(&["config", "merge.tool", "mergeiq"]);
    repo.git(&[
        "config",
        "mergetool.mergeiq.cmd",
        &format!(
            "'{}' merge \"$BASE\" \"$LOCAL\" \"$REMOTE\" \"$MERGED\"",
            mergeiq()
        ),
    ]);
    repo.git(&["config", "mergetool.mergeiq.trustExitCode", "true"]);
    repo.git(&["config", "mergetool.keepBackup", "false"]);
}

#[test]
fn invoked_by_git_mergetool() {
    let instance = start_instance(Some("one\nBOTH\nthree\n"));
    let repo = Repo::new();
    repo.merge_conflict();
    configure_mergetool(&repo);
    let out = repo
        .cmd(&["mergetool", "--no-prompt"])
        .env("MERGEIQ_SOCKET_DIR", &instance.socket_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Exit 0 + trustExitCode: git staged the result itself.
    assert_eq!(repo.unmerged(), "");
    assert_eq!(repo.read("a.txt"), "one\nBOTH\nthree\n");
    assert_eq!(repo.git(&["show", ":a.txt"]), "one\nBOTH\nthree");
}

#[test]
fn git_mergetool_sees_cancel_as_failure() {
    let instance = start_instance(None);
    let repo = Repo::new();
    repo.merge_conflict();
    configure_mergetool(&repo);
    let before = repo.read("a.txt");
    let out = repo
        .cmd(&["mergetool", "--no-prompt"])
        .env("MERGEIQ_SOCKET_DIR", &instance.socket_dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!repo.unmerged().is_empty());
    assert_eq!(repo.read("a.txt"), before);
}

#[test]
fn two_mergetool_calls_share_one_instance() {
    let instance = start_instance(Some("merged\n"));
    let tmp = tempfile::tempdir().unwrap();
    for n in 0..2 {
        let dir = tmp.path().join(n.to_string());
        std::fs::create_dir(&dir).unwrap();
        let RequestKind::Merge {
            base,
            local,
            remote,
            merged,
        } = merge_files(&dir, Some("x\n"), "a\n", "b\n", "")
        else {
            unreachable!()
        };
        let out = Command::new(mergeiq())
            .arg("merge")
            .args([&base, &local, &remote, &merged])
            .env("MERGEIQ_SOCKET_DIR", &instance.socket_dir)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(std::fs::read_to_string(merged).unwrap(), "merged\n");
    }
}

#[test]
fn resolve_without_conflicts_via_running_instance_prints_message() {
    let instance = start_instance(None);
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("plain.txt");
    std::fs::write(&path, "x\n").unwrap();
    let out = Command::new(mergeiq())
        .arg("resolve")
        .arg(&path)
        .env("MERGEIQ_SOCKET_DIR", &instance.socket_dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no conflicts in"));
}

#[test]
fn invalid_arguments_exit_2_from_the_binary() {
    let out = Command::new(mergeiq())
        .args(["merge", "only", "two"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Usage"));
}
