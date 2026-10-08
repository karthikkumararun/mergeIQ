use crate::diff::diff_token_texts;
use crate::tokenize::{tokenize, Utf16Index};

/// A UTF-16 code-unit range `[start, end)`, matching JavaScript string indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Utf16Range {
    /// UTF-16 code unit offset of the range's start.
    pub start: u32,
    /// UTF-16 code unit offset just past the range's end.
    pub end: u32,
}

/// Token-level diff of one side's text vs base text (both chunk-relative).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SideFineDiff {
    /// Ranges within the base text that differ from this side.
    pub base_ranges: Vec<Utf16Range>,
    /// Ranges within this side's text that differ from base.
    pub side_ranges: Vec<Utf16Range>,
}

/// Token-level diff of a chunk: ours vs base, and theirs vs base (computed
/// independently, since the two sides may touch different tokens).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct FineDiff {
    /// Ours vs base.
    pub ours: SideFineDiff,
    /// Theirs vs base.
    pub theirs: SideFineDiff,
}

/// Tokens are runs of word characters (Unicode alphanumeric or `_`), runs of
/// whitespace, or single other characters (see [`crate::tokenize`]).
fn diff_side(base_text: &str, side_text: &str) -> SideFineDiff {
    let base_tokens = tokenize(base_text);
    let side_tokens = tokenize(side_text);
    let base_token_texts: Vec<&str> = base_tokens.iter().map(|t| t.text(base_text)).collect();
    let side_token_texts: Vec<&str> = side_tokens.iter().map(|t| t.text(side_text)).collect();
    let hunks = diff_token_texts(&base_token_texts, &side_token_texts);

    let base_idx = Utf16Index::new(base_text);
    let side_idx = Utf16Index::new(side_text);

    let mut base_ranges = Vec::new();
    let mut side_ranges = Vec::new();
    for h in hunks {
        if !h.before.is_empty() {
            let start_byte = base_tokens[h.before.start as usize].start;
            let end_byte = base_tokens[h.before.end as usize - 1].end;
            base_ranges.push(Utf16Range {
                start: base_idx.to_utf16(start_byte) as u32,
                end: base_idx.to_utf16(end_byte) as u32,
            });
        }
        if !h.after.is_empty() {
            let start_byte = side_tokens[h.after.start as usize].start;
            let end_byte = side_tokens[h.after.end as usize - 1].end;
            side_ranges.push(Utf16Range {
                start: side_idx.to_utf16(start_byte) as u32,
                end: side_idx.to_utf16(end_byte) as u32,
            });
        }
    }
    SideFineDiff {
        base_ranges,
        side_ranges,
    }
}

/// Computes the fine (token-level) diff for a chunk: ours vs base, and theirs vs
/// base. `base_text`/`ours_text`/`theirs_text` are the chunk's own text slices on
/// each side (not the whole file).
pub fn fine_diff(base_text: &str, ours_text: &str, theirs_text: &str) -> FineDiff {
    FineDiff {
        ours: diff_side(base_text, ours_text),
        theirs: diff_side(base_text, theirs_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_change_highlighted() {
        let base = "let total = sum(a, b);";
        let ours = "let total = sum(a, c);";
        let diff = fine_diff(base, ours, base);

        assert_eq!(diff.ours.side_ranges.len(), 1);
        let r = diff.ours.side_ranges[0];
        assert_eq!(&ours[r.start as usize..r.end as usize], "c");

        assert_eq!(diff.ours.base_ranges.len(), 1);
        let r = diff.ours.base_ranges[0];
        assert_eq!(&base[r.start as usize..r.end as usize], "b");

        // theirs == base, so no differences.
        assert!(diff.theirs.side_ranges.is_empty());
        assert!(diff.theirs.base_ranges.is_empty());
    }

    #[test]
    fn non_ascii_offsets_are_utf16() {
        let base = "naïve 😀 x";
        let ours = "naïve 😀 y";
        let diff = fine_diff(base, ours, base);

        assert_eq!(diff.ours.side_ranges.len(), 1);
        let r = diff.ours.side_ranges[0];
        // UTF-16 length check: emoji is a surrogate pair (2 units).
        let prefix_utf16_len = "naïve 😀 ".encode_utf16().count() as u32;
        assert_eq!(r.start, prefix_utf16_len);
        assert_eq!(r.end, prefix_utf16_len + 1);
    }
}
