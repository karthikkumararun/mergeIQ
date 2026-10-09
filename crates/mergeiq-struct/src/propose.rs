//! Turning a merged file into per-chunk proposals.
//!
//! The root containers of the three versions are merged (recursing into entries both sides
//! changed). The merged file is diffed against the base; each changed region ("hunk") is
//! expanded to whole-chunk boundaries and becomes a proposal if it resolves at least one
//! conflict chunk, touches no protected (unresolvable) range, and parses cleanly.

use crate::entries::{self, QueryError};
use crate::keyed3::{Change, Side};
use crate::lang::Lang;
use crate::parse::{self, Parsed};
use crate::render::{merge_container, Ctx, Ev};
use mergeiq_core::{
    diff_lines, line_keys, split_lines, Analysis, Chunk, ChunkKind, Line, LineHunk, LineRange,
    WhitespacePolicy,
};
use std::ops::Range;
use std::time::Instant;

/// A syntax-aware resolution proposal covering one or more chunks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Proposal {
    /// The chunks this proposal resolves (all must be unresolved to apply it).
    pub chunk_ids: Vec<u32>,
    /// Covered lines in the base text.
    pub base_range: LineRange,
    /// Covered lines in the ours text.
    pub ours_range: LineRange,
    /// Covered lines in the theirs text.
    pub theirs_range: LineRange,
    /// The replacement for the covered base lines, with line terminators.
    pub text: String,
    /// A one-paragraph explanation of what was combined.
    pub explanation: String,
    /// Human-readable name of the innermost merged container ("" = file level).
    pub container: String,
}

/// The result of asking for structural proposals.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Outcome {
    /// The file type is not supported (or its grammar cannot parse these versions reliably).
    Unsupported,
    /// Zero or more proposals.
    Proposals(Vec<Proposal>),
    /// The deadline passed before the analysis finished; no proposals are offered.
    TimedOut,
}

/// Errors from [`propose`]. These indicate bugs in the bundled queries, not bad input.
#[derive(Debug, thiserror::Error)]
pub enum ProposeError {
    /// A bundled entries query is invalid.
    #[error(transparent)]
    Query(#[from] QueryError),
}

fn line_of(lines: &[Line], byte: usize) -> usize {
    match lines.binary_search_by(|l| l.start.cmp(&byte)) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    }
}

fn line_start(lines: &[Line], idx: usize, text_len: usize) -> usize {
    lines.get(idx).map_or(text_len, |l| l.start)
}

/// Do two half-open line ranges intersect? Empty ranges are insertion points and intersect a
/// range that contains them, boundaries included; non-empty ranges must overlap properly.
fn intersects(a: &Range<u32>, b: &Range<u32>) -> bool {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => a.start == b.start,
        (true, false) => b.start <= a.start && a.start <= b.end,
        (false, true) => a.start <= b.start && b.start <= a.end,
        (false, false) => a.start < b.end && b.start < a.end,
    }
}

fn lr(r: &Range<u32>) -> LineRange {
    LineRange {
        start: r.start,
        end: r.end,
    }
}

struct Window {
    /// First and last hunk included (inclusive).
    i: usize,
    j: usize,
    /// Base line range.
    s: u32,
    e: u32,
}

fn close_window(w: &mut Window, hunks: &[LineHunk], chunks: &[Chunk]) {
    loop {
        let before = (w.i, w.j, w.s, w.e);
        let range = w.s..w.e;
        for c in chunks {
            let cr = c.base.start..c.base.end;
            if intersects(&range, &cr) {
                w.s = w.s.min(c.base.start);
                w.e = w.e.max(c.base.end);
            }
        }
        while w.j + 1 < hunks.len() && hunks[w.j + 1].before.start < w.e {
            w.j += 1;
            w.e = w.e.max(hunks[w.j].before.end);
        }
        while w.i > 0 && hunks[w.i - 1].before.end > w.s {
            w.i -= 1;
            w.s = w.s.min(hunks[w.i].before.start);
        }
        if before == (w.i, w.j, w.s, w.e) {
            break;
        }
    }
}

