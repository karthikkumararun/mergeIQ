//! Import-specific merging: name lists of the same module imported by both sides.
//!
//! Plain import entries (different modules) are merged by [`crate::keyed3`] as a set union with
//! sorted insertion. This module handles the remaining case: both sides edited the *same*
//! statement, e.g. `from os import path` became `from os import path, sep` on one side and
//! `from os import getcwd, path` on the other.

use crate::lang::Lang;

/// Merges the imported-name lists of one import statement that both sides changed.
///
/// Returns `None` unless the statements are simple single-line imports of the same module
/// whose heads (everything but the name list) agree. The union of names is used: a name is
/// dropped only if *both* sides dropped it. If all three name lists are sorted, the result is
/// sorted; otherwise base order is kept, followed by ours' and then theirs' additions.
pub fn merge_names(lang: Lang, base: &str, ours: &str, theirs: &str) -> Option<String> {
    match lang {
        Lang::Python => merge_python(base, ours, theirs),
        Lang::JavaScript | Lang::TypeScript | Lang::Tsx => merge_js(base, ours, theirs),
        _ => None,
    }
}

struct Parts<'a> {
    head: &'a str,
    names: Vec<&'a str>,
    tail: &'a str,
    padded: bool,
}

fn split_names(inner: &str) -> Option<Vec<&str>> {
    let mut names = Vec::new();
    for raw in inner.split(',') {
        let n = raw.trim();
        if n.is_empty() {
            // Allow one trailing comma, nothing else.
            continue;
        }
        if n.contains(['*', '(', ')', '{', '}']) {
            return None;
        }
        names.push(n);
    }
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

fn parse_python(s: &str) -> Option<Parts<'_>> {
    let s = s.trim_end();
    if s.contains(['\n', '\r', '#', '(', ')', '\\', ';']) {
        return None;
    }
    let pos = s.find(" import ")?;
    if !s.starts_with("from ") {
        return None;
    }
    Some(Parts {
        head: &s[..pos],
        names: split_names(&s[pos + " import ".len()..])?,
        tail: "",
        padded: false,
    })
}

fn parse_js(s: &str) -> Option<Parts<'_>> {
    let s = s.trim_end();
    if s.contains(['\n', '\r']) || s.contains("//") || s.contains("/*") {
        return None;
    }
    let open = s.find('{')?;
    let close = s.rfind('}')?;
    if close < open || !s.starts_with("import") {
        return None;
    }
    let inner = &s[open + 1..close];
    Some(Parts {
        head: &s[..open],
        names: split_names(inner)?,
        tail: &s[close + 1..],
        padded: inner.starts_with(' '),
    })
}

fn union(base: &[&str], ours: &[&str], theirs: &[&str]) -> Vec<String> {
    let removed_by_both = |n: &str| !ours.contains(&n) && !theirs.contains(&n);
    let sorted = |v: &[&str]| v.windows(2).all(|w| w[0] <= w[1]);
    let mut out: Vec<String> = Vec::new();
    if sorted(base) && sorted(ours) && sorted(theirs) {
        for n in base.iter().chain(ours).chain(theirs) {
            if base.contains(n) && removed_by_both(n) {
                continue;
            }
            if !out.iter().any(|o| o == n) {
                out.push((*n).to_string());
            }
        }
        out.sort();
    } else {
        for n in base {
            if !removed_by_both(n) && !out.iter().any(|o| o == n) {
                out.push((*n).to_string());
            }
        }
        for n in ours.iter().chain(theirs) {
            if !base.contains(n) && !out.iter().any(|o| o == n) {
                out.push((*n).to_string());
            }
        }
    }
    out
}

fn merge_python(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let (b, o, t) = (
        parse_python(base)?,
        parse_python(ours)?,
        parse_python(theirs)?,
    );
    if b.head != o.head || b.head != t.head {
        return None;
    }
    let names = union(&b.names, &o.names, &t.names);
    if names.is_empty() {
        return None;
    }
    Some(format!("{} import {}", o.head, names.join(", ")))
}

fn merge_js(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let (b, o, t) = (parse_js(base)?, parse_js(ours)?, parse_js(theirs)?);
    if b.head != o.head || b.head != t.head || b.tail != o.tail || b.tail != t.tail {
        return None;
    }
    let names = union(&b.names, &o.names, &t.names);
    if names.is_empty() {
        return None;
    }
    let (open, close) = if o.padded { ("{ ", " }") } else { ("{", "}") };
    Some(format!(
        "{}{open}{}{close}{}",
        o.head,
        names.join(", "),
        o.tail
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_names_sorted_union() {
        assert_eq!(
            merge_names(
                Lang::Python,
                "from os import path",
                "from os import path, sep",
                "from os import getcwd, path"
            )
            .as_deref(),
            Some("from os import getcwd, path, sep")
        );
    }

    #[test]
    fn python_unsorted_keeps_base_then_ours_then_theirs() {
        assert_eq!(
            merge_names(
                Lang::Python,
                "from m import z, a",
                "from m import z, a, o",
                "from m import z, a, t"
            )
            .as_deref(),
            Some("from m import z, a, o, t")
        );
    }

    #[test]
    fn python_removal_needs_both_sides() {
        // Ours removed `b`, theirs added `c` (so theirs also edited the statement): keep `b`.
        assert_eq!(
            merge_names(
                Lang::Python,
                "from m import a, b",
                "from m import a",
                "from m import a, b, c"
            )
            .as_deref(),
            Some("from m import a, b, c")
        );
        // Both removed `b`.
        assert_eq!(
            merge_names(
                Lang::Python,
                "from m import a, b",
                "from m import a, x",
                "from m import a, y"
            )
            .as_deref(),
            Some("from m import a, x, y")
        );
    }

    #[test]
    fn python_rejects_complex_forms() {
        for s in ["from m import (a, b)", "from m import *", "import os"] {
            assert_eq!(merge_names(Lang::Python, s, s, s), None, "{s}");
        }
        assert_eq!(
            merge_names(
                Lang::Python,
                "from a import x",
                "from a import x, y",
                "from b import x, z"
            ),
            None
        );
    }

    #[test]
    fn js_named_specifiers_merge_and_keep_style() {
        assert_eq!(
            merge_names(
                Lang::TypeScript,
                r#"import { a } from "m";"#,
                r#"import { a, c } from "m";"#,
                r#"import { a, b } from "m";"#
            )
            .as_deref(),
            Some(r#"import { a, b, c } from "m";"#)
        );
        assert_eq!(
            merge_names(
                Lang::JavaScript,
                r#"import d, {a} from 'm'"#,
                r#"import d, {a, x} from 'm'"#,
                r#"import d, {a, y} from 'm'"#
            )
            .as_deref(),
            Some("import d, {a, x, y} from 'm'")
        );
    }

    #[test]
    fn js_rejects_mismatched_heads_and_multiline() {
        assert_eq!(
            merge_names(
                Lang::JavaScript,
                r#"import { a } from "m";"#,
                r#"import type { a, b } from "m";"#,
                r#"import { a, c } from "m";"#
            ),
            None
        );
        let multi = "import {\n  a,\n} from \"m\";";
        assert_eq!(merge_names(Lang::JavaScript, multi, multi, multi), None);
    }

    #[test]
    fn other_languages_are_not_handled() {
        assert_eq!(
            merge_names(Lang::Java, "import a.B;", "import a.C;", "import a.D;"),
            None
        );
    }
}
