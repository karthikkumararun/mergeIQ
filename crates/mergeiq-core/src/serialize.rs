use crate::encoding::{encode, EncodingInfo};
use crate::error::MergeError;
use crate::text::Terminator;

/// One line of a result document: its text content plus the terminator to write
/// after it (use [`Terminator::None`] only for the file's final line).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ResultLine {
    /// The line's content, excluding its terminator.
    pub text: String,
    /// The terminator to write after `text`.
    pub term: Terminator,
}

/// An unresolved conflict to splice into the output when [`SerializeOptions::emit_markers`]
/// is set. `at` is the index into the `lines` slice given to [`serialize`] where the
/// conflict markers should be inserted.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UnresolvedConflict {
    /// The chunk this conflict came from.
    pub chunk_id: u32,
    /// Index into `lines` (given to [`serialize`]) where this conflict belongs.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: usize,
    /// Label for the `<<<<<<<` marker line.
    pub ours_label: String,
    /// The ours-side lines to write inside the markers.
    pub ours_lines: Vec<ResultLine>,
    /// Label for the `>>>>>>>` marker line.
    pub theirs_label: String,
    /// The theirs-side lines to write inside the markers.
    pub theirs_lines: Vec<ResultLine>,
    /// Present when the original conflict had a base section; only rendered when
    /// [`SerializeOptions::diff3_style`] is also set.
    pub base_lines: Option<Vec<ResultLine>>,
}

/// Options controlling how [`serialize`] handles unresolved conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SerializeOptions {
    /// When `false`, any unresolved conflict makes [`serialize`] return
    /// `MergeError::Unresolved` instead of writing markers.
    pub emit_markers: bool,
    /// When `true` (and emitting markers), include a `|||||||` base section for
    /// conflicts that have one.
    pub diff3_style: bool,
}

fn push_lines(out: &mut String, lines: &[ResultLine]) {
    for line in lines {
        out.push_str(&line.text);
        out.push_str(line.term.as_str());
    }
}

/// Serializes `lines` (the resolved document) to bytes, re-encoding with `enc`.
/// `unresolved` conflicts (if any) are either spliced in as conflict markers (when
/// `opts.emit_markers`) or cause `MergeError::Unresolved`.
pub fn serialize(
    lines: &[ResultLine],
    unresolved: &[UnresolvedConflict],
    enc: &EncodingInfo,
    opts: &SerializeOptions,
) -> Result<Vec<u8>, MergeError> {
    if !unresolved.is_empty() && !opts.emit_markers {
        return Err(MergeError::Unresolved {
            chunk_ids: unresolved.iter().map(|u| u.chunk_id).collect(),
        });
    }

    let mut sorted: Vec<&UnresolvedConflict> = unresolved.iter().collect();
    sorted.sort_by_key(|u| u.at);

    let mut text = String::new();
    let mut cursor = 0usize;
    for u in &sorted {
        push_lines(&mut text, &lines[cursor..u.at]);
        cursor = u.at;

        text.push_str("<<<<<<< ");
        text.push_str(&u.ours_label);
        text.push('\n');
        push_lines(&mut text, &u.ours_lines);

        if opts.diff3_style {
            if let Some(base_lines) = &u.base_lines {
                text.push_str("|||||||\n");
                push_lines(&mut text, base_lines);
            }
        }

        text.push_str("=======\n");
        push_lines(&mut text, &u.theirs_lines);

        text.push_str(">>>>>>> ");
        text.push_str(&u.theirs_label);
        text.push('\n');
    }
    push_lines(&mut text, &lines[cursor..]);

    Ok(encode(&text, enc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{decode, Encoding};
    use crate::error::Side;
    use crate::text::split_lines;

    fn to_result_lines(text: &str) -> Vec<ResultLine> {
        split_lines(text)
            .iter()
            .map(|l| ResultLine {
                text: l.text(text).to_string(),
                term: l.term,
            })
            .collect()
    }

    #[test]
    fn no_conflicts_round_trips_byte_exact() {
        let mut original = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        original.extend_from_slice("a\nb\nc\n".as_bytes());
        let (text, enc) = decode(&original, Side::Base).unwrap();
        let lines = to_result_lines(&text);

        let out = serialize(
            &lines,
            &[],
            &enc,
            &SerializeOptions {
                emit_markers: true,
                diff3_style: true,
            },
        )
        .unwrap();
        assert_eq!(out, original);
    }

    #[test]
    fn utf16_round_trips_byte_exact() {
        let enc = EncodingInfo {
            encoding: Encoding::Utf16Le,
            bom: true,
        };
        let text = "a\nb\n";
        let mut expected = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            expected.extend_from_slice(&unit.to_le_bytes());
        }

        let out = serialize(
            &to_result_lines(text),
            &[],
            &enc,
            &SerializeOptions {
                emit_markers: true,
                diff3_style: false,
            },
        )
        .unwrap();
        assert_eq!(out, expected);
    }

    #[test]
    fn unresolved_with_markers() {
        let lines = to_result_lines("a\nc\n");
        let unresolved = vec![UnresolvedConflict {
            chunk_id: 0,
            at: 1,
            ours_label: "HEAD".to_string(),
            ours_lines: to_result_lines("OURS\n"),
            theirs_label: "branch".to_string(),
            theirs_lines: to_result_lines("THEIRS\n"),
            base_lines: None,
        }];
        let (_, enc) = decode(b"a\n", Side::Base).unwrap();
        let out = serialize(
            &lines,
            &unresolved,
            &enc,
            &SerializeOptions {
                emit_markers: true,
                diff3_style: false,
            },
        )
        .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert_eq!(
            out,
            "a\n<<<<<<< HEAD\nOURS\n=======\nTHEIRS\n>>>>>>> branch\nc\n"
        );
    }

    #[test]
    fn unresolved_without_markers_errors() {
        let lines = to_result_lines("a\nc\n");
        let unresolved = vec![UnresolvedConflict {
            chunk_id: 7,
            at: 1,
            ours_label: "HEAD".to_string(),
            ours_lines: to_result_lines("OURS\n"),
            theirs_label: "branch".to_string(),
            theirs_lines: to_result_lines("THEIRS\n"),
            base_lines: None,
        }];
        let (_, enc) = decode(b"a\n", Side::Base).unwrap();
        let err = serialize(
            &lines,
            &unresolved,
            &enc,
            &SerializeOptions {
                emit_markers: false,
                diff3_style: false,
            },
        )
        .unwrap_err();
        assert_eq!(err, MergeError::Unresolved { chunk_ids: vec![7] });
    }
}
