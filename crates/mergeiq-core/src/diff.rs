use std::ops::Range;

use imara_diff::{Algorithm, Diff, InternedInput, TokenSource};

use crate::text::Line;

/// How line content is normalized for equality when diffing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WhitespacePolicy {
    /// Lines must match byte-for-byte.
    Exact,
    /// Trailing whitespace is ignored.
    TrimTrailing,
    /// Leading/trailing whitespace is ignored and interior whitespace runs collapse
    /// to a single space.
    IgnoreAmount,
    /// All whitespace is ignored.
    IgnoreAll,
}

/// Normalizes a line's content to a comparison key under `policy`. Lines with equal
/// keys are treated as equal when diffing.
pub fn normalize_key(text: &str, policy: WhitespacePolicy) -> String {
    match policy {
        WhitespacePolicy::Exact => text.to_string(),
        WhitespacePolicy::TrimTrailing => text.trim_end().to_string(),
        WhitespacePolicy::IgnoreAmount => {
            let mut out = String::with_capacity(text.len());
            let mut last_was_space = false;
            for c in text.trim().chars() {
                if c.is_whitespace() {
                    if !last_was_space {
                        out.push(' ');
                    }
                    last_was_space = true;
                } else {
                    out.push(c);
                    last_was_space = false;
                }
            }
            out
        }
        WhitespacePolicy::IgnoreAll => text.chars().filter(|c| !c.is_whitespace()).collect(),
    }
}

/// Builds normalized comparison keys for every line of `text`, for use with
/// [`diff_lines`].
pub fn line_keys(text: &str, lines: &[Line], policy: WhitespacePolicy) -> Vec<String> {
    lines
        .iter()
        .map(|l| normalize_key(l.text(text), policy))
        .collect()
}

/// A changed region: `before`/`after` are half-open line-index ranges into the two
/// key lists given to [`diff_lines`]. An empty range on either side means a pure
/// insertion or pure removal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LineHunk {
    /// Range into the "before" key list.
    pub before: Range<u32>,
    /// Range into the "after" key list.
    pub after: Range<u32>,
}

/// Wraps a slice of pre-normalized line keys as an `imara_diff` token source, one
/// token per line, compared by the normalized key's content.
struct KeySource<'a>(&'a [String]);

impl<'a> TokenSource for KeySource<'a> {
    type Token = &'a str;
    type Tokenizer = std::iter::Map<std::slice::Iter<'a, String>, fn(&'a String) -> &'a str>;

    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().map(String::as_str)
    }

    fn estimate_tokens(&self) -> u32 {
        self.0.len() as u32
    }
}

/// Wraps a slice of borrowed string tokens (e.g. tokenizer output) as an
/// `imara_diff` token source, compared by exact content.
struct SliceSource<'a>(&'a [&'a str]);

impl<'a> TokenSource for SliceSource<'a> {
    type Token = &'a str;
    type Tokenizer = std::iter::Copied<std::slice::Iter<'a, &'a str>>;

    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().copied()
    }

    fn estimate_tokens(&self) -> u32 {
        self.0.len() as u32
    }
}

/// Diffs two token-text sequences (exact match, no normalization) using the
/// histogram algorithm, returning hunks as token-index ranges. Used by `fine.rs` and
/// `simple.rs` for word-level diffing.
pub(crate) fn diff_token_texts<'a>(before: &'a [&'a str], after: &'a [&'a str]) -> Vec<LineHunk> {
    let input = InternedInput::new(SliceSource(before), SliceSource(after));
    let diff = Diff::compute(Algorithm::Histogram, &input);
    diff.hunks()
        .map(|h| LineHunk {
            before: h.before,
            after: h.after,
        })
        .collect()
}

/// Diffs two lists of per-line normalized comparison keys using the histogram
/// algorithm (same family as `git diff --histogram`), returning hunks as line-index
/// ranges. Because keys are pre-normalized per [`WhitespacePolicy`], whitespace policy
/// is purely an interning concern here.
pub fn diff_lines(before_keys: &[String], after_keys: &[String]) -> Vec<LineHunk> {
    let input = InternedInput::new(KeySource(before_keys), KeySource(after_keys));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    diff.hunks()
        .map(|h| LineHunk {
            before: h.before,
            after: h.after,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str, policy: WhitespacePolicy) -> Vec<String> {
        let lines = crate::text::split_lines(text);
        line_keys(text, &lines, policy)
    }

    #[test]
    fn no_changes_yields_no_hunks() {
        let before = keys("a\nb\nc\n", WhitespacePolicy::Exact);
        let after = before.clone();
        assert_eq!(diff_lines(&before, &after), vec![]);
    }

    #[test]
    fn single_line_modification() {
        let before = keys("a\nb\nc\n", WhitespacePolicy::Exact);
        let after = keys("a\nB\nc\n", WhitespacePolicy::Exact);
        let hunks = diff_lines(&before, &after);
        assert_eq!(
            hunks,
            vec![LineHunk {
                before: 1..2,
                after: 1..2
            }]
        );
    }

    #[test]
    fn pure_insertion() {
        let before = keys("a\nc\n", WhitespacePolicy::Exact);
        let after = keys("a\nb\nc\n", WhitespacePolicy::Exact);
        let hunks = diff_lines(&before, &after);
        assert_eq!(
            hunks,
            vec![LineHunk {
                before: 1..1,
                after: 1..2
            }]
        );
    }

    #[test]
    fn exact_policy_sees_trailing_whitespace_change() {
        let before = keys("a\n", WhitespacePolicy::Exact);
        let after = keys("a  \n", WhitespacePolicy::Exact);
        assert_eq!(diff_lines(&before, &after).len(), 1);
    }

    #[test]
    fn trim_trailing_policy_ignores_trailing_whitespace_change() {
        let before = keys("a\n", WhitespacePolicy::TrimTrailing);
        let after = keys("a  \n", WhitespacePolicy::TrimTrailing);
        assert_eq!(diff_lines(&before, &after), vec![]);
    }

    #[test]
    fn ignore_amount_collapses_interior_whitespace() {
        let before = keys("a  b\n", WhitespacePolicy::IgnoreAmount);
        let after = keys("a b\n", WhitespacePolicy::IgnoreAmount);
        assert_eq!(diff_lines(&before, &after), vec![]);
    }

    #[test]
    fn ignore_amount_still_sees_leading_change() {
        let before = keys("a b\n", WhitespacePolicy::IgnoreAmount);
        let after = keys("x b\n", WhitespacePolicy::IgnoreAmount);
        assert_eq!(diff_lines(&before, &after).len(), 1);
    }

    #[test]
    fn ignore_all_ignores_every_whitespace_difference() {
        let before = keys("a b c\n", WhitespacePolicy::IgnoreAll);
        let after = keys("ab  c\n", WhitespacePolicy::IgnoreAll);
        assert_eq!(diff_lines(&before, &after), vec![]);
    }
}
