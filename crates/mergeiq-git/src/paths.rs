//! Repository-relative paths as raw bytes, plus an opaque IPC token.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::error::{GitError, Result};

/// A repository-relative path, `/`-separated, as raw bytes (git paths need not be UTF-8).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(Vec<u8>);

impl RepoPath {
    /// Wraps raw path bytes as reported by git.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Lossy UTF-8 display name.
    pub fn display(&self) -> String {
        String::from_utf8_lossy(&self.0).into_owned()
    }

    /// The opaque token form used over IPC.
    pub fn token(&self) -> PathToken {
        PathToken(URL_SAFE_NO_PAD.encode(&self.0))
    }

    /// Argument form for passing to git.
    pub fn to_os_string(&self) -> Result<OsString> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Ok(OsString::from_vec(self.0.clone()))
        }
        #[cfg(not(unix))]
        {
            String::from_utf8(self.0.clone())
                .map(OsString::from)
                .map_err(|_| GitError::InvalidPath)
        }
    }

    /// Absolute filesystem path under `root`.
    pub fn in_root(&self, root: &Path) -> Result<PathBuf> {
        Ok(root.join(self.to_os_string()?))
    }
}

/// Opaque, URL-safe base64 of a [`RepoPath`]'s bytes. Pass back to the adapter unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(transparent)]
pub struct PathToken(pub String);

impl PathToken {
    /// Decodes and validates the token: it must be relative and stay inside the repo.
    pub fn decode(&self) -> Result<RepoPath> {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.0)
            .map_err(|_| GitError::InvalidPath)?;
        let bad = bytes.is_empty()
            || bytes.contains(&0)
            || bytes[0] == b'/'
            || bytes
                .split(|b| *b == b'/')
                .any(|c| c.is_empty() || c == b".." || c == b"." || c == b".git");
        if bad {
            return Err(GitError::InvalidPath);
        }
        Ok(RepoPath(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trips() {
        let path = RepoPath::from_bytes("src/a b/ü.txt".as_bytes().to_vec());
        let token = path.token();
        assert!(!token.0.contains('/') && !token.0.contains('+'));
        assert_eq!(token.decode().unwrap(), path);
        assert_eq!(path.display(), "src/a b/ü.txt");
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_has_lossy_display_and_usable_token() {
        let path = RepoPath::from_bytes(vec![b'd', b'/', 0xFF, 0xFE, b'.', b't']);
        assert!(path.display().contains('\u{FFFD}'));
        let back = path.token().decode().unwrap();
        assert_eq!(back.as_bytes(), path.as_bytes());
        assert!(back.to_os_string().is_ok());
    }

    #[test]
    fn rejects_escaping_tokens() {
        for bad in ["/etc/passwd", "../x", "a/../b", ".git/config", "a//b", ""] {
            let token = PathToken(URL_SAFE_NO_PAD.encode(bad));
            assert!(token.decode().is_err(), "{bad}");
        }
        assert!(PathToken("***".into()).decode().is_err());
    }
}
