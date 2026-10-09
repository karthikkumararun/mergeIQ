use crate::diff::{diff_lines, line_keys, WhitespacePolicy};
use crate::diff3::{merge, Chunk, ChunkKind, LineRange};
use crate::encoding::{decode, EncodingInfo};
use crate::error::{MergeError, Side};
use crate::fine::fine_diff;
use crate::simple::resolve_simple;
use crate::text::{dominant_eol, split_lines, Line, Terminator};

/// The text of a chunk's range on one side: lines `[range.start, range.end)` joined
/// with their internal terminators, excluding the final line's own terminator. An
/// empty range yields an empty slice at the insertion point.
fn chunk_text<'a>(text: &'a str, lines: &[Line], range: LineRange) -> &'a str {
    if range.is_empty() {
        let pos = if (range.start as usize) < lines.len() {
            lines[range.start as usize].start
        } else {
            text.len()
        };
        return &text[pos..pos];
    }
    let start = lines[range.start as usize].start;
    let end = lines[range.end as usize - 1].end;
    &text[start..end]
}

/// Appends lines `[range.start, range.end)` to `out`, each with its own original
/// terminator (unlike [`chunk_text`], the final line's terminator is included too —
/// needed for whole-document reconstruction across range boundaries).
fn push_range(out: &mut String, text: &str, lines: &[Line], range: LineRange) {
    for line in &lines[range.as_usize_range()] {
        out.push_str(&line.with_terminator(text));
    }
}

/// Applies every non-conflicting chunk (ours/theirs where only one side changed,
/// either side's text for `BothSame`) plus unchanged gaps from base, reconstructing
/// the merged document. Returns `None` if any chunk is a `Conflict` — use that
/// chunk's `simple` resolution, or ask the user, for those instead.
pub fn apply_non_conflicting(analysis: &Analysis) -> Option<String> {
    if analysis
        .chunks
        .iter()
        .any(|c| c.kind == ChunkKind::Conflict)
    {
        return None;
    }

    let mut result = String::new();
    let mut base_pos = 0u32;
    for chunk in &analysis.chunks {
        if chunk.base.start > base_pos {
            push_range(
                &mut result,
                &analysis.base.text,
                &analysis.base.lines,
                LineRange {
                    start: base_pos,
                    end: chunk.base.start,
                },
            );
        }
        match chunk.kind {
            ChunkKind::OursOnly | ChunkKind::BothSame => {
                push_range(
                    &mut result,
                    &analysis.ours.text,
                    &analysis.ours.lines,
                    chunk.ours,
                );
            }
            ChunkKind::TheirsOnly => {
                push_range(
                    &mut result,
                    &analysis.theirs.text,
                    &analysis.theirs.lines,
                    chunk.theirs,
                );
            }
            ChunkKind::Conflict => unreachable!("checked above"),
        }
        base_pos = chunk.base.end;
    }
    let base_len = analysis.base.lines.len() as u32;
    if base_len > base_pos {
        push_range(
            &mut result,
            &analysis.base.text,
            &analysis.base.lines,
            LineRange {
                start: base_pos,
                end: base_len,
            },
        );
    }
    Some(result)
}

/// The three raw byte inputs to a 3-way merge analysis.
///
/// Borrows its input rather than owning it, since it's only ever used for the
/// duration of one [`analyze`] call; not meant to cross an IPC boundary itself (the
/// bytes it borrows would be passed as plain command arguments instead).
#[derive(Debug, Clone, Copy)]
pub struct MergeInput<'a> {
    /// The common ancestor revision's bytes.
    pub base: &'a [u8],
    /// The current/working revision's bytes.
    pub ours: &'a [u8],
    /// The incoming revision's bytes.
    pub theirs: &'a [u8],
}

/// Settings controlling how [`analyze`] diffs and classifies chunks.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Options {
    /// How line content is normalized for equality when diffing.
    pub whitespace: WhitespacePolicy,
    /// Chunks spanning more lines than this (per side) skip fine-grained diffing.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub fine_diff_max_lines: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            whitespace: WhitespacePolicy::Exact,
            fine_diff_max_lines: 2000,
        }
    }
}

