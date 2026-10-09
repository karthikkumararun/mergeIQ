/// A token class: a run of word characters, a run of whitespace, or a single other
/// character (punctuation, symbols, etc. are never grouped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum TokenKind {
    /// A maximal run of Unicode alphanumeric characters or `_`.
    Word,
    /// A maximal run of whitespace.
    Whitespace,
    /// A single character that is neither a word character nor whitespace.
    Other,
}

/// A token's byte range `[start, end)` within the source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Token {
    /// Byte offset of the token's first byte.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub start: usize,
    /// Byte offset just past the token's last byte.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub end: usize,
    /// This token's class.
    pub kind: TokenKind,
}

impl Token {
    /// The token's text within `s` (the string it was tokenized from).
    pub fn text<'a>(&self, s: &'a str) -> &'a str {
        &s[self.start..self.end]
    }
}

fn classify(c: char) -> TokenKind {
    if c.is_whitespace() {
        TokenKind::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        TokenKind::Word
    } else {
        TokenKind::Other
    }
}

/// Splits `s` into tokens: maximal runs of word characters (Unicode alphanumeric or
/// `_`), maximal runs of whitespace, or single other characters (never grouped).
pub fn tokenize(s: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let kind = classify(c);
        let mut end = start + c.len_utf8();
        if kind != TokenKind::Other {
            while let Some(&(idx, c2)) = chars.peek() {
                if classify(c2) == kind {
                    end = idx + c2.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
        }
        tokens.push(Token { start, end, kind });
    }
    tokens
}

/// Maps byte offsets of `s` (which must fall on char boundaries) to UTF-16 code unit
/// offsets, matching JavaScript string indexing.
pub struct Utf16Index {
    /// `(byte_offset, utf16_offset)` pairs at every char boundary, sorted by byte offset.
    boundaries: Vec<(usize, usize)>,
}

impl Utf16Index {
    /// Builds the byte-offset-to-UTF-16-offset index for `s`.
    pub fn new(s: &str) -> Self {
        let mut boundaries = Vec::with_capacity(s.len() + 1);
        boundaries.push((0, 0));
        let mut byte = 0usize;
        let mut utf16 = 0usize;
        for c in s.chars() {
            byte += c.len_utf8();
            utf16 += c.len_utf16();
            boundaries.push((byte, utf16));
        }
        Self { boundaries }
    }

    /// Converts a byte offset to its UTF-16 code unit offset.
    ///
    /// # Panics
    /// Panics if `byte_offset` does not fall on a char boundary of the indexed string.
    pub fn to_utf16(&self, byte_offset: usize) -> usize {
        match self
            .boundaries
            .binary_search_by_key(&byte_offset, |&(b, _)| b)
        {
            Ok(i) => self.boundaries[i].1,
            Err(_) => panic!("byte offset {byte_offset} is not a char boundary"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_whitespace_are_grouped() {
        let tokens = tokenize("let total = sum(a, b);");
        let rendered: Vec<(&str, TokenKind)> = tokens
            .iter()
            .map(|t| (t.text("let total = sum(a, b);"), t.kind))
            .collect();
        assert_eq!(
            rendered,
            vec![
                ("let", TokenKind::Word),
                (" ", TokenKind::Whitespace),
                ("total", TokenKind::Word),
                (" ", TokenKind::Whitespace),
                ("=", TokenKind::Other),
                (" ", TokenKind::Whitespace),
                ("sum", TokenKind::Word),
                ("(", TokenKind::Other),
                ("a", TokenKind::Word),
                (",", TokenKind::Other),
                (" ", TokenKind::Whitespace),
                ("b", TokenKind::Word),
                (")", TokenKind::Other),
                (";", TokenKind::Other),
            ]
        );
    }

    #[test]
    fn cjk_run_is_one_word_token() {
        let s = "你好";
        let tokens = tokenize(s);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Word);
        assert_eq!(tokens[0].text(s), "你好");
    }

    #[test]
    fn emoji_is_a_single_other_token() {
        let s = "a😀b";
        let tokens = tokenize(s);
        let rendered: Vec<(&str, TokenKind)> = tokens.iter().map(|t| (t.text(s), t.kind)).collect();
        assert_eq!(
            rendered,
            vec![
                ("a", TokenKind::Word),
                ("😀", TokenKind::Other),
                ("b", TokenKind::Word),
            ]
        );
    }

    #[test]
    fn combining_mark_is_its_own_other_token() {
        // "e" + U+0301 COMBINING ACUTE ACCENT: the mark is not alphanumeric, so it
        // tokenizes separately from the base letter under the spec's literal
        // "Unicode alphanumeric or _" word definition.
        let s = "e\u{0301}";
        let tokens = tokenize(s);
        let rendered: Vec<(&str, TokenKind)> = tokens.iter().map(|t| (t.text(s), t.kind)).collect();
        assert_eq!(
            rendered,
            vec![("e", TokenKind::Word), ("\u{0301}", TokenKind::Other)]
        );
    }

    #[test]
    fn utf16_offsets_for_ascii() {
        let idx = Utf16Index::new("abc");
        assert_eq!(idx.to_utf16(0), 0);
        assert_eq!(idx.to_utf16(1), 1);
        assert_eq!(idx.to_utf16(3), 3);
    }

    #[test]
    fn utf16_offsets_for_emoji_surrogate_pair() {
        let s = "a😀b";
        let idx = Utf16Index::new(s);
        // 'a' = 1 byte / 1 utf16 unit; emoji = 4 bytes / 2 utf16 units (surrogate pair).
        assert_eq!(idx.to_utf16(0), 0); // before 'a'
        assert_eq!(idx.to_utf16(1), 1); // before emoji
        assert_eq!(idx.to_utf16(5), 3); // before 'b' (1 + 2 surrogate units)
        assert_eq!(idx.to_utf16(6), 4); // end of string
    }

    #[test]
    fn utf16_offsets_for_cjk() {
        let s = "naïve 你好 x";
        let idx = Utf16Index::new(s);
        // Every char here is in the BMP, so byte-count differs from utf16-count only
        // where chars are multi-byte in UTF-8 but always 1 unit in UTF-16.
        let end = idx.to_utf16(s.len());
        assert_eq!(end, s.chars().count());
    }
}
