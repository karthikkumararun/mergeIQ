//! Resolving a conflict by taking one side's exact index entry.
//!
//! Binary files, LFS pointers, symlinks and submodules must not pass through text filters or
//! the merge engine: the side's blob bytes are written verbatim and the exact object is staged
//! with `git update-index --cacheinfo`, so what ends up in the index is byte-for-byte that side.

use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use crate::conflicts::StageEntry;
use crate::error::{GitError, Result};
use crate::paths::RepoPath;
use crate::repo::Repo;
use crate::write::AcceptSide;

/// Streams the blob `oid` into `target` via a same-directory temp file, then renames.
fn write_blob_exact(repo: &Repo, oid: &str, target: &Path, executable: bool) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // A leftover symlink or file must not be followed or merged with.
    match std::fs::symlink_metadata(target) {
        Ok(meta) if meta.is_dir() => {
            return Err(GitError::Unsupported {
                what: "a directory is in the way of this file".into(),
            })
        }
        Ok(_) => std::fs::remove_file(target)?,
        Err(_) => {}
    }
    let mut tmp_name = target
        .file_name()
        .ok_or(GitError::InvalidPath)?
        .to_os_string();
    tmp_name.push(".mergeiq.tmp");
    let tmp = target.with_file_name(tmp_name);
    let result = (|| -> Result<()> {
        let mut child = repo
            .exec
            .command(&repo.root, ["cat-file", "blob", oid])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut file = std::fs::File::create(&tmp)?;
        std::io::copy(&mut stdout, &mut file)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(GitError::CommandFailed {
                command: "cat-file".into(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if executable { 0o755 } else { 0o644 };
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
        }
        #[cfg(not(unix))]
        let _ = executable;
        std::fs::rename(&tmp, target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Creates a symlink `link` -> `target`. Returns `false` if the platform refused (Windows
/// without the privilege), in which case the caller writes the target as file content.
fn create_symlink(target: &str, link: &Path) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
        Ok(true)
    }
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_file(target, link) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Ok(false),
            Err(e) if e.raw_os_error() == Some(1314) => Ok(false),
            Err(e) => Err(e),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Ok(false)
    }
}

impl Repo {
    fn stage_for(&self, path: &RepoPath, side: AcceptSide) -> Result<Option<StageEntry>> {
        let entry = self
            .conflict_entry(path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let stage = match side {
            AcceptSide::Ours => 2,
            AcceptSide::Theirs => 3,
        };
        Ok(entry.stage(stage).cloned())
    }

    /// Whether symlinks should be created as real links (`core.symlinks`, default true).
    fn symlinks_enabled(&self) -> bool {
        self.git_text_opt(["config", "--type=bool", "--get", "core.symlinks"])
            .is_none_or(|v| v != "false")
    }

    fn stage_exact(&self, path: &RepoPath, mode: &str, oid: &str) -> Result<()> {
        let mut arg = std::ffi::OsString::from(format!("{mode},{oid},"));
        arg.push(path.to_os_string()?);
        self.git([
            "update-index".into(),
            "--add".into(),
            "--cacheinfo".into(),
            arg,
        ])
        .map(|_| ())
    }

    /// Resolves by taking `side` byte for byte (regular file, symlink or gitlink) and staging
    /// exactly that object. If the side deleted the path, it is removed.
    pub fn use_side_exact(&self, path: &RepoPath, side: AcceptSide) -> Result<()> {
        let stage = self.stage_for(path, side)?;
        self.note_mutation();
        let result = (|| -> Result<()> {
            let Some(stage) = stage else {
                return self
                    .git([
                        "rm".into(),
                        "-f".into(),
                        "-q".into(),
                        "--".into(),
                        path.to_os_string()?,
                    ])
                    .map(|_| ());
            };
            let target = path.in_root(&self.root)?;
            match stage.mode.as_str() {
                "160000" => {} // the submodule's working tree is left alone
                "120000" => {
                    let link =
                        String::from_utf8_lossy(&self.git(["cat-file", "blob", &stage.oid])?)
                            .into_owned();
                    if let Ok(meta) = std::fs::symlink_metadata(&target) {
                        if !meta.is_dir() {
                            std::fs::remove_file(&target)?;
                        }
                    }
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let made = self.symlinks_enabled() && create_symlink(&link, &target)?;
                    if !made {
                        // `core.symlinks=false` semantics: the target text becomes the content.
                        std::fs::write(&target, link.as_bytes())?;
                    }
                }
                mode => write_blob_exact(self, &stage.oid, &target, mode == "100755")?,
            }
            self.stage_exact(path, &stage.mode, &stage.oid)
        })();
        self.note_mutation();
        result
    }

    /// Writes the surviving side's content to the working tree without staging, so it can be
    /// edited before the conflict is marked resolved ("Keep and edit").
    pub fn write_side_unstaged(&self, path: &RepoPath, side: AcceptSide) -> Result<()> {
        let stage = self.stage_for(path, side)?;
        let stage = stage.ok_or_else(|| GitError::Unsupported {
            what: "that side deleted the file".into(),
        })?;
        if stage.mode == "160000" || stage.mode == "120000" {
            return Err(GitError::Unsupported {
                what: "only regular files can be edited".into(),
            });
        }
        self.note_mutation();
        let result = write_blob_exact(
            self,
            &stage.oid,
            &path.in_root(&self.root)?,
            stage.mode == "100755",
        );
        self.note_mutation();
        result
    }
}
