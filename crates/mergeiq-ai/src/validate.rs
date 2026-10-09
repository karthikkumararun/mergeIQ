//! Post-processing of a model's resolution: line-ending and fence clean-up, a conflict-marker
//! check and the syntax check the spec requires. Nothing here applies anything.

use mergeiq_struct::{parse::parse, Lang};
use serde::{Deserialize, Serialize};

use crate::context::{ContextInput, Span};
use crate::schema::Suggestion;

/// The message shown when a suggestion adds syntax errors.
pub const SYNTAX_WARNING: &str = "Suggestion may not compile";

/// A suggestion with its checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    /// The suggestion, with a cleaned `resolution`.
    pub suggestion: Suggestion,
    /// Set when the file with the suggestion applied has more syntax errors than any input.
    pub syntax_warning: Option<String>,
    /// Other things worth showing: conflict markers, changes made to the model's text.
    pub notes: Vec<String>,
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// `text` with the lines in `span` replaced by `replacement`.
pub fn replace_lines(text: &str, span: Span, replacement: &str) -> String {
    let lines = lines_of(text);
    let start = (span.start as usize).min(lines.len());
    let end = (span.end as usize).clamp(start, lines.len());
    let mut out = lines[..start].concat();
    out.push_str(replacement);
    out.push_str(&lines[end..].concat());
    out
}

fn dominant_eol_is_crlf(text: &str) -> bool {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    crlf > lf
}

/// Removes a fence the model wrapped around the whole answer, unless the original code has one.
fn strip_fence(resolution: &str, originals: &[&str]) -> Option<String> {
    if originals.iter().any(|o| o.contains("```")) {
        return None;
    }
    let trimmed = resolution.trim();
    let mut lines = trimmed.lines();
    let first = lines.next()?;
    if !first.starts_with("```") || trimmed.lines().count() < 2 {
        return None;
    }
    let last = trimmed.lines().last()?;
    if last.trim() != "```" {
        return None;
    }
    let inner: Vec<&str> = trimmed.lines().skip(1).collect();
    let inner = &inner[..inner.len() - 1];
    let mut out = inner.join("\n");
    if !inner.is_empty() {
        out.push('\n');
    }
    Some(out)
}

/// Whether any line starts a conflict marker.
pub fn has_conflict_markers(text: &str) -> bool {
    text.lines().any(|l| {
        l.starts_with("<<<<<<< ")
            || l == "<<<<<<<"
            || l.starts_with(">>>>>>> ")
            || l == ">>>>>>>"
            || l == "======="
            || l.starts_with("||||||| ")
    })
}

/// Cleans `suggestion.resolution` for the document it will be written into and runs checks.
pub fn check(input: &ContextInput, suggestion: Suggestion) -> Checked {
    use crate::context::Span as S;
    let mut notes = Vec::new();
    let mut resolution = suggestion.resolution.clone();

    let lines_in = |text: &str, span: S| -> String {
        let l = lines_of(text);
        let s = (span.start as usize).min(l.len());
        let e = (span.end as usize).clamp(s, l.len());
        l[s..e].concat()
    };
    let base_c = lines_in(&input.base, input.chunk.base);
    let left_c = lines_in(&input.left.text, input.chunk.left);
    let right_c = lines_in(&input.right.text, input.chunk.right);
    let current = lines_in(&input.result, input.chunk.result);

    if let Some(stripped) = strip_fence(&resolution, &[&base_c, &left_c, &right_c]) {
        resolution = stripped;
        notes.push("Removed a code fence around the suggestion".to_string());
    }

    // Match the file's line endings.
    let crlf = dominant_eol_is_crlf(&input.result);
    let normalized = resolution.replace("\r\n", "\n");
    resolution = if crlf {
        normalized.replace('\n', "\r\n")
    } else {
        normalized
    };

    // Keep the region's trailing line break: the original lines end with one, the text after
    // them would otherwise be glued on.
    let original_ends_with_break = current.ends_with('\n')
        || (current.is_empty()
            && lines_of(&input.result).len() > input.chunk.result.start as usize);
    if !resolution.is_empty() && original_ends_with_break && !resolution.ends_with('\n') {
        resolution.push_str(if crlf { "\r\n" } else { "\n" });
    }

    if has_conflict_markers(&resolution) {
        notes.push("The suggestion contains conflict markers".to_string());
    }

    let syntax_warning = syntax_warning(input, &resolution);

    Checked {
        suggestion: Suggestion {
            resolution,
            ..suggestion
        },
        syntax_warning,
        notes,
    }
}

