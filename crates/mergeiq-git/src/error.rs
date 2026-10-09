use std::io;

/// Errors from the git adapter. Serializable so IPC can pass them to the UI.
#[derive(Debug, Clone, thiserror::Error, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum GitError {
    /// No usable `git` executable was found.
    #[error("git executable not found")]
    GitNotFound,
    /// `git` is older than the minimum supported version.
    #[error("git {found} is too old; 2.30 or newer is required")]
    GitTooOld {
        /// The version string reported by git.
        found: String,
    },
    /// The path is inside a bare repository.
    #[error("bare repositories are not supported")]
    Bare,
    /// The path is not inside a git working tree.
    #[error("not a git repository: {path}")]
    NotARepo {
        /// Display form of the path that was opened.
        path: String,
    },
    /// The path has no unmerged index entries.
    #[error("no unresolved conflict at {path}")]
    NoSuchConflict {
        /// Display form of the path.
        path: String,
    },
    /// A path token was malformed or points outside the repository.
    #[error("invalid path")]
    InvalidPath,
    /// `continue` was requested while unmerged paths remain.
    #[error("{count} unresolved path(s) remain")]
    UnresolvedPaths {
        /// Number of unmerged paths.
        count: u32,
    },
    /// No merge/rebase/cherry-pick/revert/am is in progress.
    #[error("no operation in progress")]
    NoOperation,
    /// The requested action does not apply to the current operation.
    #[error("not supported: {what}")]
    Unsupported {
        /// What is unsupported.
        what: String,
    },
    /// A git command exited unsuccessfully.
    #[error("git {command} failed: {stderr}")]
    CommandFailed {
        /// The git subcommand.
        command: String,
        /// Captured stderr.
        stderr: String,
    },
    /// Filesystem or process error.
    #[error("io error: {message}")]
    Io {
        /// The underlying error text.
        message: String,
    },
}

impl From<io::Error> for GitError {
    fn from(err: io::Error) -> Self {
        GitError::Io {
            message: err.to_string(),
        }
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, GitError>;
