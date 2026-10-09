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

impl Repo {
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

    /// Commits touching `path` on each side since the merge base.
    pub fn file_context(&self, path: &RepoPath, operation: &Operation) -> Result<FileContext> {
        let read = |files: &[&str]| -> Option<String> {
            files.iter().find_map(|f| {
                let text = std::fs::read_to_string(self.git_dir.join(f)).ok()?;
                text.lines().next().map(|l| l.trim().to_string())
            })
        };
        let (theirs_rev, single) = match operation {
            Operation::Merge => (read(&["MERGE_HEAD"]), false),
            Operation::Rebase { .. } => (read(&["REBASE_HEAD", "rebase-merge/stopped-sha"]), true),
            Operation::CherryPick => (read(&["CHERRY_PICK_HEAD"]), true),
            Operation::Revert => (read(&["REVERT_HEAD"]), true),
            Operation::Am | Operation::Unknown | Operation::None => (None, false),
        };
        let Some(theirs_rev) = theirs_rev.filter(|r| !r.is_empty()) else {
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
