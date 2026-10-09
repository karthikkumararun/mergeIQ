//! Conflict classification beyond the index stage types.
//!
//! Each conflicted path gets a [`ConflictClass`] in a fixed priority order: submodule, symlink,
//! Git LFS pointer, binary, oversized, lockfile, text. Binary and LFS detection look at the
//! first bytes of each stage's blob and sizes come from the object header, so no blob is ever
//! read in full. Blob facts are cached by object id (content-addressed, so never stale).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::Stdio;
use std::sync::Arc;

use mergeiq_core::{decode, Side};

use crate::conflicts::StageEntry;
use crate::error::Result;
use crate::repo::Repo;

/// Files larger than this (any stage) are never loaded into the editor.
pub const OVERSIZED_BYTES: u64 = 20 * 1024 * 1024;

/// How many leading bytes of each blob are inspected.
pub const SNIFF_BYTES: usize = 8192;

/// Lockfiles with a known regeneration command (and `go.sum`, which merges by union).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum LockfileKind {
    /// `package-lock.json`.
    Npm,
    /// `pnpm-lock.yaml`.
    Pnpm,
    /// `yarn.lock`.
    Yarn,
    /// `poetry.lock`.
    Poetry,
    /// `Cargo.lock`.
    Cargo,
    /// `gradle.lockfile`.
    Gradle,
    /// `go.sum`.
    GoSum,
}

/// What kind of conflict a path is, independent of which stages exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "class", rename_all_fields = "camelCase")]
pub enum ConflictClass {
    /// Ordinary text: opens in the merge editor.
    Text,
    /// Binary content on some stage.
    Binary {
        /// The file name says it is an image the UI can preview.
        is_image: bool,
    },
    /// Some stage is a symbolic link.
    Symlink,
    /// Some stage is a submodule commit.
    Submodule,
    /// Some stage is a Git LFS pointer file.
    LfsPointer,
    /// Some stage is larger than [`OVERSIZED_BYTES`].
    Oversized,
    /// A known lockfile.
    Lockfile {
        /// Which lockfile.
        kind: LockfileKind,
    },
}

/// A parsed Git LFS pointer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LfsPointer {
    /// Object id as written in the pointer, e.g. `sha256:4d7a…`.
    pub oid: String,
    /// Size of the real content in bytes.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub size: u64,
}

/// Size and leading bytes of one blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobFacts {
    /// Full size in bytes (from the object header).
    pub size: u64,
    /// The first [`SNIFF_BYTES`] bytes (or the whole blob if smaller).
    pub prefix: Vec<u8>,
}

impl BlobFacts {
    /// The LFS pointer this blob is, if it is one (pointers are tiny text files).
    pub fn lfs_pointer(&self) -> Option<LfsPointer> {
        if self.size > 1024 {
            return None;
        }
        parse_lfs_pointer(&self.prefix)
    }

    /// Whether the engine would refuse to treat this blob as text.
    pub fn looks_binary(&self) -> bool {
        let mut bytes = self.prefix.as_slice();
        if self.size > bytes.len() as u64 {
            // The cut may split a multi-byte character; that is not binary content.
            if let Err(e) = std::str::from_utf8(bytes) {
                if e.error_len().is_none() {
                    bytes = &bytes[..e.valid_up_to()];
                }
            }
        }
        decode(bytes, Side::Ours).is_err()
    }
}

/// Parses a Git LFS pointer file (`version`, `oid sha256:<64 hex>`, `size <n>`).
pub fn parse_lfs_pointer(bytes: &[u8]) -> Option<LfsPointer> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    if lines.next()? != "version https://git-lfs.github.com/spec/v1" {
        return None;
    }
    let mut oid = None;
    let mut size = None;
    for line in lines {
        if let Some(rest) = line.strip_prefix("oid ") {
            let hex = rest.strip_prefix("sha256:")?;
            if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            oid = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("size ") {
            size = Some(rest.trim().parse::<u64>().ok()?);
        }
    }
    Some(LfsPointer {
        oid: oid?,
        size: size?,
    })
}

/// The lockfile kind for a file name, if it is one.
pub fn lockfile_kind(path: &str) -> Option<LockfileKind> {
    Some(match path.rsplit('/').next().unwrap_or(path) {
        "package-lock.json" => LockfileKind::Npm,
        "pnpm-lock.yaml" => LockfileKind::Pnpm,
        "yarn.lock" => LockfileKind::Yarn,
        "poetry.lock" => LockfileKind::Poetry,
        "Cargo.lock" => LockfileKind::Cargo,
        "gradle.lockfile" => LockfileKind::Gradle,
        "go.sum" => LockfileKind::GoSum,
        _ => return None,
    })
}

/// Image types the UI can preview.
pub fn is_image_name(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    ["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"]
        .iter()
        .any(|e| ext.eq_ignore_ascii_case(e))
}

