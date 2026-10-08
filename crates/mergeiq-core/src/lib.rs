//! Pure merge engine: diff3, chunks, auto-resolve. No IO, no Tauri.
#![warn(missing_docs)]

mod analysis;
mod diff;
mod diff3;
mod encoding;
mod error;
mod fine;
mod markers;
mod serialize;
mod simple;
mod text;
mod tokenize;

pub use analysis::{analyze, apply_non_conflicting, Analysis, MergeInput, Options, SideText};
pub use diff::{diff_lines, line_keys, normalize_key, LineHunk, WhitespacePolicy};
pub use diff3::{Chunk, ChunkKind, LineRange};
pub use encoding::{decode, encode, Encoding, EncodingInfo, BINARY_SCAN_LIMIT};
pub use error::{MergeError, Side};
pub use fine::{fine_diff, FineDiff, SideFineDiff, Utf16Range};
pub use markers::{parse_markers, ConflictRegion, MarkerParse};
pub use serialize::{serialize, ResultLine, SerializeOptions, UnresolvedConflict};
pub use simple::{resolve_simple, SimpleResolution};
pub use text::{dominant_eol, split_lines, Decoded, Line, Terminator};
pub use tokenize::{tokenize, Token, TokenKind, Utf16Index};

/// This crate's version, from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
