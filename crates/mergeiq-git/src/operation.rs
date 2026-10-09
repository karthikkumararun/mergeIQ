//! In-progress operation detection and contextual side labels.

use std::path::Path;

use crate::error::Result;
use crate::repo::Repo;

/// The operation that produced (or may produce) conflicts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum Operation {
    /// Nothing in progress.
    None,
    /// `git merge`.
    Merge,
    /// `git rebase` (merge or apply backend).
    Rebase {
        /// 1-based step currently stopped at.
        step: u32,
        /// Total steps.
        total: u32,
        /// Commit being rebased onto.
        onto: String,
    },
    /// `git cherry-pick`.
    CherryPick,
    /// `git revert`.
    Revert,
    /// `git am`.
    Am,
    /// Conflicts exist but no state file explains them (e.g. `git stash pop`).
    Unknown,
}

/// What one side of the conflict means to the user.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SideLabel {
    /// Human role, e.g. `"Your branch"`.
    pub role: String,
    /// Branch/ref name, if known.
    pub ref_name: Option<String>,
    /// Abbreviated commit id.
    pub short_sha: Option<String>,
    /// Commit subject.
    pub subject: Option<String>,
    /// Commit author name.
    pub author: Option<String>,
    /// Raw git term (`"ours"`/`"theirs"`), secondary info only.
    pub git_term: String,
}

/// Labels for stage 2 (`ours`) and stage 3 (`theirs`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SideLabels {
    /// Stage 2.
    pub ours: SideLabel,
    /// Stage 3.
    pub theirs: SideLabel,
}

struct CommitInfo {
    short_sha: String,
    author: String,
    subject: String,
}

