//! Writing resolutions back to the working tree and index.

use std::io::Write as _;
use std::path::Path;

use crate::error::{GitError, Result};
use crate::paths::RepoPath;
use crate::repo::Repo;

/// Which side to accept wholesale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum AcceptSide {
    /// Stage 2.
    Ours,
    /// Stage 3.
    Theirs,
}

/// Whether the working-tree copy of a file uses CRLF, per git's attributes and config.
fn wants_crlf(text_attr: &str, eol_attr: &str, autocrlf: &str, core_eol: &str) -> bool {
    if text_attr == "unset" {
        return false;
    }
    match eol_attr {
        "crlf" => return true,
        "lf" => return false,
        _ => {}
    }
    if autocrlf == "true" {
        return true;
    }
    if text_attr == "set" || text_attr == "auto" {
        return core_eol == "crlf" || (core_eol.is_empty() && cfg!(windows));
    }
    false
}

/// Converts bare LF to CRLF, leaving existing CRLF alone. Binary data is returned as is.
fn to_crlf(bytes: &[u8]) -> Vec<u8> {
    if bytes.contains(&0) {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 20);
    let mut prev = 0u8;
    for &b in bytes {
        if b == b'\n' && prev != b'\r' {
            out.push(b'\r');
        }
        out.push(b);
        prev = b;
    }
    out
}

/// Writes `bytes` to `target` via a same-directory temp file, fsync, rename.
fn write_atomic(target: &Path, bytes: &[u8], exec_hint: bool) -> Result<()> {
    let mut tmp_name = target
        .file_name()
        .ok_or(GitError::InvalidPath)?
        .to_os_string();
    tmp_name.push(".mergeiq.tmp");
    let tmp = target.with_file_name(tmp_name);
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = match std::fs::metadata(target) {
                Ok(meta) if meta.is_file() => meta.permissions().mode() & 0o7777,
                _ if exec_hint => 0o755,
                _ => 0o644,
            };
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
        }
        #[cfg(not(unix))]
        let _ = exec_hint;
        std::fs::rename(&tmp, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(result?)
}

impl Repo {
    /// Bytes as they should appear in the working tree (applies CRLF per attributes/config).
    fn to_working_bytes(&self, path: &RepoPath, bytes: &[u8]) -> Result<Vec<u8>> {
        let raw = self.git([
            "check-attr".into(),
            "-z".into(),
            "text".into(),
            "eol".into(),
            "--".into(),
            path.to_os_string()?,
        ])?;
        let fields: Vec<&[u8]> = raw.split(|b| *b == 0).collect();
        let attr = |name: &str| {
            fields
                .chunks(3)
                .find(|c| c.len() == 3 && c[1] == name.as_bytes())
                .map(|c| String::from_utf8_lossy(c[2]).into_owned())
                .unwrap_or_default()
        };
        let config = |key: &str| {
            self.git_text_opt(["config", "--get", key])
                .unwrap_or_default()
        };
        let crlf = wants_crlf(
            &attr("text"),
            &attr("eol"),
            &config("core.autocrlf"),
            &config("core.eol"),
        );
        Ok(if crlf { to_crlf(bytes) } else { bytes.to_vec() })
    }

    /// Writes `bytes` (repository/LF form) to the working tree without staging.
    pub fn save_unresolved(&self, path: &RepoPath, bytes: &[u8]) -> Result<()> {
        self.note_mutation();
        let exec_hint = self
            .conflict_entry(path)?
            .is_some_and(|e| e.stages.iter().any(|s| s.mode == "100755"));
        let data = self.to_working_bytes(path, bytes)?;
        let result = write_atomic(&path.in_root(&self.root)?, &data, exec_hint);
        self.note_mutation();
        result
    }

    /// Writes `bytes` to the working tree and stages the path (`git add`).
    pub fn save_resolved(&self, path: &RepoPath, bytes: &[u8]) -> Result<()> {
        self.save_unresolved(path, bytes)?;
        let result = self.git(["add".into(), "--".into(), path.to_os_string()?]);
        self.note_mutation();
        result.map(|_| ())
    }

    /// Takes one side wholesale and stages it; if that side deleted the file, removes it.
    pub fn accept_side(&self, path: &RepoPath, side: AcceptSide) -> Result<()> {
        let entry = self
            .conflict_entry(path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        let (stage, flag) = match side {
            AcceptSide::Ours => (2, "--ours"),
            AcceptSide::Theirs => (3, "--theirs"),
        };
        self.note_mutation();
        let os = path.to_os_string()?;
        let result = if entry.stage(stage).is_none() {
            self.git(["rm".into(), "-f".into(), "-q".into(), "--".into(), os])
        } else {
            self.git(["checkout".into(), flag.into(), "--".into(), os.clone()])
                .and_then(|_| self.git(["add".into(), "--".into(), os]))
        };
        self.note_mutation();
        result.map(|_| ())
    }

    /// Recreates the conflicted state (stages 1-3 and markers) for a resolved path.
    pub fn restore_conflict(&self, path: &RepoPath) -> Result<()> {
        self.note_mutation();
        let result = self.git([
            "checkout".into(),
            "-m".into(),
            "--".into(),
            path.to_os_string()?,
        ]);
        self.note_mutation();
        result.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_conversion_is_idempotent_and_skips_binary() {
        assert_eq!(to_crlf(b"a\nb\r\nc"), b"a\r\nb\r\nc");
        assert_eq!(to_crlf(b"a\0\n"), b"a\0\n");
    }

    #[test]
    fn eol_decisions() {
        assert!(wants_crlf("", "", "true", ""));
        assert!(!wants_crlf("", "", "input", ""));
        assert!(!wants_crlf("unset", "", "true", ""));
        assert!(wants_crlf("set", "crlf", "", ""));
        assert!(!wants_crlf("set", "lf", "true", ""));
        assert!(wants_crlf("auto", "", "", "crlf"));
        assert!(!wants_crlf("", "", "", ""));
    }
}
