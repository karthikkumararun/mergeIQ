use thiserror::Error;

/// Which of the three merge inputs an error or chunk range refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Side {
    /// The common ancestor revision.
    Base,
    /// The current/working revision.
    Ours,
    /// The incoming revision being merged in.
    Theirs,
}

/// Errors from decoding, analyzing, parsing or serializing a merge.
#[derive(Debug, Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum MergeError {
    /// `side`'s input looks non-textual (see [`crate::decode`]).
    #[error("{side:?} input is binary")]
    Binary {
        /// Which input failed.
        side: Side,
    },
    /// `side`'s input declared an encoding (via BOM) but its bytes don't decode
    /// validly under it.
    #[error("failed to decode {side:?} input: {reason}")]
    Decode {
        /// Which input failed.
        side: Side,
        /// Human-readable decode failure detail.
        reason: String,
    },
    /// A conflict marker block was opened but never closed.
    #[error("malformed conflict markers at line {line}")]
    MalformedMarkers {
        /// 0-indexed line where the unterminated marker block started.
        line: usize,
    },
    /// [`crate::serialize`] was asked to write unresolved conflicts without markers.
    #[error("unresolved conflicts: {chunk_ids:?}")]
    Unresolved {
        /// IDs of the chunks that were still unresolved.
        chunk_ids: Vec<u32>,
    },
}
