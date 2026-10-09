//! Parsing helpers: one tree per file version and syntax-error counting.

use crate::lang::Lang;
use tree_sitter::{Node, Parser, Tree};

/// A parsed file version.
pub struct Parsed {
    /// The language it was parsed as.
    pub lang: Lang,
    /// The tree-sitter tree.
    pub tree: Tree,
    /// Number of ERROR and MISSING nodes in `tree`.
    pub errors: usize,
}

/// Parses `text` as `lang`. Returns `None` only if the parser cannot produce a tree at all.
pub fn parse(lang: Lang, text: &str) -> Option<Parsed> {
    let mut parser = Parser::new();
    parser.set_language(&lang.grammar()).ok()?;
    let tree = parser.parse(text, None)?;
    let errors = count_errors(tree.root_node());
    Some(Parsed { lang, tree, errors })
}

/// Counts ERROR and MISSING nodes under `root` (inclusive).
pub fn count_errors(root: Node<'_>) -> usize {
    if !root.has_error() {
        return 0;
    }
    let mut count = 0;
    let mut cursor = root.walk();
    // Iterative pre-order walk that skips subtrees without errors.
    'outer: loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            count += 1;
        }
        if node.has_error() && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                continue 'outer;
            }
            if !cursor.goto_parent() {
                break 'outer;
            }
            if cursor.node() == root {
                break 'outer;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_source_has_no_errors() {
        let p = parse(Lang::Json, "{\"a\": 1, \"b\": [1, 2]}").unwrap();
        assert_eq!(p.errors, 0);
    }

    #[test]
    fn missing_brace_is_counted() {
        let p = parse(Lang::Java, "class A { void f() { }\n").unwrap();
        assert!(p.errors >= 1, "errors = {}", p.errors);
    }

    #[test]
    fn garbage_is_counted_and_more_garbage_is_not_fewer() {
        let one = parse(Lang::Json, "{\"a\": }").unwrap().errors;
        let two = parse(Lang::Json, "{\"a\": } }").unwrap().errors;
        assert!(one >= 1);
        assert!(two >= one);
    }

    #[test]
    fn every_language_parses_trivial_valid_source() {
        let cases: [(Lang, &str); 9] = [
            (Lang::Java, "class A {}\n"),
            (Lang::Kotlin, "class A\n"),
            (Lang::Python, "x = 1\n"),
            (Lang::Yaml, "a: 1\n"),
            (Lang::Json, "{}\n"),
            (Lang::JavaScript, "let a = 1;\n"),
            (Lang::TypeScript, "let a: number = 1;\n"),
            (Lang::Tsx, "const a = <div/>;\n"),
            (Lang::Go, "package main\n"),
        ];
        for (lang, src) in cases {
            let p = parse(lang, src).unwrap();
            assert_eq!(p.errors, 0, "{}", lang.name());
        }
    }
}
