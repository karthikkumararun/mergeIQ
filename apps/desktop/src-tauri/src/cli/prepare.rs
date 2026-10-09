//! Turns a CLI [`Request`] into something a window can show: reads the files, runs the
//! merge engine and (inside a repository) asks `mergeiq-git` for labels and context.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergeiq_core::{
    analyze, decode, parse_markers, Analysis, EncodingInfo, MergeInput, Options, Side,
    WhitespacePolicy,
};
use mergeiq_git::{write_file_atomic, FileContext, GitExec, PathToken, Repo, RepoPath, SideLabel};

use super::args::{Request, RequestKind};
use super::ipc_socket::Response;
use super::requests::{Prepared, WindowKind};

/// Exit code: the user saved a fully resolved result.
pub const EXIT_RESOLVED: i32 = 0;
/// Exit code: cancelled, or saved with conflict markers.
pub const EXIT_UNRESOLVED: i32 = 1;
/// Exit code: the request itself was invalid.
pub const EXIT_INVALID: i32 = 2;

/// How the editor's Apply was chosen (mirrors the UI's `SaveMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SaveMode {
    /// Everything resolved.
    Resolved,
    /// Unresolved conflicts written as markers.
    Markers,
    /// "Mark as resolved anyway".
    Force,
}

/// Left/right labels for the editor headers.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MergeLabels {
    /// Left pane (ours / local).
    pub left: SideLabel,
    /// Right pane (theirs / remote).
    pub right: SideLabel,
}

/// Everything the merge window needs (the UI's `MergeDocument`, minus the path token).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestDoc {
    /// Path shown in the title bar.
    pub display_path: String,
    /// Engine analysis of the three sides.
    pub analysis: Analysis,
    /// Header labels.
    pub labels: MergeLabels,
    /// Commits touching the file per side, when in a repository.
    pub context: Option<FileContext>,
}

enum Source {
    Bytes {
        base: Vec<u8>,
        ours: Vec<u8>,
        theirs: Vec<u8>,
        /// Encoding to keep when the texts are re-encoded for analysis (marker files).
        encoding: Option<EncodingInfo>,
    },
    Index {
        repo: Repo,
        token: PathToken,
    },
}

enum Target {
    /// Write the file only (git mergetool's MERGED; resolve on a marker file).
    File(PathBuf),
    /// Write and stage through the repository.
    Index { repo: Repo, path: RepoPath },
}

/// A prepared request, held by the registry while its window is open.
pub struct RequestPrepared {
    kind: WindowKind,
    title: String,
    merge: Option<MergeRequestDoc>,
    source: Option<Source>,
    target: Target,
    outcome: Mutex<Option<i32>>,
}

impl Prepared for RequestPrepared {
    fn kind(&self) -> WindowKind {
        self.kind
    }
    fn title(&self) -> String {
        self.title.clone()
    }
    fn outcome(&self) -> Option<i32> {
        *self.outcome.lock().expect("outcome lock")
    }
}

fn invalid(message: impl Into<String>) -> Response {
    Response::new(EXIT_INVALID, Some(message.into()))
}

fn configured_git() -> Option<PathBuf> {
    crate::settings::load()
        .extra
        .get("gitPath")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}

fn open_repo(near: &Path) -> Option<Repo> {
    let dir = near.parent()?;
    let exec = GitExec::locate(configured_git().as_deref()).ok()?;
    Repo::open(exec, dir).ok()
}

/// `path` relative to the repository root, as a [`RepoPath`] (forward slashes).
fn repo_relative(repo: &Repo, path: &Path) -> Option<RepoPath> {
    let canonical = path.canonicalize().ok()?;
    let root = repo.root().canonicalize().ok()?;
    let rel = canonical.strip_prefix(root).ok()?;
    let text = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    Some(RepoPath::from_bytes(text.into_bytes()))
}

fn plain_label(role: &str, file: &Path, ref_name: Option<String>) -> SideLabel {
    SideLabel {
        role: role.to_string(),
        ref_name,
        short_sha: None,
        subject: file.file_name().map(|n| n.to_string_lossy().into_owned()),
        author: None,
        git_term: if role == "Local" { "ours" } else { "theirs" }.to_string(),
    }
}

