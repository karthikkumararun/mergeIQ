use crate::encoding::decode;
use crate::error::{MergeError, Side};
use crate::text::split_lines;

const OURS_MARKER: &str = "<<<<<<<";
const BASE_MARKER: &str = "|||||||";
const SEP_MARKER: &str = "=======";
const THEIRS_MARKER: &str = ">>>>>>>";

/// One conflict region found while parsing conflict markers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ConflictRegion {
    /// Label from the `<<<<<<<` line, if any.
    pub ours_label: Option<String>,
    /// Label from the `>>>>>>>` line, if any.
    pub theirs_label: Option<String>,
    /// `false` for two-way markers (no `|||||||` section); the base text contributes
    /// nothing for this region in that case.
    pub has_base: bool,
}

/// Base/ours/theirs texts reconstructed from a file containing conflict markers,
/// plus metadata about each conflict region found (in order).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MarkerParse {
    /// Reconstructed base text (missing content for any two-way-marker region).
    pub base: String,
    /// Reconstructed ours text.
    pub ours: String,
    /// Reconstructed theirs text.
    pub theirs: String,
    /// Each conflict region found, in file order.
    pub regions: Vec<ConflictRegion>,
}

/// Returns `Some(label)` if `content` is exactly `token`, or `token` followed by a
/// single space and a label; `None` if `content` isn't a marker line for `token`.
/// Per spec, a marker is only recognized at line start followed by a space or EOL.
fn match_marker(content: &str, token: &str) -> Option<Option<String>> {
    if content == token {
        return Some(None);
    }
    content
        .strip_prefix(token)
        .and_then(|rest| rest.strip_prefix(' '))
        .map(|label| Some(label.to_string()))
}

#[derive(PartialEq, Eq)]
enum State {
    Outside,
    Ours,
    Base,
    Theirs,
}

/// Parses a file containing git conflict markers (merge-style `<<<<<<</=======/>>>>>>>`
/// or diff3/zdiff3-style with an added `|||||||` base section) into base/ours/theirs
/// texts. A `|||||||` section's content is captured verbatim as the base text for
/// that region even under zdiff3 (which stores a diff there, not literal base text).
pub fn parse_markers(bytes: &[u8]) -> Result<MarkerParse, MergeError> {
    let (text, _enc) = decode(bytes, Side::Ours)?;
    let lines = split_lines(&text);

    let mut base = String::new();
    let mut ours = String::new();
    let mut theirs = String::new();
    let mut regions = Vec::new();

    let mut state = State::Outside;
    let mut ours_label: Option<String> = None;
    let mut has_base = false;
    let mut start_line = 0usize;

    for (idx, line) in lines.iter().enumerate() {
        let content = line.text(&text);
        let piece = line.with_terminator(&text);

        match state {
            State::Outside => {
                if let Some(label) = match_marker(content, OURS_MARKER) {
                    state = State::Ours;
                    ours_label = label;
                    has_base = false;
                    start_line = idx;
                    continue;
                }
                base.push_str(&piece);
                ours.push_str(&piece);
                theirs.push_str(&piece);
            }
            State::Ours => {
                if match_marker(content, BASE_MARKER).is_some() {
                    state = State::Base;
                    has_base = true;
                    continue;
                }
                if content == SEP_MARKER {
                    state = State::Theirs;
                    continue;
                }
                ours.push_str(&piece);
            }
            State::Base => {
                if content == SEP_MARKER {
                    state = State::Theirs;
                    continue;
                }
                base.push_str(&piece);
            }
            State::Theirs => {
                if let Some(theirs_label) = match_marker(content, THEIRS_MARKER) {
                    regions.push(ConflictRegion {
                        ours_label: ours_label.take(),
                        theirs_label,
                        has_base,
                    });
                    state = State::Outside;
                    continue;
                }
                theirs.push_str(&piece);
            }
        }
    }

    if state != State::Outside {
        return Err(MergeError::MalformedMarkers { line: start_line });
    }

    Ok(MarkerParse {
        base,
        ours,
        theirs,
        regions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{analyze, MergeInput, Options};
    use crate::diff3::ChunkKind;

    #[test]
    fn diff3_style_markers_round_trip_into_one_conflict() {
        let marked =
            "a\nb\n<<<<<<< HEAD\nOURS\n||||||| base\nBASE\n=======\nTHEIRS\n>>>>>>> branch\nc\n";
        let parse = parse_markers(marked.as_bytes()).unwrap();

        assert_eq!(parse.regions.len(), 1);
        assert_eq!(parse.regions[0].ours_label, Some("HEAD".to_string()));
        assert_eq!(parse.regions[0].theirs_label, Some("branch".to_string()));
        assert!(parse.regions[0].has_base);
        assert_eq!(parse.base, "a\nb\nBASE\nc\n");
        assert_eq!(parse.ours, "a\nb\nOURS\nc\n");
        assert_eq!(parse.theirs, "a\nb\nTHEIRS\nc\n");

        let analysis = analyze(
            MergeInput {
                base: parse.base.as_bytes(),
                ours: parse.ours.as_bytes(),
                theirs: parse.theirs.as_bytes(),
            },
            &Options::default(),
        )
        .unwrap();
        assert_eq!(analysis.chunks.len(), 1);
        assert_eq!(analysis.chunks[0].kind, ChunkKind::Conflict);
    }

    #[test]
    fn two_way_markers_have_no_base() {
        let marked = "a\n<<<<<<< ours\nOURS\n=======\nTHEIRS\n>>>>>>> theirs\n";
        let parse = parse_markers(marked.as_bytes()).unwrap();
        assert_eq!(parse.regions.len(), 1);
        assert!(!parse.regions[0].has_base);
        assert_eq!(parse.base, "a\n");
        assert_eq!(parse.ours, "a\nOURS\n");
        assert_eq!(parse.theirs, "a\nTHEIRS\n");
    }

    #[test]
    fn malformed_unclosed_start_marker() {
        let marked = "a\n<<<<<<< ours\nOURS\n=======\nTHEIRS\n";
        let err = parse_markers(marked.as_bytes()).unwrap_err();
        assert_eq!(err, MergeError::MalformedMarkers { line: 1 });
    }

    #[test]
    fn nested_looking_content_is_not_a_marker() {
        let marked = "x <<<<<<< y\nb\n";
        let parse = parse_markers(marked.as_bytes()).unwrap();
        assert_eq!(parse.regions.len(), 0);
        assert_eq!(parse.base, marked);
        assert_eq!(parse.ours, marked);
        assert_eq!(parse.theirs, marked);
    }

    #[test]
    fn marker_without_label_has_none() {
        let marked = "<<<<<<<\nOURS\n=======\nTHEIRS\n>>>>>>>\n";
        let parse = parse_markers(marked.as_bytes()).unwrap();
        assert_eq!(parse.regions[0].ours_label, None);
        assert_eq!(parse.regions[0].theirs_label, None);
    }
}