fn describe(ev: &Ev) -> String {
    let n = &ev.name;
    match ev.change {
        Change::Added(Side::Ours) => format!("left added `{n}`"),
        Change::Added(Side::Theirs) => format!("right added `{n}`"),
        Change::Changed(Side::Ours) => format!("left changed `{n}`"),
        Change::Changed(Side::Theirs) => format!("right changed `{n}`"),
        Change::Removed(Side::Ours) => format!("left removed `{n}`"),
        Change::Removed(Side::Theirs) => format!("right removed `{n}`"),
        Change::BothSame => format!("both made the same change to `{n}`"),
        Change::Combined => format!("both changed `{n}` and the changes were combined"),
    }
}

fn explain(events: &[&Ev]) -> (String, String) {
    let mut order: Vec<&str> = Vec::new();
    for ev in events {
        if !order.contains(&ev.container.as_str()) {
            order.push(&ev.container);
        }
    }
    let mut sentences: Vec<String> = Vec::new();
    for label in &order {
        let mut items: Vec<String> = events
            .iter()
            .filter(|e| e.container == *label)
            .map(|e| describe(e))
            .collect();
        let extra = items.len().saturating_sub(6);
        items.truncate(6);
        let mut list = match items.len() {
            0 => String::new(),
            1 => items[0].clone(),
            n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
        };
        if extra > 0 {
            list.push_str(&format!(" (and {extra} more)"));
        }
        let mut chars = list.chars();
        let list = match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
            None => list,
        };
        let sentence = if order.len() == 1 {
            format!("{list}.")
        } else if label.is_empty() {
            format!("At the top level of the file: {}.", lower_first(&list))
        } else {
            format!("In `{label}`: {}.", lower_first(&list))
        };
        sentences.push(sentence);
    }
    let container = order
        .iter()
        .rev()
        .find(|l| !l.is_empty())
        .map(|l| (*l).to_string())
        .unwrap_or_default();
    (sentences.join(" "), container)
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A candidate result is valid if it has no more syntax errors than the worst input.
fn is_valid(lang: Lang, candidate: &str, max_errors: usize) -> bool {
    matches!(parse::parse(lang, candidate), Some(p) if p.errors <= max_errors)
}

fn parse_all(lang: Lang, a: &Analysis) -> Option<(Parsed, Parsed, Parsed)> {
    Some((
        parse::parse(lang, &a.base.text)?,
        parse::parse(lang, &a.ours.text)?,
        parse::parse(lang, &a.theirs.text)?,
    ))
}

/// Computes structural proposals for the conflict chunks of `analysis`.
pub fn propose(path: &str, analysis: &Analysis) -> Result<Outcome, ProposeError> {
    propose_until(path, analysis, None)
}