/// Classifies one conflicted path from its stages. `facts` supplies blob facts by object id
/// (only consulted for regular files).
pub fn classify(
    path: &str,
    stages: &[StageEntry],
    facts: &dyn Fn(&str) -> Option<Arc<BlobFacts>>,
) -> ConflictClass {
    if stages.iter().any(|s| s.mode == "160000") {
        return ConflictClass::Submodule;
    }
    if stages.iter().any(|s| s.mode == "120000") {
        return ConflictClass::Symlink;
    }
    let blobs: Vec<Arc<BlobFacts>> = stages.iter().filter_map(|s| facts(&s.oid)).collect();
    if blobs.iter().any(|b| b.lfs_pointer().is_some()) {
        return ConflictClass::LfsPointer;
    }
    if blobs.iter().any(|b| b.looks_binary()) {
        return ConflictClass::Binary {
            is_image: is_image_name(path),
        };
    }
    if blobs.iter().any(|b| b.size > OVERSIZED_BYTES) {
        return ConflictClass::Oversized;
    }
    match lockfile_kind(path) {
        Some(kind) => ConflictClass::Lockfile { kind },
        None => ConflictClass::Text,
    }
}

impl Repo {
    /// Size and leading bytes for each object id, from one `git cat-file --batch` run.
    /// Unknown ids are left out. Results are cached by id.
    pub(crate) fn blob_facts(&self, oids: &[String]) -> Result<HashMap<String, Arc<BlobFacts>>> {
        let mut found: HashMap<String, Arc<BlobFacts>> = HashMap::new();
        let mut missing: Vec<String> = Vec::new();
        if let Ok(cache) = self.facts_cache.lock() {
            for oid in oids {
                match cache.get(oid) {
                    Some(f) => {
                        found.insert(oid.clone(), f.clone());
                    }
                    None if !missing.contains(oid) => missing.push(oid.clone()),
                    None => {}
                }
            }
        }
        if missing.is_empty() {
            return Ok(found);
        }
        let mut child = self
            .exec
            .command(&self.root, ["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let input = format!("{}\n", missing.join("\n"));
        // Feed ids from a thread so a large answer cannot deadlock against our reads.
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
        let mut out = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut parsed: Vec<(String, BlobFacts)> = Vec::new();
        for oid in &missing {
            let mut header = String::new();
            if out.read_line(&mut header)? == 0 {
                break;
            }
            let mut parts = header.split_whitespace();
            let (_, Some(kind), Some(size)) = (parts.next(), parts.next(), parts.next()) else {
                continue; // "<oid> missing"
            };
            let Ok(size) = size.parse::<u64>() else {
                continue;
            };
            let keep = size.min(SNIFF_BYTES as u64) as usize;
            let mut prefix = vec![0u8; keep];
            out.read_exact(&mut prefix)?;
            // Drop the rest of the object and the trailing newline.
            std::io::copy(
                &mut out.by_ref().take(size - keep as u64 + 1),
                &mut std::io::sink(),
            )?;
            if kind == "blob" {
                parsed.push((oid.clone(), BlobFacts { size, prefix }));
            }
        }
        let _ = writer.join();
        let _ = child.wait();
        if let Ok(mut cache) = self.facts_cache.lock() {
            for (oid, facts) in parsed {
                let facts = Arc::new(facts);
                cache.insert(oid.clone(), facts.clone());
                found.insert(oid, facts);
            }
        }
        Ok(found)
    }

    /// Fills in [`crate::ConflictEntry::class`] for `entries`.
    pub(crate) fn classify_entries(&self, entries: &mut [crate::ConflictEntry]) -> Result<()> {
        let oids: Vec<String> = entries
            .iter()
            .filter(|e| !e.has_gitlink && !e.has_symlink)
            .flat_map(|e| e.stages.iter().map(|s| s.oid.clone()))
            .collect();
        let facts = if oids.is_empty() {
            HashMap::new()
        } else {
            self.blob_facts(&oids)?
        };
        for entry in entries {
            entry.class = classify(&entry.display, &entry.stages, &|oid| {
                facts.get(oid).cloned()
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(n: u8, mode: &str, oid: &str) -> StageEntry {
        StageEntry {
            stage: n,
            mode: mode.into(),
            oid: oid.into(),
        }
    }

    fn facts(size: u64, prefix: &[u8]) -> Arc<BlobFacts> {
        Arc::new(BlobFacts {
            size,
            prefix: prefix.to_vec(),
        })
    }

    const POINTER: &str = "version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 12345\n";

    fn run(path: &str, stages: &[StageEntry], blobs: &[(&str, Arc<BlobFacts>)]) -> ConflictClass {
        let map: HashMap<&str, Arc<BlobFacts>> = blobs.iter().cloned().collect();
        classify(path, stages, &|oid| map.get(oid).cloned())
    }

    #[test]
    fn lfs_pointer_is_parsed() {
        let p = parse_lfs_pointer(POINTER.as_bytes()).unwrap();
        assert_eq!(p.size, 12345);
        assert!(p.oid.starts_with("sha256:4d7a"));
        assert_eq!(parse_lfs_pointer(b"hello\n"), None);
        assert_eq!(
            parse_lfs_pointer(
                b"version https://git-lfs.github.com/spec/v1\noid sha256:zz\nsize 1\n"
            ),
            None
        );
        assert_eq!(
            parse_lfs_pointer(b"version https://git-lfs.github.com/spec/v1\nsize 1\n"),
            None
        );
    }

    #[test]
    fn lockfile_names() {
        assert_eq!(
            lockfile_kind("web/pnpm-lock.yaml"),
            Some(LockfileKind::Pnpm)
        );
        assert_eq!(lockfile_kind("package-lock.json"), Some(LockfileKind::Npm));
        assert_eq!(lockfile_kind("a/yarn.lock"), Some(LockfileKind::Yarn));
        assert_eq!(lockfile_kind("poetry.lock"), Some(LockfileKind::Poetry));
        assert_eq!(lockfile_kind("x/Cargo.lock"), Some(LockfileKind::Cargo));
        assert_eq!(lockfile_kind("gradle.lockfile"), Some(LockfileKind::Gradle));
        assert_eq!(lockfile_kind("go.sum"), Some(LockfileKind::GoSum));
        assert_eq!(lockfile_kind("go.mod"), None);
        assert_eq!(lockfile_kind("my-yarn.lock.txt"), None);
    }

    #[test]
    fn image_names() {
        for n in [
            "a.png", "b/C.JPG", "d.jpeg", "e.gif", "f.webp", "g.svg", "h.ico",
        ] {
            assert!(is_image_name(n), "{n}");
        }
        assert!(!is_image_name("a.png.txt"));
        assert!(!is_image_name("png"));
    }

    #[test]
    fn priority_order() {
        let text = facts(10, b"hello\n");
        let nul = facts(10, b"a\0b");
        let ptr = facts(POINTER.len() as u64, POINTER.as_bytes());
        let huge = facts(OVERSIZED_BYTES + 1, b"line\n");
        let blobs = [("t", text), ("b", nul), ("l", ptr), ("h", huge)];
        let st = |mode: &str, oid: &str| vec![stage(2, mode, oid), stage(3, "100644", "t")];

        assert_eq!(
            run("a", &st("160000", "t"), &blobs),
            ConflictClass::Submodule
        );
        assert_eq!(run("a", &st("120000", "t"), &blobs), ConflictClass::Symlink);
        assert_eq!(
            run("a", &st("100644", "l"), &blobs),
            ConflictClass::LfsPointer
        );
        assert_eq!(
            run("logo.png", &st("100644", "b"), &blobs),
            ConflictClass::Binary { is_image: true }
        );
        assert_eq!(
            run("data.bin", &st("100644", "b"), &blobs),
            ConflictClass::Binary { is_image: false }
        );
        assert_eq!(
            run("a.csv", &st("100644", "h"), &blobs),
            ConflictClass::Oversized
        );
        assert_eq!(
            run("pnpm-lock.yaml", &st("100644", "t"), &blobs),
            ConflictClass::Lockfile {
                kind: LockfileKind::Pnpm
            }
        );
        assert_eq!(
            run("a.txt", &st("100644", "t"), &blobs),
            ConflictClass::Text
        );
        // Submodule beats symlink; binary beats oversized; oversized beats lockfile.
        let both = vec![stage(2, "160000", "t"), stage(3, "120000", "t")];
        assert_eq!(run("a", &both, &blobs), ConflictClass::Submodule);
        let big_bin = facts(OVERSIZED_BYTES + 1, b"\0\0");
        assert_eq!(
            run("x.dat", &[stage(2, "100644", "g")], &[("g", big_bin)]),
            ConflictClass::Binary { is_image: false }
        );
        let big = facts(OVERSIZED_BYTES + 1, b"text\n");
        assert_eq!(
            run("go.sum", &[stage(2, "100644", "g")], &[("g", big)]),
            ConflictClass::Oversized
        );
    }

    #[test]
    fn a_cut_multibyte_character_is_not_binary() {
        // "é" is 0xC3 0xA9; cut after the first byte of a larger blob.
        let mut prefix = b"caf".to_vec();
        prefix.push(0xC3);
        let f = BlobFacts {
            size: 10_000,
            prefix,
        };
        assert!(!f.looks_binary());
        // The same bytes as a complete blob are invalid UTF-8, i.e. binary.
        let whole = BlobFacts {
            size: 4,
            prefix: vec![b'c', b'a', b'f', 0xC3],
        };
        assert!(whole.looks_binary());
    }

    #[test]
    fn classification_serializes_with_a_class_tag() {
        let v = serde_json::to_value(ConflictClass::Lockfile {
            kind: LockfileKind::Pnpm,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"class": "Lockfile", "kind": "Pnpm"}));
        let v = serde_json::to_value(ConflictClass::Binary { is_image: true }).unwrap();
        assert_eq!(v, serde_json::json!({"class": "Binary", "isImage": true}));
        assert_eq!(
            serde_json::to_value(ConflictClass::Text).unwrap(),
            serde_json::json!({"class": "Text"})
        );
    }
}
