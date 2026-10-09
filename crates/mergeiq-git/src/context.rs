//! Commits on each side that touched a conflicted path.

use crate::error::Result;
use crate::operation::Operation;
use crate::paths::RepoPath;
use crate::repo::Repo;

/// Maximum commits listed per side.
pub const MAX_CONTEXT_COMMITS: u32 = 50;

/// One commit in a side's history.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    /// Full commit id.
    pub sha: String,
    /// Abbreviated commit id.
    pub short_sha: String,
    /// Subject line.
    pub subject: String,
    /// Author name.
    pub author: String,
    /// Author date, ISO 8601.
    pub date: String,
}

/// Commits per side for one path, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileContext {
    /// Commits on our side since the merge base.
    pub ours: Vec<CommitSummary>,
    /// Commits on their side (a single commit for rebase/cherry-pick/revert).
    pub theirs: Vec<CommitSummary>,
}

const LOG_FORMAT: &str = "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%s";

fn parse_log(raw: &[u8]) -> Vec<CommitSummary> {
    raw.split(|b| *b == 0)
        .filter(|r| !r.is_empty())
        .filter_map(|record| {
            let text = String::from_utf8_lossy(record);
            let mut parts = text.trim_start_matches('\n').split('\u{1f}');
            Some(CommitSummary {
                sha: parts.next()?.to_string(),
                short_sha: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                subject: parts.next()?.to_string(),
            })
        })
        .collect()
}

/// Longest commit body kept, in characters.
pub const MAX_BODY_CHARS: usize = 2000;

impl Repo {
    /// Message bodies (everything after the subject) of `shas`, keyed by full sha. Commits
    /// without a body are omitted. Unknown shas are skipped.
    pub fn commit_bodies(
        &self,
        shas: &[String],
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut out = std::collections::HashMap::new();
        if shas.is_empty() {
            return Ok(out);
        }
        let mut args: Vec<std::ffi::OsString> = vec![
            "show".into(),
            "-s".into(),
            "-z".into(),
            "--format=%H%x1f%b".into(),
        ];
        args.extend(
            shas.iter()
                .filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(Into::into),
        );
        args.push("--".into());
        let Ok(raw) = self.git(args) else {
            return Ok(out);
        };
        for record in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let text = String::from_utf8_lossy(record);
            let Some((sha, body)) = text.trim_start_matches('\n').split_once('\u{1f}') else {
                continue;
            };
            let body: String = body.trim().chars().take(MAX_BODY_CHARS).collect();
            if !body.is_empty() {
                out.insert(sha.to_string(), body);
            }
        }
        Ok(out)
    }

    fn log_path(&self, range: &str, path: &RepoPath) -> Result<Vec<CommitSummary>> {
        let raw = self.git([
            "log".into(),
            "-z".into(),
            "-n".into(),
            MAX_CONTEXT_COMMITS.to_string().into(),
            LOG_FORMAT.into(),
            range.into(),
            "--".into(),
            path.to_os_string()?,
        ])?;
        Ok(parse_log(&raw))
    }

    fn single_commit(&self, rev: &str) -> Result<Vec<CommitSummary>> {
        let raw = self.git(["log", "-z", "-1", LOG_FORMAT, rev, "--"])?;
        Ok(parse_log(&raw))
    }

    /// The incoming commit of the operation in progress, and whether it replays a single
    /// commit (rebase, cherry-pick, revert) rather than a branch.
    pub(crate) fn theirs_rev(&self, operation: &Operation) -> Option<(String, bool)> {
        let read = |files: &[&str]| -> Option<String> {
            files.iter().find_map(|f| {
                let text = std::fs::read_to_string(self.git_dir.join(f)).ok()?;
                text.lines().next().map(|l| l.trim().to_string())
            })
        };
        let (rev, single) = match operation {
            Operation::Merge => (read(&["MERGE_HEAD"]), false),
            Operation::Rebase { .. } => (read(&["REBASE_HEAD", "rebase-merge/stopped-sha"]), true),
            Operation::CherryPick => (read(&["CHERRY_PICK_HEAD"]), true),
            Operation::Revert => (read(&["REVERT_HEAD"]), true),
            Operation::Am | Operation::Unknown | Operation::None => (None, false),
        };
        rev.filter(|r| !r.is_empty()).map(|r| (r, single))
    }

    /// Commits touching `path` on each side since the merge base.
    pub fn file_context(&self, path: &RepoPath, operation: &Operation) -> Result<FileContext> {
        let Some((theirs_rev, single)) = self.theirs_rev(operation) else {
            return Ok(FileContext::default());
        };
        let base = self.git_text_opt(["merge-base", "HEAD", &theirs_rev]);
        let range = |tip: &str| match &base {
            Some(base) => format!("{base}..{tip}"),
            None => tip.to_string(),
        };
        let theirs = if single {
            self.single_commit(&theirs_rev)?
        } else {
            self.log_path(&range(&theirs_rev), path)?
        };
        Ok(FileContext {
            ours: self.log_path(&range("HEAD"), path)?,
            theirs,
        })
    }
}