fn syntax_warning(input: &ContextInput, resolution: &str) -> Option<String> {
    let lang = Lang::from_path(&input.path)?;
    let errors = |text: &str| parse(lang, text).map(|p| p.errors);
    let baseline = [&input.base, &input.left.text, &input.right.text]
        .iter()
        .map(|t| errors(t))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max()
        .unwrap_or(0);
    let candidate = replace_lines(&input.result, input.chunk.result, resolution);
    let after = errors(&candidate)?;
    (after > baseline).then(|| SYNTAX_WARNING.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ChunkSpans, SideInput};
    use crate::schema::{Confidence, Strategy};

    fn input(
        path: &str,
        base: &str,
        left: &str,
        right: &str,
        result: &str,
        spans: ChunkSpans,
    ) -> ContextInput {
        ContextInput {
            path: path.into(),
            base: base.into(),
            left: SideInput {
                label: "l".into(),
                text: left.into(),
                commits: vec![],
            },
            right: SideInput {
                label: "r".into(),
                text: right.into(),
                commits: vec![],
            },
            result: result.into(),
            chunk: spans,
        }
    }

    fn sug(resolution: &str) -> Suggestion {
        Suggestion {
            resolution: resolution.into(),
            explanation: "e".into(),
            confidence: Confidence::High,
            strategy: Strategy::New,
            risks: vec![],
        }
    }

    fn span(s: u32, e: u32) -> Span {
        Span { start: s, end: e }
    }

    fn java() -> ContextInput {
        let base = "class A {\n    void f() {\n        old();\n    }\n}\n";
        let left = "class A {\n    void f() {\n        left();\n    }\n}\n";
        let right = "class A {\n    void f() {\n        right();\n    }\n}\n";
        input(
            "A.java",
            base,
            left,
            right,
            base,
            ChunkSpans {
                base: span(2, 3),
                left: span(2, 3),
                right: span(2, 3),
                result: span(2, 3),
            },
        )
    }

    #[test]
    fn a_valid_java_resolution_has_no_warning() {
        let c = check(&java(), sug("        left();\n        right();\n"));
        assert_eq!(c.syntax_warning, None);
        assert!(c.notes.is_empty());
    }

    #[test]
    fn an_unclosed_brace_triggers_the_warning() {
        let c = check(&java(), sug("        if (x) {\n            left();\n"));
        assert_eq!(
            c.syntax_warning.as_deref(),
            Some("Suggestion may not compile")
        );
    }

    #[test]
    fn inputs_that_already_have_errors_raise_the_bar() {
        let mut i = java();
        // The base itself is broken (extra brace): the same breakage in the answer is no news.
        i.base = "class A {\n    void f() {\n        old();\n    }\n}\n}\n".into();
        i.result = i.base.clone();
        let c = check(&i, sug("        left();\n"));
        assert_eq!(c.syntax_warning, None);
    }

    #[test]
    fn unsupported_languages_are_not_checked() {
        let mut i = java();
        i.path = "notes.txt".into();
        assert_eq!(check(&i, sug("{{{{ (( ")).syntax_warning, None);
    }

    #[test]
    fn a_missing_trailing_newline_is_restored() {
        let c = check(&java(), sug("        left();"));
        assert_eq!(c.suggestion.resolution, "        left();\n");
    }

    #[test]
    fn crlf_files_get_crlf() {
        let mut i = java();
        i.result = i.result.replace('\n', "\r\n");
        let c = check(&i, sug("        a();\n        b();\n"));
        assert_eq!(c.suggestion.resolution, "        a();\r\n        b();\r\n");
        // Already-CRLF answers are not doubled.
        let c = check(&i, sug("        a();\r\n"));
        assert_eq!(c.suggestion.resolution, "        a();\r\n");
    }

    #[test]
    fn a_wrapping_fence_is_removed_but_a_real_one_is_kept() {
        let c = check(&java(), sug("```java\n        a();\n```"));
        assert_eq!(c.suggestion.resolution, "        a();\n");
        assert_eq!(
            c.notes,
            vec!["Removed a code fence around the suggestion".to_string()]
        );

        let mut md = input(
            "doc.md",
            "```\nold\n```\n",
            "```\nleft\n```\n",
            "```\nright\n```\n",
            "```\nold\n```\n",
            ChunkSpans {
                base: span(0, 3),
                left: span(0, 3),
                right: span(0, 3),
                result: span(0, 3),
            },
        );
        md.path = "doc.md".into();
        let c = check(&md, sug("```\nboth\n```\n"));
        assert_eq!(c.suggestion.resolution, "```\nboth\n```\n");
        assert!(c.notes.is_empty());
    }

    #[test]
    fn conflict_markers_are_called_out() {
        let c = check(
            &java(),
            sug("<<<<<<< HEAD\na();\n=======\nb();\n>>>>>>> x\n"),
        );
        assert!(c.notes.iter().any(|n| n.contains("conflict markers")));
        assert!(!has_conflict_markers("a = b == c;\nx ======= y\n"));
    }

    #[test]
    fn an_empty_resolution_deletes_the_region() {
        let c = check(&java(), sug(""));
        assert_eq!(c.suggestion.resolution, "");
        assert_eq!(replace_lines("a\nb\nc\n", span(1, 2), ""), "a\nc\n");
        assert_eq!(
            replace_lines("a\nb\nc\n", span(1, 1), "X\n"),
            "a\nX\nb\nc\n"
        );
        assert_eq!(replace_lines("a\nb", span(1, 2), "X"), "a\nX");
    }
}
