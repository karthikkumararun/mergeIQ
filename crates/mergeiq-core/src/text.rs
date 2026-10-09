use crate::encoding::EncodingInfo;

/// A line's original terminator, preserved for byte-exact round-tripping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Terminator {
    /// `\n`.
    Lf,
    /// `\r\n`.
    Crlf,
    /// A lone `\r`.
    Cr,
    /// The final line of a file with no trailing terminator.
    None,
}

impl Terminator {
    /// The literal terminator bytes (`""` for [`Terminator::None`]).
    pub fn as_str(self) -> &'static str {
        match self {
            Terminator::Lf => "\n",
            Terminator::Crlf => "\r\n",
            Terminator::Cr => "\r",
            Terminator::None => "",
        }
    }

    /// Byte length of [`Self::as_str`].
    pub fn len(self) -> usize {
        self.as_str().len()
    }

    /// `true` for [`Terminator::None`].
    pub fn is_empty(self) -> bool {
        self == Terminator::None
    }
}

/// A line's content range `[start, end)` (byte offsets, terminator excluded) plus its
/// original terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Line {
    /// Byte offset of the line's first content byte.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub start: usize,
    /// Byte offset just past the line's last content byte (terminator excluded).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub end: usize,
    /// This line's original terminator.
    pub term: Terminator,
}

impl Line {
    /// The line's content, excluding its terminator.
    pub fn text<'a>(&self, full: &'a str) -> &'a str {
        &full[self.start..self.end]
    }

    /// The line's content plus its original terminator.
    pub fn with_terminator<'a>(&self, full: &'a str) -> std::borrow::Cow<'a, str> {
        if self.term == Terminator::None {
            std::borrow::Cow::Borrowed(self.text(full))
        } else {
            std::borrow::Cow::Owned(format!("{}{}", self.text(full), self.term.as_str()))
        }
    }
}

/// Decoded text plus its line table. Produced from raw bytes via [`crate::decode`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Decoded {
    /// The decoded text.
    pub text: String,
    /// The encoding/BOM `text` was decoded from.
    pub encoding: EncodingInfo,
    /// `text`'s line table (see [`split_lines`]).
    pub lines: Vec<Line>,
}

impl Decoded {
    /// Decodes `text`'s line table and bundles it with `text` and `encoding`.
    pub fn new(text: String, encoding: EncodingInfo) -> Self {
        let lines = split_lines(&text);
        Self {
            text,
            encoding,
            lines,
        }
    }
}

/// Splits `text` into lines, keeping each line's original terminator. An empty string
/// yields zero lines. A trailing terminator does not produce a phantom empty final line.
pub fn split_lines(text: &str) -> Vec<Line> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                lines.push(Line {
                    start,
                    end: i,
                    term: Terminator::Lf,
                });
                i += 1;
                start = i;
            }
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    lines.push(Line {
                        start,
                        end: i,
                        term: Terminator::Crlf,
                    });
                    i += 2;
                } else {
                    lines.push(Line {
                        start,
                        end: i,
                        term: Terminator::Cr,
                    });
                    i += 1;
                }
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < bytes.len() {
        lines.push(Line {
            start,
            end: bytes.len(),
            term: Terminator::None,
        });
    }
    lines
}

/// The most common terminator among `terms`, defaulting to `Lf` when none are
/// terminated (empty input, or lines with no trailing terminator). Accepts an
/// iterator so callers can combine terminators from multiple sides' line tables.
pub fn dominant_eol(terms: impl IntoIterator<Item = Terminator>) -> Terminator {
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    for term in terms {
        match term {
            Terminator::Lf => lf += 1,
            Terminator::Crlf => crlf += 1,
            Terminator::Cr => cr += 1,
            Terminator::None => {}
        }
    }
    if crlf > lf && crlf > cr {
        Terminator::Crlf
    } else if cr > lf && cr > crlf {
        Terminator::Cr
    } else {
        Terminator::Lf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_has_no_lines() {
        assert_eq!(split_lines(""), vec![]);
    }

    #[test]
    fn lf_only() {
        let lines = split_lines("a\nb\n");
        assert_eq!(
            lines,
            vec![
                Line {
                    start: 0,
                    end: 1,
                    term: Terminator::Lf
                },
                Line {
                    start: 2,
                    end: 3,
                    term: Terminator::Lf
                },
            ]
        );
    }

    #[test]
    fn crlf_only() {
        let lines = split_lines("a\r\nb\r\n");
        assert_eq!(
            lines,
            vec![
                Line {
                    start: 0,
                    end: 1,
                    term: Terminator::Crlf
                },
                Line {
                    start: 3,
                    end: 4,
                    term: Terminator::Crlf
                },
            ]
        );
    }

    #[test]
    fn lone_cr_is_its_own_terminator() {
        let lines = split_lines("a\rb");
        assert_eq!(
            lines,
            vec![
                Line {
                    start: 0,
                    end: 1,
                    term: Terminator::Cr
                },
                Line {
                    start: 2,
                    end: 3,
                    term: Terminator::None
                },
            ]
        );
    }

    #[test]
    fn missing_final_newline() {
        let lines = split_lines("a\nb");
        assert_eq!(
            lines,
            vec![
                Line {
                    start: 0,
                    end: 1,
                    term: Terminator::Lf
                },
                Line {
                    start: 2,
                    end: 3,
                    term: Terminator::None
                },
            ]
        );
        assert_eq!(lines[1].text("a\nb"), "b");
    }

    #[test]
    fn with_terminator_reconstructs_original_bytes() {
        let text = "a\r\nb";
        let lines = split_lines(text);
        let rebuilt: String = lines.iter().map(|l| l.with_terminator(text)).collect();
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn dominant_eol_picks_majority() {
        let text = "a\nb\nc\r\n";
        assert_eq!(
            dominant_eol(split_lines(text).iter().map(|l| l.term)),
            Terminator::Lf
        );
    }

    #[test]
    fn dominant_eol_defaults_to_lf_when_untermianted() {
        assert_eq!(
            dominant_eol(split_lines("a").iter().map(|l| l.term)),
            Terminator::Lf
        );
        assert_eq!(dominant_eol(std::iter::empty()), Terminator::Lf);
    }
}
