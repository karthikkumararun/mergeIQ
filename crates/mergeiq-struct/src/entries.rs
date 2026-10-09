//! Container / entry extraction from a parse tree, driven by `queries/<lang>/entries.scm`.
//!
//! # Query capture convention
//!
//! * `@container` – a node whose named children are entries. Its *interior* is the text between
//!   its opening and closing delimiter (`{}`, `[]`, `()`); for undelimited containers (a module,
//!   a Python class body, a nested YAML mapping) it is the node's whole lines.
//! * `@entry` – an entry node. It must be a direct child of a `@container` node.
//! * `@entry.key` – a node whose text contributes to the entry's key. Several captures (also
//!   from several matches of the same entry node) are joined in source order.
//! * `@import` – on the same node as `@entry`: the entry is an import (set-union semantics).
//! * `@sep` – an anonymous separator token (`,` or `;`) that is a direct child of a container.
//! * `(#set! entry.kind "label")` – overrides the node kind used as the key prefix, so that
//!   e.g. `export function f` and `function f` share an identity.
//!
//! Named children of a container that no pattern claims become *opaque* entries keyed by their
//! kind and first line, so that nothing in a container is ever silently dropped.

use crate::lang::Lang;
use crate::parse::Parsed;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::OnceLock;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

/// Errors from loading queries.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// A bundled query failed to compile (a bug in `entries.scm`).
    #[error("invalid entries query for {lang}: {message}")]
    Invalid {
        /// The language name.
        lang: &'static str,
        /// The tree-sitter error message.
        message: String,
    },
}

static QUERIES: [OnceLock<Result<Query, String>>; 9] = [
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
];

fn lang_slot(lang: Lang) -> usize {
    Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0)
}

/// Returns the compiled (and cached) entries query for `lang`.
pub fn entries_query(lang: Lang) -> Result<&'static Query, QueryError> {
    QUERIES[lang_slot(lang)]
        .get_or_init(|| {
            Query::new(&lang.grammar(), lang.entries_query_source()).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|message| QueryError::Invalid {
            lang: lang.name(),
            message: message.clone(),
        })
}

/// One entry of a container.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The identity of the entry within its container.
    pub key: String,
    /// The entry is an import (merged with set-union semantics).
    pub is_import: bool,
    /// No query claimed the entry; its key is positional/textual.
    pub opaque: bool,
    /// The whitespace between the previous entry (and separator) and `core`.
    pub lead: Range<usize>,
    /// The entry text: leading comments, the node itself and any attached terminator or
    /// same-line trailing comment.
    pub core: Range<usize>,
    /// A separator token follows this entry directly.
    pub sep_after: bool,
    /// Index into [`Doc::containers`] of the single container nested inside this entry.
    pub child: Option<usize>,
}

/// A node whose children are keyed entries.
#[derive(Debug, Clone)]
pub struct Container {
    /// Byte range of the container node.
    pub node: Range<usize>,
    /// Byte range of the text between the delimiters (or the whole lines of the node).
    pub interior: Range<usize>,
    /// The interior starts at the beginning of a line in the source.
    pub interior_at_line_start: bool,
    /// The container is delimited by brackets.
    pub delimited: bool,
    /// The entries in source order.
    pub entries: Vec<Entry>,
    /// The separator token between entries, if the container uses one.
    pub sep: Option<String>,
    /// The last entry is followed by a separator.
    pub trailing_sep: bool,
    /// Text after the last entry (and its trailing separator) up to the end of the interior.
    pub tail: Range<usize>,
    /// `(container, entry)` this container is nested in.
    pub parent: Option<(usize, usize)>,
    /// Why this container cannot be merged, if so.
    pub bail: Option<String>,
}

/// All containers of one file version.
#[derive(Debug, Clone)]
pub struct Doc<'a> {
    /// The source text.
    pub text: &'a str,
    /// All containers, outermost first.
    pub containers: Vec<Container>,
    /// Indices of containers that are not nested inside an entry, in document order.
    pub roots: Vec<usize>,
}