/// One side's decoded text and line table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SideText {
    /// The decoded text.
    pub text: String,
    /// `text`'s line table.
    pub lines: Vec<Line>,
}

/// The full result of a 3-way merge analysis: decoded sides plus classified chunks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Analysis {
    /// The base side.
    pub base: SideText,
    /// The ours side.
    pub ours: SideText,
    /// The theirs side.
    pub theirs: SideText,
    /// Every changed region, in base order.
    pub chunks: Vec<Chunk>,
    /// Encoding/BOM to use when serializing the result (see [`analyze`]'s doc for
    /// which side this is derived from).
    pub encoding: EncodingInfo,
    /// The result's dominant line terminator, for use on user-typed lines.
    pub dominant_eol: Terminator,
}

/// Decodes, diffs and classifies `input` into an [`Analysis`]. Pure and deterministic
/// (no IO). Returns `MergeError::Binary`/`Decode` if any side can't be decoded as text.
pub fn analyze(input: MergeInput<'_>, opts: &Options) -> Result<Analysis, MergeError> {
    let (base_text, _base_enc) = decode(input.base, Side::Base)?;
    let (ours_text, ours_enc) = decode(input.ours, Side::Ours)?;
    let (theirs_text, _theirs_enc) = decode(input.theirs, Side::Theirs)?;

    let base_lines = split_lines(&base_text);
    let ours_lines = split_lines(&ours_text);
    let theirs_lines = split_lines(&theirs_text);

    let base_keys = line_keys(&base_text, &base_lines, opts.whitespace);
    let ours_keys = line_keys(&ours_text, &ours_lines, opts.whitespace);
    let theirs_keys = line_keys(&theirs_text, &theirs_lines, opts.whitespace);

    let ours_hunks = diff_lines(&base_keys, &ours_keys);
    let theirs_hunks = diff_lines(&base_keys, &theirs_keys);
    let mut chunks = merge(&ours_hunks, &theirs_hunks, &ours_keys, &theirs_keys);

    for chunk in &mut chunks {
        let base_slice = chunk_text(&base_text, &base_lines, chunk.base);
        let ours_slice = chunk_text(&ours_text, &ours_lines, chunk.ours);
        let theirs_slice = chunk_text(&theirs_text, &theirs_lines, chunk.theirs);

        let too_big = chunk.base.len() as usize > opts.fine_diff_max_lines
            || chunk.ours.len() as usize > opts.fine_diff_max_lines
            || chunk.theirs.len() as usize > opts.fine_diff_max_lines;

        if too_big {
            chunk.fine_diff_skipped = true;
        } else {
            chunk.fine = Some(fine_diff(base_slice, ours_slice, theirs_slice));
        }

        if chunk.kind == ChunkKind::Conflict {
            chunk.simple = Some(resolve_simple(base_slice, ours_slice, theirs_slice));
        }
    }

    // All three sides are expected to share one encoding in practice, being
    // revisions of the same file; `ours` (the working copy) wins if they somehow
    // differ, since the result is serialized based on it.
    let dominant = dominant_eol(
        base_lines
            .iter()
            .chain(ours_lines.iter())
            .chain(theirs_lines.iter())
            .map(|l| l.term),
    );

    Ok(Analysis {
        base: SideText {
            text: base_text,
            lines: base_lines,
        },
        ours: SideText {
            text: ours_text,
            lines: ours_lines,
        },
        theirs: SideText {
            text: theirs_text,
            lines: theirs_lines,
        },
        chunks,
        encoding: ours_enc,
        dominant_eol: dominant,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff3::{ChunkKind, LineRange};

    fn run(base: &str, ours: &str, theirs: &str, whitespace: WhitespacePolicy) -> Analysis {
        let opts = Options {
            whitespace,
            ..Options::default()
        };
        analyze(
            MergeInput {
                base: base.as_bytes(),
                ours: ours.as_bytes(),
                theirs: theirs.as_bytes(),
            },
            &opts,
        )
        .unwrap()
    }

    #[test]
    fn no_changes_yields_no_chunks() {
        let base = "a\nb\nc\n";
        assert_eq!(
            run(base, base, base, WhitespacePolicy::Exact).chunks,
            vec![]
        );
    }

    #[test]
    fn non_overlapping_edits() {
        let base: String = (1..=20).map(|n| format!("l{n}\n")).collect();
        let ours = base.replacen("l2\n", "L2\n", 1);
        let theirs = base.replacen("l10\n", "L10\n", 1);
        let chunks = run(&base, &ours, &theirs, WhitespacePolicy::Exact).chunks;
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::OursOnly);
        assert_eq!(chunks[0].base, LineRange { start: 1, end: 2 });
        assert_eq!(chunks[1].kind, ChunkKind::TheirsOnly);
        assert_eq!(chunks[1].base, LineRange { start: 9, end: 10 });
    }

    #[test]
    fn identical_edits() {
        let base = "a\nb\nc\n";
        let changed = "a\nB\nc\n";
        let chunks = run(base, changed, changed, WhitespacePolicy::Exact).chunks;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::BothSame);
    }

    #[test]
    fn overlapping_edits() {
        let base = "l1\nl2\nl3\nl4\nl5\nl6\nl7\n";
        let ours = "l1\nl2\nL3\nL4\nL5\nl6\nl7\n";
        let theirs = "l1\nl2\nl3\nl4\nX5\nX6\nX7\n";
        let chunks = run(base, ours, theirs, WhitespacePolicy::Exact).chunks;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Conflict);
        assert_eq!(chunks[0].base, LineRange { start: 2, end: 7 });
    }

    #[test]
    fn insertions_at_the_same_point() {
        let base = "l1\nl2\nl3\nl4\nl5\n";
        let ours = "l1\nl2\nl3\nl4\nOURS\nl5\n";
        let theirs = "l1\nl2\nl3\nl4\nTHEIRS\nl5\n";
        let chunks = run(base, ours, theirs, WhitespacePolicy::Exact).chunks;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Conflict);
        assert_eq!(chunks[0].base, LineRange { start: 4, end: 4 });
    }

    #[test]
    fn trailing_whitespace_ignored_is_theirs_only() {
        let base = "a\nb\nc\n";
        let ours = "a\nb  \nc\n"; // trailing spaces only
        let theirs = "a\nB\nc\n"; // real content change
        let chunks = run(base, ours, theirs, WhitespacePolicy::TrimTrailing).chunks;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::TheirsOnly);
    }

    #[test]
    fn exact_policy_sees_trailing_whitespace_as_a_conflict() {
        let base = "a\nb\nc\n";
        let ours = "a\nb  \nc\n";
        let theirs = "a\nB\nc\n";
        let chunks = run(base, ours, theirs, WhitespacePolicy::Exact).chunks;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Conflict);
    }

    #[test]
    fn binary_input_is_rejected() {
        let err = analyze(
            MergeInput {
                base: b"hello\0world",
                ours: b"a",
                theirs: b"a",
            },
            &Options::default(),
        )
        .unwrap_err();
        assert_eq!(err, MergeError::Binary { side: Side::Base });
    }

    #[test]
    fn apply_non_conflicting_reconstructs_merged_document() {
        let base: String = (1..=20).map(|n| format!("l{n}\n")).collect();
        let ours = base.replacen("l2\n", "L2\n", 1);
        let theirs = base.replacen("l10\n", "L10\n", 1);
        let analysis = analyze(
            MergeInput {
                base: base.as_bytes(),
                ours: ours.as_bytes(),
                theirs: theirs.as_bytes(),
            },
            &Options::default(),
        )
        .unwrap();
        let merged = apply_non_conflicting(&analysis).unwrap();
        let expected: String = base
            .replacen("l2\n", "L2\n", 1)
            .replacen("l10\n", "L10\n", 1);
        assert_eq!(merged, expected);
    }

    #[test]
    fn apply_non_conflicting_is_none_for_conflicts() {
        let analysis = run("a\n", "b\n", "c\n", WhitespacePolicy::Exact);
        assert!(apply_non_conflicting(&analysis).is_none());
    }
}
