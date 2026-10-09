//! Rename awareness for conflicted paths.
//!
//! Git reports a rename/rename conflict as two unrelated-looking entries (`b.ts` deleted by
//! them, `c.ts` deleted by us). Rename detection from the merge base to each side explains
//! them. It runs over the whole tree (a rename's source path is not a conflicted path), so it
//! is computed lazily, on demand, and cached per operation head.

use std::sync::Arc;

use crate::error::{GitError, Result};
use crate::operation::Operation;
use crate::paths::{PathToken, RepoPath};
use crate::repo::Repo;

/// Which side renamed a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum RenameSide {
    /// The current / working side (left).
    Ours,
    /// The incoming side (right).
    Theirs,
}

/// One rename detected from the merge base to a side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// Which side renamed.
    pub side: RenameSide,
    /// Old path.
    pub from: RepoPath,
    /// New path.
    pub to: RepoPath,
}

/// A rename involving a conflicted path, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RenameInfo {
    /// Which side renamed.
    pub side: RenameSide,
    /// Old path (display form).
    pub from: String,
    /// New path (display form).
    pub to: String,
    /// Token of the new path.
    pub to_path: PathToken,
}

/// Both sides renamed the same file to different paths.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RenamePair {
    /// The original path (display form).
    pub from: String,
    /// Where our side moved it.
    pub ours: RenameInfo,
    /// Where their side moved it.
    pub theirs: RenameInfo,
    /// The two destinations hold different content, so a text merge is needed after choosing.
    pub contents_differ: bool,
}

/// The result of choosing a path for a rename/rename conflict.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RenameOutcome {
    /// The chosen path.
    pub chosen: PathToken,
    /// The chosen path is now a text conflict to merge (otherwise it is resolved and staged).
    pub needs_merge: bool,
}

/// Parses `git diff --name-status -M -z` output into renames (`R<score> from to`).
pub(crate) fn parse_renames(raw: &[u8], side: RenameSide) -> Vec<Rename> {
    let mut out = Vec::new();
    let mut fields = raw.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let Some(status) = fields.next() {
        match status[0] {
            b'R' | b'C' => {
                let (Some(from), Some(to)) = (fields.next(), fields.next()) else {
                    break;
                };
                if status[0] == b'R' {
                    out.push(Rename {
                        side,
                        from: RepoPath::from_bytes(from.to_vec()),
                        to: RepoPath::from_bytes(to.to_vec()),
                    });
                }
            }
            _ => {
                let _ = fields.next();
            }
        }
    }
    out
}

fn info(r: &Rename) -> RenameInfo {
    RenameInfo {
        side: r.side,
        from: r.from.display(),
        to: r.to.display(),
        to_path: r.to.token(),
    }
}

impl Repo {
    /// The base, ours and theirs revisions the index stages come from.
    fn stage_revs(&self, operation: &Operation) -> Option<(String, String, String)> {
        let (incoming, single) = self.theirs_rev(operation)?;
        let head = self.git_text_opt(["rev-parse", "HEAD"])?;
        match operation {
            Operation::Merge => {
                let base = self.git_text_opt(["merge-base", "HEAD", &incoming])?;
                Some((base, head, incoming))
            }
            Operation::Revert => {
                // A revert merges with the reverted commit as base and its parent as theirs.
                let parent = self.git_text_opt(["rev-parse", &format!("{incoming}^")])?;
                Some((incoming, head, parent))
            }
            _ if single => {
                let base = self.git_text_opt(["rev-parse", &format!("{incoming}^")])?;
                Some((base, head, incoming))
            }
            _ => None,
        }
    }

    /// Every rename from the merge base to each side (cached per ours/theirs head).
    pub(crate) fn detect_renames(&self) -> Result<Arc<Vec<Rename>>> {
        let operation = self.operation()?;
        let Some((base, ours, theirs)) = self.stage_revs(&operation) else {
            return Ok(Arc::new(Vec::new()));
        };
        let key = (ours.clone(), theirs.clone());
        if let Ok(cache) = self.renames_cache.lock() {
            if let Some(hit) = cache.get(&key) {
                return Ok(hit.clone());
            }
        }
        let mut all = Vec::new();
        for (side, tip) in [(RenameSide::Ours, &ours), (RenameSide::Theirs, &theirs)] {
            let raw = self.git([
                "diff",
                "--name-status",
                "-M",
                "-z",
                base.as_str(),
                tip.as_str(),
            ])?;
            all.extend(parse_renames(&raw, side));
        }
        let all = Arc::new(all);
        if let Ok(mut cache) = self.renames_cache.lock() {
            cache.clear(); // only the current operation head is interesting
            cache.insert(key, all.clone());
        }
        Ok(all)
    }

    /// Renames that created or removed `path` on either side.
    pub fn renames_of(&self, path: &RepoPath) -> Result<Vec<RenameInfo>> {
        Ok(self
            .detect_renames()?
            .iter()
            .filter(|r| r.to == *path || r.from == *path)
            .map(info)
            .collect())
    }

    /// The pair of renames (ours, theirs) when both sides renamed the same original to
    /// different paths and `path` is the original or one of the two destinations.
    fn rename_pair_raw(&self, path: &RepoPath) -> Result<Option<(Rename, Rename)>> {
        let all = self.detect_renames()?;
        for ours in all.iter().filter(|r| r.side == RenameSide::Ours) {
            for theirs in all.iter().filter(|r| r.side == RenameSide::Theirs) {
                if ours.from == theirs.from
                    && ours.to != theirs.to
                    && (ours.to == *path || theirs.to == *path || ours.from == *path)
                {
                    return Ok(Some((ours.clone(), theirs.clone())));
                }
            }
        }
        Ok(None)
    }