impl Doc<'_> {
    /// The text of `range`.
    pub fn slice(&self, range: &Range<usize>) -> &str {
        &self.text[range.clone()]
    }
}

struct EntryAcc<'t> {
    node: Node<'t>,
    parts: Vec<(usize, String)>,
    import: bool,
    kind: Option<String>,
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_comment(kind: &str) -> bool {
    kind.contains("comment")
}

fn opener_closer(kind: &str) -> Option<&'static str> {
    match kind {
        "{" => Some("}"),
        "[" => Some("]"),
        "(" => Some(")"),
        _ => None,
    }
}

/// Extracts the containers and entries of `parsed` (whose source is `text`).
pub fn extract<'a>(parsed: &Parsed, text: &'a str) -> Result<Doc<'a>, QueryError> {
    extract_with(parsed, text, entries_query(parsed.lang)?)
}

/// Like [`extract`], with an explicit query (used to test query-driven behaviour).
pub fn extract_with<'a>(
    parsed: &Parsed,
    text: &'a str,
    query: &Query,
) -> Result<Doc<'a>, QueryError> {
    let names = query.capture_names();
    let cap = |wanted: &str| names.iter().position(|n| *n == wanted);
    let (c_container, c_entry, c_key, c_import, c_sep) = (
        cap("container"),
        cap("entry"),
        cap("entry.key"),
        cap("import"),
        cap("sep"),
    );

    let root = parsed.tree.root_node();
    let mut container_nodes: Vec<Node> = Vec::new();
    let mut entry_accs: HashMap<usize, EntryAcc> = HashMap::new();
    let mut sep_nodes: Vec<Node> = Vec::new();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, root, text.as_bytes());
    while let Some(m) = matches.next() {
        let mut entry_node: Option<Node> = None;
        let mut keys: Vec<Node> = Vec::new();
        let mut import = false;
        for c in m.captures() {
            let idx = Some(c.index as usize);
            if idx == c_container {
                if !container_nodes.iter().any(|n| n.id() == c.node.id()) {
                    container_nodes.push(c.node);
                }
            } else if idx == c_entry {
                entry_node = Some(c.node);
            } else if idx == c_key {
                keys.push(c.node);
            } else if idx == c_import {
                import = true;
            } else if idx == c_sep && !sep_nodes.iter().any(|n| n.id() == c.node.id()) {
                sep_nodes.push(c.node);
            }
        }
        if let Some(node) = entry_node {
            let kind = query
                .property_settings(m.pattern_index)
                .iter()
                .find(|p| &*p.key == "entry.kind")
                .and_then(|p| p.value.as_ref().map(|v| v.to_string()));
            let acc = entry_accs.entry(node.id()).or_insert_with(|| EntryAcc {
                node,
                parts: Vec::new(),
                import: false,
                kind: None,
            });
            acc.import |= import;
            if kind.is_some() {
                acc.kind = kind;
            }
            for k in keys {
                if !acc.parts.iter().any(|(s, _)| *s == k.start_byte()) {
                    acc.parts
                        .push((k.start_byte(), collapse_ws(&text[k.byte_range()])));
                }
            }
        }
    }

    // Outermost first.
    container_nodes.sort_by(|a, b| {
        a.start_byte()
            .cmp(&b.start_byte())
            .then(b.end_byte().cmp(&a.end_byte()))
    });

    let mut containers: Vec<Container> = Vec::with_capacity(container_nodes.len());
    // entry node id -> (container idx, entry idx)
    let mut entry_index: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut subtree_has_anchor: HashMap<usize, bool> = HashMap::new();

    for (ci, node) in container_nodes.iter().enumerate() {
        let is_root_node = node.parent().is_none();
        let container = build_container(
            *node,
            is_root_node,
            text,
            &entry_accs,
            &sep_nodes,
            parsed.lang,
            &mut subtree_has_anchor,
        );
        for (ei, node_id) in container.1.iter().enumerate() {
            entry_index.insert(*node_id, (ci, ei));
        }
        containers.push(container.0);
    }

    // Parent links: nearest ancestor that is a registered entry.
    let mut children_of: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (ci, node) in container_nodes.iter().enumerate() {
        // A container may itself be an entry of an outer container (e.g. a Go `const (...)` group).
        let mut cur = Some(*node);
        while let Some(p) = cur {
            if let Some(&(pc, pe)) = entry_index.get(&p.id()) {
                if pc != ci {
                    containers[ci].parent = Some((pc, pe));
                    children_of.entry((pc, pe)).or_default().push(ci);
                }
                break;
            }
            cur = p.parent();
        }
    }
    for ((pc, pe), kids) in children_of {
        if kids.len() == 1 {
            containers[pc].entries[pe].child = Some(kids[0]);
        }
    }

    // A nested container must lie inside its parent entry. Undelimited interiors are extended
    // to whole lines, which can run past the entry (its trailing newline): clamp them.
    for ci in 0..containers.len() {
        let Some((pc, pe)) = containers[ci].parent else {
            continue;
        };
        if pc == usize::MAX {
            continue;
        }
        let core = containers[pc].entries[pe].core.clone();
        let c = &mut containers[ci];
        if c.node == core && !c.delimited {
            // The container is the entry node itself (e.g. a Go `const (...)` group).
            c.interior.start = c.interior.start.max(core.start);
        }
        if c.interior.end > core.end {
            c.interior.end = core.end;
        }
        if c.interior.start < core.start || c.interior.start > c.interior.end {
            c.bail
                .get_or_insert_with(|| "container outside its entry".into());
            c.interior.start = c.interior.start.min(c.interior.end);
        }
        c.tail.end = c.interior.end;
        if c.tail.start > c.tail.end {
            c.tail.start = c.tail.end;
        }
    }

    // A container nested in another container but not in one of its entries (because the outer
    // container could not be analysed) is not a root: it must not be merged on its own.
    let container_ids: HashSet<usize> = container_nodes.iter().map(|n| n.id()).collect();
    for (ci, node) in container_nodes.iter().enumerate() {
        if containers[ci].parent.is_some() {
            continue;
        }
        let mut cur = node.parent();
        while let Some(p) = cur {
            if container_ids.contains(&p.id()) {
                containers[ci]
                    .bail
                    .get_or_insert_with(|| "orphaned container".into());
                containers[ci].parent = Some((usize::MAX, usize::MAX));
                break;
            }
            cur = p.parent();
        }
    }

    let roots = containers
        .iter()
        .enumerate()
        .filter(|(_, c)| c.parent.is_none())
        .map(|(i, _)| i)
        .collect();
    Ok(Doc {
        text,
        containers,
        roots,
    })
}