/// Like [`propose`], giving up with [`Outcome::TimedOut`] once `deadline` has passed. The
/// deadline is checked between phases, so a single huge parse can still overrun it slightly.
pub fn propose_until(
    path: &str,
    analysis: &Analysis,
    deadline: Option<Instant>,
) -> Result<Outcome, ProposeError> {
    let expired = || deadline.is_some_and(|d| Instant::now() >= d);
    let Some(lang) = Lang::from_path(path) else {
        return Ok(Outcome::Unsupported);
    };
    let Some((pb, po, pt)) = parse_all(lang, analysis) else {
        return Ok(Outcome::Unsupported);
    };
    // The Kotlin grammar is not mature enough to trust on files it reports errors for.
    if lang == Lang::Kotlin && (pb.errors > 0 || po.errors > 0 || pt.errors > 0) {
        return Ok(Outcome::Unsupported);
    }
    if !analysis
        .chunks
        .iter()
        .any(|c| c.kind == ChunkKind::Conflict)
    {
        return Ok(Outcome::Proposals(Vec::new()));
    }
    if expired() {
        return Ok(Outcome::TimedOut);
    }

    let (bt, ot, tt) = (
        analysis.base.text.as_str(),
        analysis.ours.text.as_str(),
        analysis.theirs.text.as_str(),
    );
    let (db, d_ours, dt) = (
        entries::extract(&pb, bt)?,
        entries::extract(&po, ot)?,
        entries::extract(&pt, tt)?,
    );
    if db.roots.is_empty()
        || db.roots.len() != d_ours.roots.len()
        || db.roots.len() != dt.roots.len()
    {
        return Ok(Outcome::Proposals(Vec::new()));
    }
    if expired() {
        return Ok(Outcome::TimedOut);
    }
    let ctx = Ctx {
        lang,
        base: &db,
        ours: &d_ours,
        theirs: &dt,
    };

    // Merge every root and splice the results into a copy of the base text.
    let mut merged = String::with_capacity(bt.len());
    let mut cursor = 0usize;
    let mut protected_bytes: Vec<Range<usize>> = Vec::new();
    let mut events: Vec<Ev> = Vec::new();
    for ((rb, ro), rt) in db.roots.iter().zip(&d_ours.roots).zip(&dt.roots) {
        let Ok(r) = merge_container(&ctx, *rb, *ro, *rt) else {
            continue;
        };
        let interior = &db.containers[*rb].interior;
        if interior.start < cursor {
            continue;
        }
        merged.push_str(&bt[cursor..interior.start]);
        merged.push_str(&r.text);
        cursor = interior.end;
        protected_bytes.extend(r.protected);
        events.extend(r.events);
    }
    merged.push_str(&bt[cursor..]);
    if expired() {
        return Ok(Outcome::TimedOut);
    }
    if merged == bt {
        return Ok(Outcome::Proposals(Vec::new()));
    }

    let base_lines = &analysis.base.lines;
    let merged_lines = split_lines(&merged);
    let hunks = diff_lines(
        &line_keys(bt, base_lines, WhitespacePolicy::Exact),
        &line_keys(&merged, &merged_lines, WhitespacePolicy::Exact),
    );
    if hunks.is_empty() {
        return Ok(Outcome::Proposals(Vec::new()));
    }

    // Protected byte ranges -> base line ranges.
    let protected: Vec<Range<u32>> = protected_bytes
        .iter()
        .map(|r| {
            let s = line_of(base_lines, r.start) as u32;
            let e = if r.is_empty() {
                s + 1
            } else {
                line_of(base_lines, r.end - 1) as u32 + 1
            };
            s..e
        })
        .collect();

    // Grow every hunk to whole-chunk boundaries; overlapping windows merge.
    let mut windows: Vec<Window> = Vec::new();
    for (k, h) in hunks.iter().enumerate() {
        if windows.last().is_some_and(|w| w.j >= k) {
            continue;
        }
        let mut w = Window {
            i: k,
            j: k,
            s: h.before.start,
            e: h.before.end,
        };
        close_window(&mut w, &hunks, &analysis.chunks);
        match windows.last_mut() {
            Some(prev) if w.i <= prev.j || w.s < prev.e => {
                prev.i = prev.i.min(w.i);
                prev.j = prev.j.max(w.j);
                prev.s = prev.s.min(w.s);
                prev.e = prev.e.max(w.e);
                close_window(prev, &hunks, &analysis.chunks);
            }
            _ => windows.push(w),
        }
    }

    let max_errors = pb.errors.max(po.errors).max(pt.errors);
    let mut proposals: Vec<Proposal> = Vec::new();
    for w in &windows {
        if expired() {
            return Ok(Outcome::TimedOut);
        }
        let range = w.s..w.e;
        let covered: Vec<&Chunk> = analysis
            .chunks
            .iter()
            .filter(|c| intersects(&range, &(c.base.start..c.base.end)))
            .collect();
        if !covered.iter().any(|c| c.kind == ChunkKind::Conflict) {
            continue;
        }
        if covered.iter().any(|c| {
            let cr = c.base.start..c.base.end;
            protected.iter().any(|p| intersects(&cr, p))
        }) || protected.iter().any(|p| intersects(&range, p))
        {
            continue;
        }

        // The same window in merged coordinates: unchanged lines extend both sides equally.
        let ms = hunks[w.i].after.start - (hunks[w.i].before.start - w.s);
        let me = hunks[w.j].after.end + (w.e - hunks[w.j].before.end);
        let text = merged[line_start(&merged_lines, ms as usize, merged.len())
            ..line_start(&merged_lines, me as usize, merged.len())]
            .to_string();

        // Validate by reparsing the base with the proposal applied.
        let b0 = line_start(base_lines, w.s as usize, bt.len());
        let b1 = line_start(base_lines, w.e as usize, bt.len());
        let candidate = format!("{}{}{}", &bt[..b0], text, &bt[b1..]);
        if !is_valid(lang, &candidate, max_errors) {
            continue;
        }

        // Ours / theirs ranges: shift by the net size change of the chunks before the window.
        let shift = |side: fn(&Chunk) -> &LineRange, start: bool| -> u32 {
            let mut delta: i64 = 0;
            for c in &analysis.chunks {
                let inside = covered.iter().any(|x| x.id == c.id);
                let before_edge = if start {
                    c.base.end <= w.s
                } else {
                    c.base.end <= w.e
                };
                if before_edge && (!start || !inside) {
                    let r = side(c);
                    delta += i64::from(r.end - r.start) - i64::from(c.base.end - c.base.start);
                }
            }
            let pos = if start { w.s } else { w.e };
            (i64::from(pos) + delta).max(0) as u32
        };
        let ours_range = LineRange {
            start: shift(|c| &c.ours, true),
            end: shift(|c| &c.ours, false),
        };
        let theirs_range = LineRange {
            start: shift(|c| &c.theirs, true),
            end: shift(|c| &c.theirs, false),
        };

        // Events sit at entry boundaries, which may be separated from the window by whitespace.
        let lo = bt[..b0]
            .trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .len();
        let in_window: Vec<&Ev> = events
            .iter()
            .filter(|e| e.byte >= lo && e.byte <= b1)
            .collect();
        let (explanation, container) = explain(&in_window);
        proposals.push(Proposal {
            chunk_ids: covered.iter().map(|c| c.id).collect(),
            base_range: lr(&range),
            ours_range,
            theirs_range,
            text,
            explanation,
            container,
        });
    }
    Ok(Outcome::Proposals(proposals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mergeiq_core::{analyze, MergeInput, Options};

    fn run(path: &str, base: &str, ours: &str, theirs: &str) -> Outcome {
        let a = analyze(
            MergeInput {
                base: base.as_bytes(),
                ours: ours.as_bytes(),
                theirs: theirs.as_bytes(),
            },
            &Options::default(),
        )
        .unwrap();
        propose(path, &a).unwrap()
    }

    fn proposals(o: Outcome) -> Vec<Proposal> {
        match o {
            Outcome::Proposals(p) => p,
            other => panic!("expected proposals, got {other:?}"),
        }
    }

    #[test]
    fn an_expired_deadline_times_out() {
        let base = "{\n  \"a\": 1\n}\n";
        let ours = "{\n  \"a\": 1,\n  \"o\": 2\n}\n";
        let theirs = "{\n  \"a\": 1,\n  \"t\": 3\n}\n";
        let a = analyze(
            MergeInput {
                base: base.as_bytes(),
                ours: ours.as_bytes(),
                theirs: theirs.as_bytes(),
            },
            &Options::default(),
        )
        .unwrap();
        let past = Instant::now() - std::time::Duration::from_secs(1);
        assert_eq!(
            propose_until("a.json", &a, Some(past)).unwrap(),
            Outcome::TimedOut
        );
        assert!(matches!(
            propose_until("a.json", &a, None).unwrap(),
            Outcome::Proposals(p) if p.len() == 1
        ));
    }

    #[test]
    fn unsupported_extension() {
        assert_eq!(run("README.md", "a\n", "b\n", "c\n"), Outcome::Unsupported);
        assert_eq!(run("Makefile", "a\n", "b\n", "c\n"), Outcome::Unsupported);
    }

    #[test]
    fn kotlin_with_parse_errors_is_unsupported() {
        // Single-line object bodies trip the Kotlin grammar.
        let base = "object O { fun f() {} }\n";
        let ours = "object O { fun f() {} fun a() {} }\n";
        let theirs = "object O { fun f() {} fun b() {} }\n";
        assert_eq!(run("a.kt", base, ours, theirs), Outcome::Unsupported);
    }

    #[test]
    fn files_without_conflicts_have_no_proposals() {
        let o = run(
            "a.json",
            "{\n  \"a\": 1\n}\n",
            "{\n  \"a\": 2\n}\n",
            "{\n  \"a\": 1\n}\n",
        );
        assert_eq!(proposals(o), Vec::new());
    }

    #[test]
    fn crlf_files_keep_crlf_in_proposals() {
        let base = "{\r\n  \"a\": 1,\r\n  \"s\": {\r\n    \"x\": 1\r\n  }\r\n}\r\n";
        let ours = base.replace("\"x\": 1", "\"x\": 1,\r\n    \"o\": 2");
        let theirs = base.replace("\"x\": 1", "\"x\": 1,\r\n    \"t\": 3");
        let ps = proposals(run("a.json", base, &ours, &theirs));
        assert_eq!(ps.len(), 1);
        let text = &ps[0].text;
        assert!(
            text.contains("\"o\": 2") && text.contains("\"t\": 3"),
            "{text:?}"
        );
        assert!(
            !text.replace("\r\n", "").contains('\n'),
            "bare LF in {text:?}"
        );
    }

    #[test]
    fn proposal_reports_ranges_chunks_and_explanation() {
        let base = "{\n  \"s\": {\n    \"a\": 1\n  }\n}\n";
        let ours = "{\n  \"s\": {\n    \"a\": 1,\n    \"o\": 2\n  }\n}\n";
        let theirs = "{\n  \"s\": {\n    \"a\": 1,\n    \"t\": 3\n  }\n}\n";
        let ps = proposals(run("x.json", base, ours, theirs));
        assert_eq!(ps.len(), 1);
        let p = &ps[0];
        assert_eq!(p.chunk_ids, vec![0]);
        assert_eq!((p.base_range.start, p.base_range.end), (2, 3));
        assert_eq!((p.ours_range.start, p.ours_range.end), (2, 4));
        assert_eq!((p.theirs_range.start, p.theirs_range.end), (2, 4));
        assert_eq!(p.container, "s");
        assert_eq!(p.explanation, "Left added `o` and right added `t`.");
    }

    #[test]
    fn invalid_output_is_discarded() {
        assert!(is_valid(Lang::Json, "{\"a\": 1}", 0));
        assert!(!is_valid(Lang::Json, "{\"a\": 1", 0));
        assert!(!is_valid(Lang::Java, "class A { void f() { }\n", 0));
        // An input that was already broken tolerates the same amount of breakage.
        assert!(is_valid(Lang::Json, "{\"a\": 1", 1));
    }

    fn chunk(id: u32, kind: ChunkKind, base: (u32, u32)) -> Chunk {
        let r = LineRange {
            start: base.0,
            end: base.1,
        };
        Chunk {
            id,
            kind,
            base: r,
            ours: r,
            theirs: r,
            fine: None,
            fine_diff_skipped: false,
            simple: None,
        }
    }

    fn hunk(before: (u32, u32), after: (u32, u32)) -> LineHunk {
        LineHunk {
            before: before.0..before.1,
            after: after.0..after.1,
        }
    }

    #[test]
    fn windows_grow_to_chunk_boundaries_and_pull_in_hunks() {
        let hunks = vec![hunk((2, 3), (2, 4)), hunk((6, 7), (7, 9))];
        let chunks = vec![
            chunk(0, ChunkKind::Conflict, (1, 3)),
            chunk(1, ChunkKind::OursOnly, (3, 4)), // touches only: not covered
            chunk(2, ChunkKind::Conflict, (6, 8)),
        ];
        let mut w = Window {
            i: 0,
            j: 0,
            s: 2,
            e: 3,
        };
        close_window(&mut w, &hunks, &chunks);
        assert_eq!((w.i, w.j, w.s, w.e), (0, 0, 1, 3));

        // A chunk reaching the next hunk pulls it in.
        let chunks = vec![chunk(0, ChunkKind::Conflict, (2, 7))];
        let mut w = Window {
            i: 0,
            j: 0,
            s: 2,
            e: 3,
        };
        close_window(&mut w, &hunks, &chunks);
        assert_eq!((w.i, w.j, w.s, w.e), (0, 1, 2, 7));
    }

    #[test]
    fn intersection_rules() {
        assert!(intersects(&(2..4), &(3..5)));
        assert!(
            !intersects(&(2..3), &(3..4)),
            "touching ranges do not intersect"
        );
        assert!(intersects(&(3..3), &(2..4)), "insertion point inside");
        assert!(
            intersects(&(3..3), &(3..5)),
            "insertion at the start boundary"
        );
        assert!(
            intersects(&(3..3), &(1..3)),
            "insertion at the end boundary"
        );
        assert!(intersects(&(3..3), &(3..3)));
        assert!(!intersects(&(3..3), &(4..4)));
    }
}
