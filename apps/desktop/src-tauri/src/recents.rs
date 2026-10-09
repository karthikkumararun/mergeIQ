//! Recent repositories (most recent first, capped), stored in the settings file.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Maximum number of remembered repositories.
pub const MAX_RECENTS: usize = 15;

/// One remembered repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    /// Worktree root.
    pub path: String,
    /// Unix time (seconds) it was last opened.
    pub opened_at: u32,
}

/// Puts `path` first (removing an earlier entry for it) and trims to [`MAX_RECENTS`].
pub fn push(list: &mut Vec<RecentRepo>, path: &str, now: u32) {
    list.retain(|r| r.path != path);
    list.insert(
        0,
        RecentRepo {
            path: path.to_string(),
            opened_at: now,
        },
    );
    list.truncate(MAX_RECENTS);
}

/// Drops the entry for `path`.
pub fn remove(list: &mut Vec<RecentRepo>, path: &str) {
    list.retain(|r| r.path != path);
}

/// Folder name shown as the repository's title.
pub fn display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

/// Current Unix time in seconds.
pub fn now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[RecentRepo]) -> Vec<&str> {
        list.iter().map(|r| r.path.as_str()).collect()
    }

    #[test]
    fn recents_ordering() {
        let mut list = Vec::new();
        push(&mut list, "/code/a", 1);
        push(&mut list, "/code/b", 2);
        assert_eq!(paths(&list), ["/code/b", "/code/a"]);
        // Re-opening moves to the front without duplicating.
        push(&mut list, "/code/a", 3);
        assert_eq!(paths(&list), ["/code/a", "/code/b"]);
        assert_eq!(list[0].opened_at, 3);
    }

    #[test]
    fn capped_at_fifteen() {
        let mut list = Vec::new();
        for i in 0..20 {
            push(&mut list, &format!("/r/{i}"), i);
        }
        assert_eq!(list.len(), MAX_RECENTS);
        assert_eq!(list[0].path, "/r/19");
        assert_eq!(list[14].path, "/r/5");
    }

    #[test]
    fn remove_and_names() {
        let mut list = Vec::new();
        push(&mut list, "/code/shop-web", 1);
        remove(&mut list, "/code/shop-web");
        assert!(list.is_empty());
        assert_eq!(display_name("/code/shop-web"), "shop-web");
    }
}