fn has_anchor_or_alias(node: Node<'_>, cache: &mut HashMap<usize, bool>) -> bool {
    if let Some(v) = cache.get(&node.id()) {
        return *v;
    }
    let mut found = matches!(node.kind(), "anchor" | "alias" | "tag");
    if !found {
        let mut cur = node.walk();
        for ch in node.children(&mut cur) {
            if has_anchor_or_alias(ch, cache) {
                found = true;
                break;
            }
        }
    }
    cache.insert(node.id(), found);
    found
}

type BuiltContainer = (Container, Vec<usize>);

fn build_container(
    node: Node<'_>,
    is_root_node: bool,
    text: &str,
    accs: &HashMap<usize, EntryAcc>,
    sep_nodes: &[Node],
    lang: Lang,
    anchor_cache: &mut HashMap<usize, bool>,
) -> BuiltContainer {
    let mut bail: Option<String> = None;
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();

    // Delimiters.
    let first_entryish = children
        .iter()
        .position(|c| c.is_named() && !is_comment(c.kind()));
    let opener_idx = children
        .iter()
        .take(first_entryish.unwrap_or(children.len()))
        .position(|c| !c.is_named() && opener_closer(c.kind()).is_some());
    let closer_idx = match (opener_idx, children.last()) {
        (Some(o), Some(last))
            if !last.is_named()
                && children.len() - 1 > o
                && opener_closer(children[o].kind()) == Some(last.kind()) =>
        {
            Some(children.len() - 1)
        }
        _ => None,
    };
    let delimited = opener_idx.is_some() && closer_idx.is_some();
    let (inner_from, inner_to) = if delimited {
        (
            opener_idx.map_or(0, |i| i + 1),
            closer_idx.unwrap_or(children.len()),
        )
    } else {
        (0, children.len())
    };

    let interior: Range<usize> = if delimited {
        let o = &children[opener_idx.unwrap_or(0)];
        let c = &children[closer_idx.unwrap_or(0)];
        o.end_byte()..c.start_byte()
    } else if is_root_node {
        0..text.len()
    } else {
        let start = text[..node.start_byte()]
            .rfind(['\n', '\r'])
            .map_or(0, |i| i + 1);
        let end = text[node.end_byte()..]
            .find(['\n', '\r'])
            .map_or(text.len(), |i| {
                let mut e = node.end_byte() + i;
                let bytes = text.as_bytes();
                if bytes[e] == b'\r' && bytes.get(e + 1) == Some(&b'\n') {
                    e += 2;
                } else {
                    e += 1;
                }
                e
            });
        start..end
    };
    let interior_at_line_start =
        interior.start == 0 || matches!(text.as_bytes()[interior.start - 1], b'\n' | b'\r');

    if lang == Lang::Yaml && has_anchor_or_alias(node, anchor_cache) {
        bail = Some("YAML anchors, aliases or tags".into());
    }

    // Separator kind(s) used directly under this container.
    let mut sep_kinds: Vec<&str> = sep_nodes
        .iter()
        .filter(|s| s.parent().is_some_and(|p| p.id() == node.id()))
        .map(|s| s.kind())
        .collect();
    sep_kinds.sort_unstable();
    sep_kinds.dedup();
    if sep_kinds.len() > 1 {
        bail = Some("mixed separators".into());
    }
    let sep: Option<String> = sep_kinds.first().map(|s| s.to_string());

    let mut entries: Vec<Entry> = Vec::new();
    let mut entry_node_ids: Vec<usize> = Vec::new();
    let mut prev_end = interior.start;
    let mut pending: Option<usize> = None;
    let mut opaque_seen: HashMap<String, usize> = HashMap::new();
    let mut import_seen: HashMap<String, usize> = HashMap::new();

    for child in &children[inner_from..inner_to] {
        if bail.is_some() {
            break;
        }
        let kind = child.kind();
        if !child.is_named() {
            let last_adjacent = entries.last().is_some()
                && pending.is_none()
                && text[prev_end..child.start_byte()].trim().is_empty();
            if sep.as_deref() == Some(kind) {
                match entries.last_mut() {
                    Some(last) if pending.is_none() && last.core.end == child.start_byte() => {
                        last.sep_after = true;
                        prev_end = child.end_byte();
                    }
                    _ => bail = Some("separator in unexpected position".into()),
                }
            } else if (kind == ";" || kind == ",") && last_adjacent {
                if let Some(last) = entries.last_mut() {
                    if text[last.core.end..child.start_byte()].is_empty() {
                        last.core.end = child.end_byte();
                        prev_end = child.end_byte();
                    } else {
                        bail = Some("detached terminator".into());
                    }
                }
            } else {
                bail = Some(format!("unexpected token `{kind}`"));
            }
            continue;
        }
        if is_comment(kind) {
            let same_row_as_prev = entries.last().is_some_and(|last| {
                pending.is_none()
                    && !text[last.core.end..child.start_byte()].contains(['\n', '\r'])
                    && prev_end <= child.start_byte()
            });
            if same_row_as_prev {
                if sep.is_some() {
                    bail = Some("trailing comment in separated container".into());
                } else if let Some(last) = entries.last_mut() {
                    last.core.end = child.end_byte();
                    prev_end = child.end_byte();
                }
            } else if pending.is_none() {
                pending = Some(child.start_byte());
            }
            continue;
        }
        // An entry.
        let core_start = pending.unwrap_or_else(|| child.start_byte());
        let lead = prev_end..core_start;
        if !text[lead.clone()].trim().is_empty() {
            bail = Some("unexpected text between entries".into());
            continue;
        }
        let acc = accs.get(&child.id()).filter(|a| a.node.parent().is_some());
        let first_line = || {
            let t = &text[child.byte_range()];
            let line = t.lines().next().unwrap_or("");
            let mut s = collapse_ws(line);
            s.truncate(s.char_indices().nth(80).map_or(s.len(), |(i, _)| i));
            s
        };
        let (mut key, is_import, opaque) = match acc {
            Some(a) => {
                let mut parts = a.parts.clone();
                parts.sort_by_key(|(s, _)| *s);
                let kind_label = a.kind.clone().unwrap_or_else(|| kind.to_string());
                let joined = parts
                    .into_iter()
                    .map(|(_, t)| t)
                    .collect::<Vec<_>>()
                    .join("\u{1f}");
                (format!("{kind_label}:{joined}"), a.import, false)
            }
            None => (format!("~{kind}:{}", first_line()), false, true),
        };
        if opaque {
            let n = opaque_seen.entry(key.clone()).or_insert(0);
            key = format!("{key}#{n}");
            *n += 1;
        } else if is_import {
            let n = import_seen.entry(key.clone()).or_insert(0);
            if *n > 0 {
                key = format!("{key}#{n}");
            }
            *n += 1;
        }
        // Some grammars (YAML) let the last node of a document swallow its newline.
        let core_end = text[..child.end_byte()]
            .trim_end()
            .len()
            .max(child.start_byte());
        entries.push(Entry {
            key,
            is_import,
            opaque,
            lead,
            core: core_start..core_end,
            sep_after: false,
            child: None,
        });
        entry_node_ids.push(child.id());
        pending = None;
        prev_end = core_end;
    }

    // Separator regularity: every entry but the last must be followed by one.
    let trailing_sep = entries.last().is_some_and(|e| e.sep_after);
    if bail.is_none() && sep.is_some() && entries.len() > 1 {
        let n = entries.len();
        if entries[..n - 1].iter().any(|e| !e.sep_after) {
            bail = Some("irregular separators".into());
        }
    }
    if bail.is_none() && sep.is_none() && entries.iter().any(|e| e.sep_after) {
        bail = Some("unexpected separator".into());
    }
    if bail.is_none() {
        let mut seen: HashSet<&str> = HashSet::new();
        for e in &entries {
            if !seen.insert(&e.key) {
                bail = Some(format!("duplicate key `{}`", e.key));
                break;
            }
        }
    }

    let tail = prev_end.min(interior.end)..interior.end;
    (
        Container {
            node: node.byte_range(),
            interior,
            interior_at_line_start,
            delimited,
            entries,
            sep,
            trailing_sep,
            tail,
            parent: None,
            bail,
        },
        entry_node_ids,
    )
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn a_new_key_rule_takes_effect_without_code_changes() {
        let text = "{\"a\": 1, \"b\": 2}";
        let parsed = crate::parse::parse(Lang::Json, text).unwrap();
        let default = extract(&parsed, text).unwrap();
        assert_eq!(default.containers[0].entries[0].key, "pair:\"a\"");

        // Same grammar, different rule: key pairs by their *value*.
        let custom = Query::new(
            &Lang::Json.grammar(),
            "(object) @container (object \",\" @sep) (pair value: (_) @entry.key) @entry",
        )
        .unwrap();
        let doc = extract_with(&parsed, text, &custom).unwrap();
        assert_eq!(doc.containers[0].entries[0].key, "pair:1");
        assert_eq!(doc.containers[0].entries[1].key, "pair:2");
    }

    #[test]
    fn every_bundled_query_compiles() {
        for lang in Lang::ALL {
            if let Err(e) = entries_query(lang) {
                panic!("{e}");
            }
        }
    }
}