fn read_required(path: &Path, what: &str) -> Result<Vec<u8>, Response> {
    std::fs::read(path).map_err(|e| invalid(format!("cannot read {what} {}: {e}", path.display())))
}

fn run_analysis(
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    encoding: Option<EncodingInfo>,
    opts: &Options,
) -> Result<Analysis, String> {
    let mut analysis =
        analyze(MergeInput { base, ours, theirs }, opts).map_err(|e| e.to_string())?;
    if let Some(encoding) = encoding {
        analysis.encoding = encoding;
    }
    Ok(analysis)
}

/// Checks that `dir` can be opened as a repository window (exit 2 with a message if not).
pub fn check_open(dir: &Path) -> Result<(), Response> {
    let exec = GitExec::locate(configured_git().as_deref())
        .map_err(|e| invalid(format!("mergeiq: {e}")))?;
    Repo::open(exec, dir)
        .map(|_| ())
        .map_err(|e| invalid(format!("mergeiq: {}: {e}", dir.display())))
}

/// Prepares `request`, or returns the response (exit 2 + message) explaining why not.
pub fn prepare(request: &Request) -> Result<RequestPrepared, Response> {
    match &request.kind {
        RequestKind::Merge {
            base,
            local,
            remote,
            merged,
        } => prepare_merge(base, local, remote, merged),
        RequestKind::Resolve { path } => prepare_resolve(path),
        RequestKind::Open { .. } => Err(invalid(
            "open requests open a repository window and have no merge document",
        )),
    }
}

fn prepare_merge(
    base: &Path,
    local: &Path,
    remote: &Path,
    merged: &Path,
) -> Result<RequestPrepared, Response> {
    // A missing BASE (add/add conflicts) is an empty base, as is an empty file.
    let base_bytes = std::fs::read(base).unwrap_or_default();
    let ours = read_required(local, "LOCAL")?;
    let theirs = read_required(remote, "REMOTE")?;
    let analysis = run_analysis(&base_bytes, &ours, &theirs, None, &Options::default())
        .map_err(|e| invalid(format!("cannot merge {}: {e}", merged.display())))?;

    let mut display_path = merged.display().to_string();
    let mut context = None;
    let mut labels = MergeLabels {
        left: plain_label("Local", local, None),
        right: plain_label("Remote", remote, None),
    };
    if let Some(repo) = open_repo(merged) {
        if let Some(rel) = repo_relative(&repo, merged) {
            display_path = rel.display();
            if let Ok(operation) = repo.operation() {
                if operation != mergeiq_git::Operation::None {
                    if let Ok(side) = repo.side_labels(&operation) {
                        labels = MergeLabels {
                            left: side.ours,
                            right: side.theirs,
                        };
                        context = repo.file_context(&rel, &operation).ok();
                    }
                }
            }
        }
    }
    Ok(RequestPrepared {
        kind: WindowKind::Merge,
        title: format!("Merge — {display_path}"),
        merge: Some(MergeRequestDoc {
            display_path,
            analysis,
            labels,
            context,
        }),
        source: Some(Source::Bytes {
            base: base_bytes,
            ours,
            theirs,
            encoding: None,
        }),
        target: Target::File(merged.to_path_buf()),
        outcome: Mutex::new(None),
    })
}

fn prepare_resolve(path: &Path) -> Result<RequestPrepared, Response> {
    if !path.is_file() {
        return Err(invalid(format!("no such file: {}", path.display())));
    }
    if let Some(repo) = open_repo(path) {
        if let Some(rel) = repo_relative(&repo, path) {
            let token = rel.token();
            let is_unmerged = repo
                .list_conflicts()
                .map(|list| list.iter().any(|e| e.path == token))
                .unwrap_or(false);
            if is_unmerged {
                let load = repo
                    .load_conflict(&token, &Options::default())
                    .map_err(|e| invalid(e.to_string()))?;
                let Some(analysis) = load.analysis else {
                    return Err(invalid(format!(
                        "cannot merge {}: {}",
                        rel.display(),
                        load.analysis_error.unwrap_or_default()
                    )));
                };
                return Ok(RequestPrepared {
                    kind: WindowKind::Merge,
                    title: format!("Merge — {}", rel.display()),
                    merge: Some(MergeRequestDoc {
                        display_path: rel.display(),
                        analysis,
                        labels: MergeLabels {
                            left: load.labels.ours,
                            right: load.labels.theirs,
                        },
                        context: Some(load.context),
                    }),
                    source: Some(Source::Index {
                        repo: repo.clone(),
                        token,
                    }),
                    target: Target::Index { repo, path: rel },
                    outcome: Mutex::new(None),
                });
            }
        }
    }
    prepare_markers(path)
}

