//! Reading stage blobs and working-tree bytes.

use crate::error::{GitError, Result};
use crate::paths::RepoPath;
use crate::repo::Repo;

/// Raw bytes of a conflicted path's index stages and working-tree file.
///
/// Stage bytes are in repository form (before smudge/EOL conversion). A stage is `None`
/// when absent from the index, or when it is a gitlink (submodule commit, no blob).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageBlobs {
    /// Stage 1.
    pub base: Option<Vec<u8>>,
    /// Stage 2.
    pub ours: Option<Vec<u8>>,
    /// Stage 3.
    pub theirs: Option<Vec<u8>>,
    /// Current working-tree bytes (`None` if the file does not exist).
    pub working: Option<Vec<u8>>,
}

impl Repo {
    /// Reads all three stages plus the working-tree file for `path`.
    pub fn read_blobs(&self, path: &RepoPath) -> Result<StageBlobs> {
        let entry = self
            .conflict_entry(path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let read = |stage: u8| -> Result<Option<Vec<u8>>> {
            match entry.stage(stage) {
                Some(s) if s.mode != "160000" => Ok(Some(self.git(["cat-file", "blob", &s.oid])?)),
                _ => Ok(None),
            }
        };
        Ok(StageBlobs {
            base: read(1)?,
            ours: read(2)?,
            theirs: read(3)?,
            working: self.read_working(path)?,
        })
    }

    /// The working-tree bytes of `path` (a symlink yields its target), or `None`.
    pub fn read_working(&self, path: &RepoPath) -> Result<Option<Vec<u8>>> {
        let full = path.in_root(&self.root)?;
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::read_link(&full)?;
                Ok(Some(target.to_string_lossy().into_owned().into_bytes()))
            }
            Ok(meta) if meta.is_dir() => Ok(None),
            Ok(_) => Ok(Some(std::fs::read(&full)?)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}
