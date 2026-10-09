//! Extra facts about one conflicted path for the special-conflict panels: per-stage sizes,
//! symlink targets and LFS pointers, rename information, and blob payloads for previews.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use mergeiq_core::{decode, diff_lines, line_keys, split_lines, LineHunk, Side, WhitespacePolicy};

use crate::classify::LfsPointer;
use crate::conflicts::{ConflictEntry, ConflictType};
use crate::context::FileContext;
use crate::error::{GitError, Result};
use crate::operation::SideLabels;
use crate::paths::{PathToken, RepoPath};
use crate::renames::{RenameInfo, RenamePair, RenameSide};
use crate::repo::Repo;

/// Largest blob sent to the UI for previews.
pub const MAX_PREVIEW_BYTES: u64 = 20 * 1024 * 1024;

/// Largest text shown in a modify/delete diff.
pub const MAX_DIFF_BYTES: u64 = 2 * 1024 * 1024;

/// Facts about one index stage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StageMeta {
    /// 1 = base, 2 = ours, 3 = theirs.
    pub stage: u8,
    /// Octal file mode.
    pub mode: String,
    /// Blob (or gitlink commit) id.
    pub oid: String,
    /// Blob size in bytes (`None` for a gitlink).
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub size: Option<u64>,
    /// Link target when the stage is a symlink.
    pub symlink_target: Option<String>,
    /// The pointer when the stage is a Git LFS pointer file.
    pub lfs: Option<LfsPointer>,
}

/// Everything the special-conflict panels need beyond the index entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetails {
    /// The index entry, including its class.
    pub entry: ConflictEntry,
    /// Side labels.
    pub labels: SideLabels,
    /// Commits per side touching the file.
    pub context: FileContext,
    /// Per-stage facts, in stage order.
    pub stages: Vec<StageMeta>,
    /// Renames that created or removed this path.
    pub renames: Vec<RenameInfo>,
    /// Set when both sides renamed the same file to different paths.
    pub rename_pair: Option<RenamePair>,
}

/// A blob prepared for the UI (an image preview).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StageBlob {
    /// MIME type derived from the file extension.
    pub mime: String,
    /// Blob size in bytes.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub size: u64,
    /// Standard base64 of the bytes.
    pub base64: String,
}

/// The surviving side of a modify/delete conflict compared with the base.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ModifyDeleteView {
    /// Which side deleted the file.
    pub deleted_by: RenameSide,
    /// The base text.
    pub base_text: Option<String>,
    /// The surviving side's text.
    pub survivor_text: Option<String>,
    /// Changed line ranges (`before` in the base, `after` in the survivor).
    pub hunks: Vec<LineHunk>,
    /// Why no text diff is available (binary, too large, ...).
    pub note: Option<String>,
}

/// MIME type for an image file name (`application/octet-stream` otherwise).
pub fn mime_for(path: &str) -> &'static str {
    let ext = path
        .rsplit('/')
        .next()
        .and_then(|n| n.rsplit_once('.'))
        .map_or("", |(_, e)| e)
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

