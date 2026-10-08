use crate::diff::diff_token_texts;
use crate::diff3::{merge, ChunkKind};
use crate::tokenize::{tokenize, Token};

/// Result of a token-level 3-way merge attempt for one conflict chunk.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SimpleResolution {
    /// Token-level edits from both sides didn't overlap or touch; this is the
    /// merged text.
    Resolved(String),
    /// Token-level edits overlapped or touched; needs a human.
    Unresolvable,
}

fn push_token_range(out: &mut String, tokens: &[Token], text: &str, start: u32, end: u32) {
    if start == end {
        return;
    }
    let s = tokens[start as usize].start;
    let e = tokens[end as usize - 1].end;
    out.push_str(&text[s..e]);
}

/// Attempts a token-level 3-way merge of a chunk's base/ours/theirs text (the
/// "magic wand" resolve-simple-conflicts operation). Reuses the same sweep-merge
/// algorithm as line-level chunk merging (see the `diff3` module), applied to tokens instead
/// of lines: if every resulting region touches at most one side, the merge succeeds;
/// any token-level `Conflict` region makes the whole chunk `Unresolvable`.
pub fn resolve_simple(base_text: &str, ours_text: &str, theirs_text: &str) -> SimpleResolution {
    let base_tokens = tokenize(base_text);
    let ours_tokens = tokenize(ours_text);
    let theirs_tokens = tokenize(theirs_text);

    let base_token_texts: Vec<&str> = base_tokens.iter().map(|t| t.text(base_text)).collect();
    let ours_token_texts: Vec<&str> = ours_tokens.iter().map(|t| t.text(ours_text)).collect();
    let theirs_token_texts: Vec<&str> = theirs_tokens.iter().map(|t| t.text(theirs_text)).collect();

    let ours_hunks = diff_token_texts(&base_token_texts, &ours_token_texts);
    let theirs_hunks = diff_token_texts(&base_token_texts, &theirs_token_texts);

    let ours_keys: Vec<String> = ours_token_texts.iter().map(|s| s.to_string()).collect();
    let theirs_keys: Vec<String> = theirs_token_texts.iter().map(|s| s.to_string()).collect();

    let chunks = merge(&ours_hunks, &theirs_hunks, &ours_keys, &theirs_keys);

    if chunks.iter().any(|c| c.kind == ChunkKind::Conflict) {
        return SimpleResolution::Unresolvable;
    }

    let mut result = String::new();
    let mut base_pos = 0u32;
    for chunk in &chunks {
        if chunk.base.start > base_pos {
            push_token_range(
                &mut result,
                &base_tokens,
                base_text,
                base_pos,
                chunk.base.start,
            );
        }
        match chunk.kind {
            ChunkKind::OursOnly | ChunkKind::BothSame => {
                push_token_range(
                    &mut result,
                    &ours_tokens,
                    ours_text,
                    chunk.ours.start,
                    chunk.ours.end,
                );
            }
            ChunkKind::TheirsOnly => {
                push_token_range(
                    &mut result,
                    &theirs_tokens,
                    theirs_text,
                    chunk.theirs.start,
                    chunk.theirs.end,
                );
            }
            ChunkKind::Conflict => unreachable!("handled above"),
        }
        base_pos = chunk.base.end;
    }
    let base_len = base_tokens.len() as u32;
    if base_len > base_pos {
        push_token_range(&mut result, &base_tokens, base_text, base_pos, base_len);
    }

    SimpleResolution::Resolved(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_words_on_the_same_line() {
        let result = resolve_simple("foo(a, b)", "foo(x, b)", "foo(a, y)");
        assert_eq!(result, SimpleResolution::Resolved("foo(x, y)".to_string()));
    }

    #[test]
    fn same_word_changed_differently() {
        let result = resolve_simple("foo(a)", "foo(x)", "foo(y)");
        assert_eq!(result, SimpleResolution::Unresolvable);
    }

    #[test]
    fn adjacent_token_edits_are_not_merged() {
        // Both sides modify the same base token ("ab" -> "AB" / "ab" -> "abX"),
        // which is a direct overlap at token granularity.
        let result = resolve_simple("ab cd", "AB cd", "abX cd");
        assert_eq!(result, SimpleResolution::Unresolvable);
    }

    #[test]
    fn no_edits_resolve_to_base() {
        let result = resolve_simple("foo(a, b)", "foo(a, b)", "foo(a, b)");
        assert_eq!(result, SimpleResolution::Resolved("foo(a, b)".to_string()));
    }

    #[test]
    fn realistic_cases() {
        let cases: &[(&str, &str, &str, Option<&str>)] = &[
            ("x = 1;", "x = 2;", "y = 1;", Some("y = 2;")),
            (
                "return a + b;",
                "return a + c;",
                "return d + b;",
                Some("return d + c;"),
            ),
            (
                "foo(1, 2, 3)",
                "foo(1, 2, 3)",
                "bar(1, 2, 3)",
                Some("bar(1, 2, 3)"),
            ),
            ("let x = 1", "let x = 2", "let x = 3", None),
            ("import a", "import b", "import c", None),
            ("name: alice", "name: bob", "name: alice", Some("name: bob")),
            ("a.b.c()", "a.b.d()", "a.b.c()", Some("a.b.d()")),
            ("[1, 2, 3]", "[1, 2, 3]", "[1, 2, 3]", Some("[1, 2, 3]")),
            (
                "key = \"v1\"",
                "key = \"v2\"",
                "key2 = \"v1\"",
                Some("key2 = \"v2\""),
            ),
            ("f(x)", "g(x)", "f(y)", Some("g(y)")),
        ];
        for (base, ours, theirs, expected) in cases {
            let result = resolve_simple(base, ours, theirs);
            match expected {
                Some(text) => assert_eq!(
                    result,
                    SimpleResolution::Resolved(text.to_string()),
                    "base={base:?} ours={ours:?} theirs={theirs:?}"
                ),
                None => assert_eq!(
                    result,
                    SimpleResolution::Unresolvable,
                    "base={base:?} ours={ours:?} theirs={theirs:?}"
                ),
            }
        }
    }
}
