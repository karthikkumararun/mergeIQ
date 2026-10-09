//! Aggregate views for the UI: repo status and a fully loaded conflict.

use mergeiq_core::{analyze, Analysis, MergeInput, Options};

use crate::conflicts::ConflictEntry;
use crate::context::FileContext;
use crate::error::{GitError, Result};
use crate::operation::{Operation, SideLabels};
use crate::paths::PathToken;
use crate::repo::Repo;

/// Snapshot of a repository's conflict state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    /// Worktree root (lossy display form).
    pub root: String,
    /// Checked-out branch, or `None` when HEAD is detached (e.g. mid-rebase).
    pub branch: Option<String>,
    /// Operation in progress.
    pub operation: Operation,
    /// Side labels for that operation.
    pub labels: SideLabels,
    /// Unmerged paths.
    pub conflicts: Vec<ConflictEntry>,
}

/// Everything the merge editor needs for one conflicted file.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictLoad {
    /// The index entry.
    pub entry: ConflictEntry,
    /// Side labels.
    pub labels: SideLabels,
    /// Commits per side touching the file.
    pub context: FileContext,
    /// Merge analysis of the stages (an absent stage counts as empty). `None` when the
    /// content cannot be analysed as text; see `analysis_error`.
    pub analysis: Option<Analysis>,
    /// Why `analysis` is `None` (binary, undecodable, symlink, submodule).
    pub analysis_error: Option<String>,
}

impl Repo {
    /// Operation, labels and conflict list in one call.
    pub fn status(&self) -> Result<RepoStatus> {
        let operation = self.operation()?;
        Ok(RepoStatus {
            root: self.root.display().to_string(),
            branch: self.git_text_opt(["symbolic-ref", "-q", "--short", "HEAD"]),
            labels: self.side_labels(&operation)?,
            operation,
            conflicts: self.list_conflicts()?,
        })
    }

    /// Loads stage content through `mergeiq-core`, plus labels and commit context.
    pub fn load_conflict(&self, token: &PathToken, opts: &Options) -> Result<ConflictLoad> {
        let path = token.decode()?;
        let entry = self
            .conflict_entry(&path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let operation = self.operation()?;
        let blobs = self.read_blobs(&path)?;
        let (analysis, analysis_error) = if entry.has_symlink || entry.has_gitlink {
            (
                None,
                Some("symlink or submodule conflicts cannot be merged as text".to_string()),
            )
        } else {
            let input = MergeInput {
                base: blobs.base.as_deref().unwrap_or_default(),
                ours: blobs.ours.as_deref().unwrap_or_default(),
                theirs: blobs.theirs.as_deref().unwrap_or_default(),
            };
            match analyze(input, opts) {
                Ok(analysis) => (Some(analysis), None),
                Err(err) => (None, Some(err.to_string())),
            }
        };
        Ok(ConflictLoad {
            labels: self.side_labels(&operation)?,
            context: self.file_context(&path, &operation)?,
            entry,
            analysis,
            analysis_error,
        })
    }
}