fn prepare_markers(path: &Path) -> Result<RequestPrepared, Response> {
    let bytes = read_required(path, "file")?;
    let no_conflicts = || invalid(format!("no conflicts in {}", path.display()));
    let parse = parse_markers(&bytes).map_err(|_| no_conflicts())?;
    if parse.regions.is_empty() {
        return Err(no_conflicts());
    }
    let (_, encoding) = decode(&bytes, Side::Ours).map_err(|_| no_conflicts())?;
    let (base, ours, theirs) = (
        parse.base.into_bytes(),
        parse.ours.into_bytes(),
        parse.theirs.into_bytes(),
    );
    let analysis = run_analysis(&base, &ours, &theirs, Some(encoding), &Options::default())
        .map_err(|e| invalid(format!("cannot merge {}: {e}", path.display())))?;
    let label = |role: &str, name: &Option<String>| plain_label(role, path, name.clone());
    let first = &parse.regions[0];
    let display_path = path.display().to_string();
    Ok(RequestPrepared {
        kind: WindowKind::Merge,
        title: format!("Merge — {display_path}"),
        merge: Some(MergeRequestDoc {
            display_path,
            analysis,
            labels: MergeLabels {
                left: label("Local", &first.ours_label),
                right: label("Remote", &first.theirs_label),
            },
            context: None,
        }),
        source: Some(Source::Bytes {
            base,
            ours,
            theirs,
            encoding: Some(encoding),
        }),
        target: Target::File(path.to_path_buf()),
        outcome: Mutex::new(None),
    })
}

impl RequestPrepared {
    /// The merge window's document.
    pub fn merge_doc(&self) -> Option<&MergeRequestDoc> {
        self.merge.as_ref()
    }

    /// Re-runs the engine with another whitespace policy.
    pub fn reanalyze(&self, whitespace: WhitespacePolicy) -> Result<Analysis, String> {
        let opts = Options {
            whitespace,
            ..Options::default()
        };
        match &self.source {
            Some(Source::Bytes {
                base,
                ours,
                theirs,
                encoding,
            }) => run_analysis(base, ours, theirs, *encoding, &opts),
            Some(Source::Index { repo, token }) => repo
                .load_conflict(token, &opts)
                .map_err(|e| e.to_string())?
                .analysis
                .ok_or_else(|| "this file cannot be analysed as text".to_string()),
            None => Err("this request has no merge document".to_string()),
        }
    }

    /// Writes the editor's result and records the exit code. `Resolved`/`Force` exit 0
    /// (and stage the file when it is an unmerged index entry); `Markers` exits 1 and
    /// never stages. git mergetool stages MERGED itself after exit 0.
    pub fn save(&self, text: &str, encoding: &EncodingInfo, mode: SaveMode) -> Result<i32, String> {
        let bytes = mergeiq_core::encode(text, encoding);
        match &self.target {
            Target::File(path) => write_file_atomic(path, &bytes).map_err(|e| e.to_string())?,
            Target::Index { repo, path } => match mode {
                SaveMode::Resolved | SaveMode::Force => repo.save_resolved(path, &bytes),
                SaveMode::Markers => repo.save_unresolved(path, &bytes),
            }
            .map_err(|e| e.to_string())?,
        }
        let code = match mode {
            SaveMode::Resolved | SaveMode::Force => EXIT_RESOLVED,
            SaveMode::Markers => EXIT_UNRESOLVED,
        };
        *self.outcome.lock().expect("outcome lock") = Some(code);
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_of_a_folder_that_is_not_a_repo_exits_2() {
        let tmp = tempfile::tempdir().unwrap();
        let err = check_open(tmp.path()).unwrap_err();
        assert_eq!(err.exit_code, EXIT_INVALID);
        assert!(err.message.unwrap().contains("not a git repository"));
    }

    #[test]
    fn open_of_a_repo_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let out = std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["init", "-q"])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(check_open(tmp.path()).is_ok());
    }
}
