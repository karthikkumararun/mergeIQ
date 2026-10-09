//! Submodule conflicts: the commit on each side and how the commits relate.

use std::path::Path;

use crate::error::{GitError, Result};
use crate::paths::PathToken;
use crate::repo::Repo;

/// A submodule commit, with details when the submodule is checked out and has the commit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleCommit {
    /// Full commit id.
    pub sha: String,
    /// Subject line, if known.
    pub subject: Option<String>,
    /// Author date (ISO 8601), if known.
    pub date: Option<String>,
}

/// How the two sides' commits relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SubmoduleRelation {
    /// The left commit is an ancestor of the right one (right is newer).
    LeftAncestorOfRight,
    /// The right commit is an ancestor of the left one (left is newer).
    RightAncestorOfLeft,
    /// Neither contains the other.
    Diverged,
    /// Cannot tell (submodule not checked out, or a commit is missing).
    Unknown,
}

/// Facts shown by the submodule panel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleDetails {
    /// The submodule has a working tree here.
    pub checked_out: bool,
    /// The merge base's commit.
    pub base: Option<SubmoduleCommit>,
    /// Our (left) commit.
    pub left: Option<SubmoduleCommit>,
    /// Their (right) commit.
    pub right: Option<SubmoduleCommit>,
    /// How left and right relate.
    pub relation: SubmoduleRelation,
}

impl Repo {
    fn sub_git(&self, dir: &Path, args: &[&str]) -> Option<std::process::Output> {
        self.exec.run(dir, args).ok()
    }

    /// Commits and ancestry for a conflicted gitlink.
    pub fn submodule_details(&self, token: &PathToken) -> Result<SubmoduleDetails> {
        let path = token.decode()?;
        let entry = self
            .conflict_entry(&path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let dir = path.in_root(&self.root)?;
        let checked_out = self
            .sub_git(&dir, &["rev-parse", "--git-dir"])
            .is_some_and(|o| o.status.success())
            && dir.join(".git").exists();
        let commit = |stage: u8| -> Option<SubmoduleCommit> {
            let sha = entry
                .stage(stage)
                .filter(|s| s.mode == "160000")?
                .oid
                .clone();
            let (mut subject, mut date) = (None, None);
            if checked_out {
                if let Some(out) =
                    self.sub_git(&dir, &["log", "-1", "--format=%s%x1f%aI", sha.as_str()])
                {
                    if out.status.success() {
                        let text = String::from_utf8_lossy(&out.stdout);
                        let mut parts = text.trim_end().split('\u{1f}');
                        subject = parts.next().map(str::to_string);
                        date = parts.next().map(str::to_string);
                    }
                }
            }
            Some(SubmoduleCommit { sha, subject, date })
        };
        let (base, left, right) = (commit(1), commit(2), commit(3));
        let is_ancestor = |a: &str, b: &str| -> Option<bool> {
            let out = self.sub_git(&dir, &["merge-base", "--is-ancestor", a, b])?;
            match out.status.code() {
                Some(0) => Some(true),
                Some(1) => Some(false),
                _ => None,
            }
        };
        let relation = match (&left, &right, checked_out) {
            (Some(l), Some(r), true) => {
                match (is_ancestor(&l.sha, &r.sha), is_ancestor(&r.sha, &l.sha)) {
                    (Some(true), _) => SubmoduleRelation::LeftAncestorOfRight,
                    (_, Some(true)) => SubmoduleRelation::RightAncestorOfLeft,
                    (Some(false), Some(false)) => SubmoduleRelation::Diverged,
                    _ => SubmoduleRelation::Unknown,
                }
            }
            _ => SubmoduleRelation::Unknown,
        };
        Ok(SubmoduleDetails {
            checked_out,
            base,
            left,
            right,
            relation,
        })
    }
}