impl Repo {
    /// Details for a conflicted path (see [`ConflictDetails`]).
    pub fn conflict_details(&self, token: &PathToken) -> Result<ConflictDetails> {
        let path = token.decode()?;
        let entry = self
            .conflict_entry(&path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let operation = self.operation()?;
        let oids: Vec<String> = entry
            .stages
            .iter()
            .filter(|s| s.mode != "160000")
            .map(|s| s.oid.clone())
            .collect();
        let facts = self.blob_facts(&oids)?;
        let stages = entry
            .stages
            .iter()
            .map(|s| {
                let f = facts.get(&s.oid);
                StageMeta {
                    stage: s.stage,
                    mode: s.mode.clone(),
                    oid: s.oid.clone(),
                    size: f.map(|f| f.size),
                    symlink_target: (s.mode == "120000")
                        .then(|| f.map(|f| String::from_utf8_lossy(&f.prefix).into_owned()))
                        .flatten(),
                    lfs: f.and_then(|f| f.lfs_pointer()),
                }
            })
            .collect();
        Ok(ConflictDetails {
            labels: self.side_labels(&operation)?,
            context: self.file_context(&path, &operation)?,
            renames: self.renames_of(&path)?,
            rename_pair: self.rename_pair_of(&path)?,
            stages,
            entry,
        })
    }

    /// The bytes of one stage for an image preview, capped at [`MAX_PREVIEW_BYTES`].
    pub fn stage_blob(&self, token: &PathToken, stage: u8) -> Result<StageBlob> {
        let path = token.decode()?;
        let entry = self
            .conflict_entry(&path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let s = entry
            .stage(stage)
            .filter(|s| s.mode != "160000")
            .ok_or_else(|| GitError::Unsupported {
                what: "that side has no file content".into(),
            })?;
        let size: u64 = self
            .git(["cat-file", "-s", s.oid.as_str()])
            .ok()
            .and_then(|b| String::from_utf8_lossy(&b).trim().parse().ok())
            .unwrap_or(0);
        if size > MAX_PREVIEW_BYTES {
            return Err(GitError::Unsupported {
                what: format!(
                    "the file is larger than {} MB",
                    MAX_PREVIEW_BYTES / 1024 / 1024
                ),
            });
        }
        let bytes = self.git(["cat-file", "blob", s.oid.as_str()])?;
        Ok(StageBlob {
            mime: mime_for(&entry.display).to_string(),
            size,
            base64: STANDARD.encode(&bytes),
        })
    }

    /// The text diff of the surviving side against the base for a modify/delete conflict.
    pub fn modify_delete_view(&self, token: &PathToken) -> Result<ModifyDeleteView> {
        let path = token.decode()?;
        let entry = self
            .conflict_entry(&path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let (deleted_by, survivor) = match entry.conflict_type {
            ConflictType::DeletedByUs => (RenameSide::Ours, 3),
            ConflictType::DeletedByThem => (RenameSide::Theirs, 2),
            _ => {
                return Err(GitError::Unsupported {
                    what: "this is not a modify/delete conflict".into(),
                })
            }
        };
        let read = |stage: u8| -> Result<std::result::Result<String, String>> {
            let Some(s) = entry.stage(stage) else {
                return Ok(Err("missing".into()));
            };
            let size: u64 = self
                .git(["cat-file", "-s", s.oid.as_str()])
                .ok()
                .and_then(|b| String::from_utf8_lossy(&b).trim().parse().ok())
                .unwrap_or(0);
            if size > MAX_DIFF_BYTES {
                return Ok(Err("the file is too large to diff here".into()));
            }
            let bytes = self.git(["cat-file", "blob", s.oid.as_str()])?;
            Ok(decode(&bytes, Side::Ours)
                .map(|(t, _)| t)
                .map_err(|_| "the file is binary".to_string()))
        };
        let base = read(1)?;
        let surviving = read(survivor)?;
        let note = [&base, &surviving]
            .iter()
            .find_map(|r| r.as_ref().err().filter(|e| *e != "missing").cloned());
        let (base_text, survivor_text) = (base.ok(), surviving.ok());
        let hunks = match (&base_text, &survivor_text) {
            (Some(b), Some(s)) => {
                let (bl, sl) = (split_lines(b), split_lines(s));
                diff_lines(
                    &line_keys(b, &bl, WhitespacePolicy::Exact),
                    &line_keys(s, &sl, WhitespacePolicy::Exact),
                )
            }
            _ => Vec::new(),
        };
        Ok(ModifyDeleteView {
            deleted_by,
            base_text,
            survivor_text,
            hunks,
            note,
        })
    }
}

/// A path token for display-form paths returned in details (helper for callers).
pub fn token_of(display: &str) -> PathToken {
    RepoPath::from_bytes(display.as_bytes().to_vec()).token()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_types() {
        assert_eq!(mime_for("a/logo.PNG"), "image/png");
        assert_eq!(mime_for("x.jpeg"), "image/jpeg");
        assert_eq!(mime_for("x.svg"), "image/svg+xml");
        assert_eq!(mime_for("x.ico"), "image/x-icon");
        assert_eq!(mime_for("x.bin"), "application/octet-stream");
        assert_eq!(mime_for("noext"), "application/octet-stream");
    }
}