    /// If both sides renamed the same original to different paths and `path` is the original
    /// or one of the two destinations, the pair (with whether the contents also differ).
    pub fn rename_pair_of(&self, path: &RepoPath) -> Result<Option<RenamePair>> {
        let Some((ours, theirs)) = self.rename_pair_raw(path)? else {
            return Ok(None);
        };
        let operation = self.operation()?;
        let contents_differ = match self.stage_revs(&operation) {
            Some((_, ours_rev, theirs_rev)) => {
                let a = self.tree_entry(&ours_rev, &ours.to)?;
                let b = self.tree_entry(&theirs_rev, &theirs.to)?;
                a != b
            }
            None => true,
        };
        Ok(Some(RenamePair {
            from: ours.from.display(),
            ours: info(&ours),
            theirs: info(&theirs),
            contents_differ,
        }))
    }

    /// `(mode, oid)` of `path` in the tree of `rev`.
    fn tree_entry(&self, rev: &str, path: &RepoPath) -> Result<Option<(String, String)>> {
        let raw = self.git([
            "ls-tree".into(),
            "-z".into(),
            rev.into(),
            "--".into(),
            path.to_os_string()?,
        ])?;
        let record = raw.split(|b| *b == 0).next().unwrap_or_default();
        let text = String::from_utf8_lossy(record);
        let meta = text.split('\t').next().unwrap_or_default();
        let mut parts = meta.split(' ');
        Ok(match (parts.next(), parts.next(), parts.next()) {
            (Some(mode), Some(_kind), Some(oid)) if !mode.is_empty() => {
                Some((mode.to_string(), oid.to_string()))
            }
            _ => None,
        })
    }

    /// Resolves a rename/rename conflict by choosing the final path.
    ///
    /// The three involved paths are removed from the index and working tree. If both sides
    /// have identical content at their destinations, `chosen` is staged with it. Otherwise
    /// `chosen` becomes an ordinary three-way text conflict (base from the original path,
    /// ours and theirs from the two destinations) for the merge editor.
    pub fn choose_rename_path(&self, chosen: &RepoPath) -> Result<RenameOutcome> {
        let unsupported = |what: &str| GitError::Unsupported { what: what.into() };
        let (ours, theirs) = self
            .rename_pair_raw(chosen)?
            .filter(|(o, t)| o.to == *chosen || t.to == *chosen)
            .ok_or_else(|| unsupported("this path is not a rename/rename destination"))?;
        let operation = self.operation()?;
        let (base_rev, ours_rev, theirs_rev) = self
            .stage_revs(&operation)
            .ok_or_else(|| unsupported("the operation's commits are not available"))?;
        let base_entry = self.tree_entry(&base_rev, &ours.from)?;
        let ours_entry = self
            .tree_entry(&ours_rev, &ours.to)?
            .ok_or_else(|| unsupported("the left side's file is missing"))?;
        let theirs_entry = self
            .tree_entry(&theirs_rev, &theirs.to)?
            .ok_or_else(|| unsupported("the right side's file is missing"))?;

        self.note_mutation();
        let result = (|| -> Result<bool> {
            for p in [&ours.from, &ours.to, &theirs.to] {
                self.git([
                    "rm".into(),
                    "-f".into(),
                    "-q".into(),
                    "--ignore-unmatch".into(),
                    "--".into(),
                    p.to_os_string()?,
                ])?;
            }
            if ours_entry == theirs_entry {
                let mut arg =
                    std::ffi::OsString::from(format!("{},{},", ours_entry.0, ours_entry.1));
                arg.push(chosen.to_os_string()?);
                self.git([
                    "update-index".into(),
                    "--add".into(),
                    "--cacheinfo".into(),
                    arg,
                ])?;
                self.git([
                    "checkout-index".into(),
                    "-f".into(),
                    "--".into(),
                    chosen.to_os_string()?,
                ])?;
                return Ok(false);
            }
            let mut input: Vec<u8> = Vec::new();
            for (stage, entry) in [
                (1, &base_entry),
                (2, &Some(ours_entry.clone())),
                (3, &Some(theirs_entry.clone())),
            ] {
                if let Some((mode, oid)) = entry {
                    input.extend_from_slice(format!("{mode} {oid} {stage}\t").as_bytes());
                    input.extend_from_slice(chosen.as_bytes());
                    input.push(0);
                }
            }
            self.git_with_stdin(["update-index", "--add", "-z", "--index-info"], &input)?;
            self.git([
                "checkout".into(),
                "-m".into(),
                "--".into(),
                chosen.to_os_string()?,
            ])?;
            Ok(true)
        })();
        self.note_mutation();
        Ok(RenameOutcome {
            chosen: chosen.token(),
            needs_merge: result?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_status() {
        let raw = b"M\0src/a.rs\0R087\0old/x.ts\0new/x.ts\0D\0gone\0C100\0orig\0copy\0A\0new\0";
        let r = parse_renames(raw, RenameSide::Theirs);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].from.display(), "old/x.ts");
        assert_eq!(r[0].to.display(), "new/x.ts");
        assert_eq!(r[0].side, RenameSide::Theirs);
    }

    #[test]
    fn tolerates_truncated_output() {
        assert!(parse_renames(b"R100\0only-one", RenameSide::Ours).is_empty());
        assert!(parse_renames(b"", RenameSide::Ours).is_empty());
    }
}