fn read_trimmed(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

impl Repo {
    /// Detects the in-progress operation from the git dir's state files.
    pub fn operation(&self) -> Result<Operation> {
        let dir = &self.git_dir;
        let merge_dir = dir.join("rebase-merge");
        if merge_dir.is_dir() {
            let num = |name: &str| {
                read_trimmed(&merge_dir.join(name))
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(0)
            };
            return Ok(Operation::Rebase {
                step: num("msgnum"),
                total: num("end"),
                onto: read_trimmed(&merge_dir.join("onto")).unwrap_or_default(),
            });
        }
        let apply_dir = dir.join("rebase-apply");
        if apply_dir.is_dir() {
            if apply_dir.join("applying").exists() {
                return Ok(Operation::Am);
            }
            let num = |name: &str| {
                read_trimmed(&apply_dir.join(name))
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(0)
            };
            return Ok(Operation::Rebase {
                step: num("next"),
                total: num("last"),
                onto: read_trimmed(&apply_dir.join("onto")).unwrap_or_default(),
            });
        }
        if dir.join("CHERRY_PICK_HEAD").exists() {
            return Ok(Operation::CherryPick);
        }
        if dir.join("REVERT_HEAD").exists() {
            return Ok(Operation::Revert);
        }
        if dir.join("MERGE_HEAD").exists() {
            return Ok(Operation::Merge);
        }
        if self.unmerged_count()? > 0 {
            return Ok(Operation::Unknown);
        }
        Ok(Operation::None)
    }

    fn commit_info(&self, rev: &str) -> Option<CommitInfo> {
        let text = self.git_text_opt(["log", "-1", "--format=%h%x1f%an%x1f%s", rev, "--"])?;
        let mut parts = text.split('\u{1f}');
        Some(CommitInfo {
            short_sha: parts.next()?.to_string(),
            author: parts.next()?.to_string(),
            subject: parts.next().unwrap_or_default().to_string(),
        })
    }

    /// Friendly name for a commit: branch/tag name if one points at it, else `None`.
    fn name_of(&self, rev: &str) -> Option<String> {
        let name = self.git_text_opt(["name-rev", "--name-only", "--no-undefined", rev])?;
        if name.contains('~') || name.contains('^') {
            return None;
        }
        let name = name.strip_prefix("tags/").unwrap_or(&name);
        let name = name.strip_prefix("remotes/").unwrap_or(name);
        Some(name.to_string())
    }

    fn state_commit(&self, files: &[&str]) -> Option<String> {
        files
            .iter()
            .find_map(|f| read_trimmed(&self.git_dir.join(f)))
            .and_then(|t| t.lines().next().map(str::to_string))
    }

    /// Side labels for the given operation.
    pub fn side_labels(&self, operation: &Operation) -> Result<SideLabels> {
        let head_branch = self.git_text_opt(["symbolic-ref", "-q", "--short", "HEAD"]);
        let make = |role: String,
                    ref_name: Option<String>,
                    info: Option<CommitInfo>,
                    term: &str| SideLabel {
            role,
            ref_name,
            short_sha: info.as_ref().map(|i| i.short_sha.clone()),
            subject: info.as_ref().map(|i| i.subject.clone()),
            author: info.map(|i| i.author),
            git_term: term.to_string(),
        };
        let current = |role: &str| {
            make(
                role.into(),
                head_branch.clone(),
                self.commit_info("HEAD"),
                "ours",
            )
        };

        Ok(match operation {
            Operation::Merge => {
                let merge_head = self.state_commit(&["MERGE_HEAD"]);
                let name = read_trimmed(&self.git_dir.join("MERGE_MSG"))
                    .and_then(|m| merge_msg_ref(m.lines().next().unwrap_or_default()))
                    .or_else(|| merge_head.as_deref().and_then(|h| self.name_of(h)));
                let info = merge_head.as_deref().and_then(|h| self.commit_info(h));
                let shown = name
                    .clone()
                    .or_else(|| info.as_ref().map(|i| i.short_sha.clone()))
                    .unwrap_or_else(|| "unknown".into());
                SideLabels {
                    ours: current("Your branch"),
                    theirs: make(format!("Incoming: {shown}"), name, info, "theirs"),
                }
            }
            Operation::Rebase { onto, .. } => {
                let onto_name = (!onto.is_empty())
                    .then(|| self.name_of(onto))
                    .flatten()
                    .or_else(|| {
                        (!onto.is_empty())
                            .then(|| self.commit_info(onto).map(|i| i.short_sha))
                            .flatten()
                    })
                    .unwrap_or_else(|| "upstream".into());
                let replayed = self.state_commit(&["REBASE_HEAD", "rebase-merge/stopped-sha"]);
                let branch = self
                    .state_commit(&["rebase-merge/head-name", "rebase-apply/head-name"])
                    .map(|h| h.strip_prefix("refs/heads/").unwrap_or(&h).to_string())
                    .filter(|h| h != "detached HEAD");
                SideLabels {
                    ours: make(
                        format!("Upstream (rebasing onto {onto_name})"),
                        Some(onto_name),
                        self.commit_info("HEAD"),
                        "ours",
                    ),
                    theirs: make(
                        "Your commit being replayed".into(),
                        branch,
                        replayed.as_deref().and_then(|r| self.commit_info(r)),
                        "theirs",
                    ),
                }
            }
            Operation::CherryPick => {
                let head = self.state_commit(&["CHERRY_PICK_HEAD"]);
                SideLabels {
                    ours: current("Current branch"),
                    theirs: make(
                        "Cherry-picked commit".into(),
                        None,
                        head.as_deref().and_then(|h| self.commit_info(h)),
                        "theirs",
                    ),
                }
            }
            Operation::Revert => {
                let head = self.state_commit(&["REVERT_HEAD"]);
                let info = head.as_deref().and_then(|h| self.commit_info(h));
                let sha = info
                    .as_ref()
                    .map_or_else(|| "commit".to_string(), |i| i.short_sha.clone());
                SideLabels {
                    ours: current("Current branch"),
                    theirs: make(format!("Revert of {sha}"), None, info, "theirs"),
                }
            }
            Operation::Am => SideLabels {
                ours: current("Current branch"),
                theirs: make("Patch being applied".into(), None, None, "theirs"),
            },
            Operation::Unknown | Operation::None => SideLabels {
                ours: current("Current changes"),
                theirs: make("Incoming changes".into(), None, None, "theirs"),
            },
        })
    }
}

/// Extracts the merged ref from a `MERGE_MSG` subject such as
/// `Merge branch 'feature' into main` or `Merge remote-tracking branch 'origin/x'`.
fn merge_msg_ref(subject: &str) -> Option<String> {
    let rest = subject.strip_prefix("Merge ")?;
    let start = rest.find('\'')? + 1;
    let end = start + rest[start..].find('\'')?;
    Some(rest[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_merge_msg_subjects() {
        assert_eq!(
            merge_msg_ref("Merge branch 'feature'"),
            Some("feature".into())
        );
        assert_eq!(
            merge_msg_ref("Merge branch 'feature/x' into main"),
            Some("feature/x".into())
        );
        assert_eq!(
            merge_msg_ref("Merge remote-tracking branch 'origin/x'"),
            Some("origin/x".into())
        );
        assert_eq!(merge_msg_ref("Merge commit abc"), None);
    }
}
