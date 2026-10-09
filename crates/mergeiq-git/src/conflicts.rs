//! Listing unmerged index entries.

use std::collections::BTreeMap;

use crate::classify::ConflictClass;
use crate::error::Result;
use crate::paths::{PathToken, RepoPath};
use crate::repo::Repo;

/// Which index stages are present for a conflicted path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ConflictType {
    /// Stages 1, 2, 3.
    BothModified,
    /// Stages 2, 3.
    BothAdded,
    /// Stages 1, 3 (ours deleted it).
    DeletedByUs,
    /// Stages 1, 2 (theirs deleted it).
    DeletedByThem,
    /// Stage 2 only.
    AddedByUs,
    /// Stage 3 only.
    AddedByThem,
    /// Stage 1 only.
    BothDeleted,
}

impl ConflictType {
    fn from_stages(has: [bool; 3]) -> Option<Self> {
        Some(match has {
            [true, true, true] => Self::BothModified,
            [false, true, true] => Self::BothAdded,
            [true, false, true] => Self::DeletedByUs,
            [true, true, false] => Self::DeletedByThem,
            [false, true, false] => Self::AddedByUs,
            [false, false, true] => Self::AddedByThem,
            [true, false, false] => Self::BothDeleted,
            [false, false, false] => return None,
        })
    }
}

/// One index stage of a conflicted path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StageEntry {
    /// 1 = base, 2 = ours, 3 = theirs.
    pub stage: u8,
    /// Octal file mode, e.g. `"100644"`.
    pub mode: String,
    /// Blob (or gitlink commit) id.
    pub oid: String,
}

/// A conflicted path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntry {
    /// Opaque path token for other calls.
    pub path: PathToken,
    /// Lossy UTF-8 display name.
    pub display: String,
    /// Derived from the stages present.
    pub conflict_type: ConflictType,
    /// Present stages in order.
    pub stages: Vec<StageEntry>,
    /// Any stage is a symlink (mode 120000).
    pub has_symlink: bool,
    /// Any stage is a gitlink / submodule (mode 160000).
    pub has_gitlink: bool,
    /// What kind of conflict this is (binary, symlink, lockfile, ...), see `classify`.
    pub class: ConflictClass,
}

impl ConflictEntry {
    /// The stage entry for `stage` (1..=3), if present.
    pub fn stage(&self, stage: u8) -> Option<&StageEntry> {
        self.stages.iter().find(|s| s.stage == stage)
    }
}

/// Parses `git ls-files -u -z` output into entries sorted by path bytes.
pub(crate) fn parse_unmerged(raw: &[u8]) -> Vec<ConflictEntry> {
    let mut by_path: BTreeMap<Vec<u8>, Vec<StageEntry>> = BTreeMap::new();
    for record in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let Some(tab) = record.iter().position(|b| *b == b'\t') else {
            continue;
        };
        let meta = String::from_utf8_lossy(&record[..tab]);
        let mut parts = meta.split(' ');
        let (Some(mode), Some(oid), Some(stage)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(stage) = stage.parse::<u8>() else {
            continue;
        };
        by_path
            .entry(record[tab + 1..].to_vec())
            .or_default()
            .push(StageEntry {
                stage,
                mode: mode.to_string(),
                oid: oid.to_string(),
            });
    }
    by_path
        .into_iter()
        .filter_map(|(path, mut stages)| {
            stages.sort_by_key(|s| s.stage);
            let mut has = [false; 3];
            for s in &stages {
                if (1..=3).contains(&s.stage) {
                    has[usize::from(s.stage) - 1] = true;
                }
            }
            let conflict_type = ConflictType::from_stages(has)?;
            let path = RepoPath::from_bytes(path);
            Some(ConflictEntry {
                display: path.display(),
                path: path.token(),
                conflict_type,
                has_symlink: stages.iter().any(|s| s.mode == "120000"),
                has_gitlink: stages.iter().any(|s| s.mode == "160000"),
                class: ConflictClass::Text,
                stages,
            })
        })
        .collect()
}

impl Repo {
    /// All unmerged paths, sorted.
    pub fn list_conflicts(&self) -> Result<Vec<ConflictEntry>> {
        let mut entries = parse_unmerged(&self.git(["ls-files", "-u", "-z"])?);
        self.classify_entries(&mut entries)?;
        Ok(entries)
    }

    /// The conflict entry for one path, if it is still unmerged.
    pub(crate) fn conflict_entry(&self, path: &RepoPath) -> Result<Option<ConflictEntry>> {
        let raw = self.git([
            "ls-files".into(),
            "-u".into(),
            "-z".into(),
            "--".into(),
            path.to_os_string()?,
        ])?;
        let mut entries = parse_unmerged(&raw);
        self.classify_entries(&mut entries)?;
        Ok(entries.into_iter().find(|e| e.path == path.token()))
    }

    /// Number of unmerged paths.
    pub fn unmerged_count(&self) -> Result<u32> {
        Ok(u32::try_from(self.list_conflicts()?.len()).unwrap_or(u32::MAX))
    }
}
