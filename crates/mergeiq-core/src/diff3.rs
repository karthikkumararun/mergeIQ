use std::ops::Range;

use crate::diff::LineHunk;

/// A half-open `[start, end)` line-index range. An empty range (`start == end`)
/// represents an insertion point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LineRange {
    /// Line index of the range's start.
    pub start: u32,
    /// Line index just past the range's end.
    pub end: u32,
}

impl LineRange {
    /// Number of lines spanned.
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// `true` for an insertion point (`start == end`).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// This range as a `usize` range, for slicing.
    pub fn as_usize_range(&self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// How a [`Chunk`]'s region was changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ChunkKind {
    /// Only `ours` changed this region.
    OursOnly,
    /// Only `theirs` changed this region.
    TheirsOnly,
    /// Both sides changed this region identically (under the active whitespace policy).
    BothSame,
    /// Both sides changed this region differently, or touched it at the same point.
    Conflict,
}

/// One changed (or conflicting) region spanning base/ours/theirs, as produced by
/// [`crate::analyze`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Chunk {
    /// Stable index of this chunk within its [`crate::Analysis`].
    pub id: u32,
    /// How this region changed.
    pub kind: ChunkKind,
    /// Range in the base text.
    pub base: LineRange,
    /// Range in the ours text.
    pub ours: LineRange,
    /// Range in the theirs text.
    pub theirs: LineRange,
    /// Token-level diff of ours/theirs vs base within this chunk. `None` if skipped
    /// (see `fine_diff_skipped`) or not yet computed.
    pub fine: Option<crate::fine::FineDiff>,
    /// Set when this chunk exceeded `Options::fine_diff_max_lines` and fine diffing
    /// was skipped.
    pub fine_diff_skipped: bool,
    /// Token-level 3-way merge result, precomputed for `Conflict` chunks only.
    pub simple: Option<crate::simple::SimpleResolution>,
}

/// Two ranges "touch" if they overlap, or if both are insertions (empty ranges) at the
/// exact same point. Two non-overlapping, non-identical-point ranges that merely abut
/// (e.g. `[1,2)` and `[2,3)`) do NOT touch — matching git's leniency for independent
/// adjacent edits; see `design.md` Decisions.
fn ranges_touch(a: &Range<u32>, b: &Range<u32>) -> bool {
    if a.start == a.end && b.start == b.end {
        a.start == b.start
    } else {
        a.start < b.end && b.start < a.end
    }
}

fn size_delta(h: &LineHunk) -> i64 {
    (h.after.end as i64 - h.after.start as i64) - (h.before.end as i64 - h.before.start as i64)
}

/// Merges base→ours and base→theirs line hunks (sorted, non-overlapping within each
/// list — as produced by [`crate::diff::diff_lines`]) into an ordered list of chunks
/// covering every changed region.
///
/// `ours_keys`/`theirs_keys` are the full-file normalized per-line comparison keys
/// (see [`crate::diff::line_keys`]), used to decide `BothSame` vs `Conflict` when both
/// sides touch the same region.
pub fn merge(
    ours_hunks: &[LineHunk],
    theirs_hunks: &[LineHunk],
    ours_keys: &[String],
    theirs_keys: &[String],
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut ai = 0usize;
    let mut bi = 0usize;
    let mut base_pos = 0u32;
    let mut ours_pos = 0u32;
    let mut theirs_pos = 0u32;

    while ai < ours_hunks.len() || bi < theirs_hunks.len() {
        let next_a = ours_hunks.get(ai).map(|h| h.before.start);
        let next_b = theirs_hunks.get(bi).map(|h| h.before.start);
        let next_change = match (next_a, next_b) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => unreachable!("loop condition guarantees a hunk remains"),
        };

        if next_change > base_pos {
            let gap = next_change - base_pos;
            base_pos += gap;
            ours_pos += gap;
            theirs_pos += gap;
            continue;
        }

        // Seed the cluster with every hunk starting exactly at base_pos (there can be
        // one from each side, e.g. same-point insertions from both).
        let mut frontier = base_pos;
        let mut cluster_a: Vec<usize> = Vec::new();
        let mut cluster_b: Vec<usize> = Vec::new();
        loop {
            let a_here = ai + cluster_a.len() < ours_hunks.len()
                && ours_hunks[ai + cluster_a.len()].before.start == base_pos;
            let b_here = bi + cluster_b.len() < theirs_hunks.len()
                && theirs_hunks[bi + cluster_b.len()].before.start == base_pos;
            if a_here {
                frontier = frontier.max(ours_hunks[ai + cluster_a.len()].before.end);
                cluster_a.push(ai + cluster_a.len());
            } else if b_here {
                frontier = frontier.max(theirs_hunks[bi + cluster_b.len()].before.end);
                cluster_b.push(bi + cluster_b.len());
            } else {
                break;
            }
        }

        // Grow the cluster while later hunks still touch the (growing) group range.
        loop {
            let a_idx = ai + cluster_a.len();
            let b_idx = bi + cluster_b.len();
            let touches_a = a_idx < ours_hunks.len()
                && ranges_touch(&(base_pos..frontier), &ours_hunks[a_idx].before);
            let touches_b = b_idx < theirs_hunks.len()
                && ranges_touch(&(base_pos..frontier), &theirs_hunks[b_idx].before);
            if !touches_a && !touches_b {
                break;
            }
            let take_a = touches_a
                && (!touches_b
                    || ours_hunks[a_idx].before.start <= theirs_hunks[b_idx].before.start);
            if take_a {
                frontier = frontier.max(ours_hunks[a_idx].before.end);
                cluster_a.push(a_idx);
            } else {
                frontier = frontier.max(theirs_hunks[b_idx].before.end);
                cluster_b.push(b_idx);
            }
        }

        let group_len = frontier - base_pos;
        let ours_delta: i64 = cluster_a.iter().map(|&k| size_delta(&ours_hunks[k])).sum();
        let theirs_delta: i64 = cluster_b
            .iter()
            .map(|&k| size_delta(&theirs_hunks[k]))
            .sum();
        let ours_range = LineRange {
            start: ours_pos,
            end: (ours_pos as i64 + group_len as i64 + ours_delta) as u32,
        };
        let theirs_range = LineRange {
            start: theirs_pos,
            end: (theirs_pos as i64 + group_len as i64 + theirs_delta) as u32,
        };
        let base_range = LineRange {
            start: base_pos,
            end: frontier,
        };

        let kind = if cluster_a.is_empty() {
            ChunkKind::TheirsOnly
        } else if cluster_b.is_empty() {
            ChunkKind::OursOnly
        } else {
            let ours_slice = &ours_keys[ours_range.as_usize_range()];
            let theirs_slice = &theirs_keys[theirs_range.as_usize_range()];
            if ours_slice == theirs_slice {
                ChunkKind::BothSame
            } else {
                ChunkKind::Conflict
            }
        };

        chunks.push(Chunk {
            id: chunks.len() as u32,
            kind,
            base: base_range,
            ours: ours_range,
            theirs: theirs_range,
            fine: None,
            fine_diff_skipped: false,
            simple: None,
        });

        base_pos = frontier;
        ours_pos = ours_range.end;
        theirs_pos = theirs_range.end;
        ai += cluster_a.len();
        bi += cluster_b.len();
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{diff_lines, line_keys, WhitespacePolicy};
    use crate::text::split_lines;

    fn run(base: &str, ours: &str, theirs: &str) -> Vec<Chunk> {
        let base_lines = split_lines(base);
        let ours_lines = split_lines(ours);
        let theirs_lines = split_lines(theirs);
        let base_keys = line_keys(base, &base_lines, WhitespacePolicy::Exact);
        let ours_keys = line_keys(ours, &ours_lines, WhitespacePolicy::Exact);
        let theirs_keys = line_keys(theirs, &theirs_lines, WhitespacePolicy::Exact);
        let ours_hunks = diff_lines(&base_keys, &ours_keys);
        let theirs_hunks = diff_lines(&base_keys, &theirs_keys);
        merge(&ours_hunks, &theirs_hunks, &ours_keys, &theirs_keys)
    }

    #[test]
    fn no_changes_yields_no_chunks() {
        let base = "a\nb\nc\n";
        assert_eq!(run(base, base, base), vec![]);
    }

    #[test]
    fn non_overlapping_edits_produce_two_chunks() {
        let base: String = (1..=20).map(|n| format!("l{n}\n")).collect();
        let mut ours = base.clone();
        ours = ours.replacen("l2\n", "L2\n", 1);
        let mut theirs = base.clone();
        theirs = theirs.replacen("l10\n", "L10\n", 1);

        let chunks = run(&base, &ours, &theirs);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::OursOnly);
        assert_eq!(chunks[0].base, LineRange { start: 1, end: 2 });
        assert_eq!(chunks[1].kind, ChunkKind::TheirsOnly);
        assert_eq!(chunks[1].base, LineRange { start: 9, end: 10 });
    }

    #[test]
    fn identical_edits_are_both_same() {
        let base = "a\nb\nc\n";
        let changed = "a\nB\nc\n";
        let chunks = run(base, changed, changed);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::BothSame);
        assert_eq!(chunks[0].base, LineRange { start: 1, end: 2 });
        assert_eq!(chunks[0].ours, LineRange { start: 1, end: 2 });
        assert_eq!(chunks[0].theirs, LineRange { start: 1, end: 2 });
    }

    #[test]
    fn overlapping_edits_are_one_conflict_chunk() {
        let base = "l1\nl2\nl3\nl4\nl5\nl6\nl7\n";
        // ours changes lines 3-5 (idx 2..5), theirs changes lines 5-7 (idx 4..7);
        // they share base line 5 (idx 4), a true overlap.
        let ours = "l1\nl2\nL3\nL4\nL5\nl6\nl7\n";
        let theirs = "l1\nl2\nl3\nl4\nX5\nX6\nX7\n";
        let chunks = run(base, ours, theirs);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Conflict);
        assert_eq!(chunks[0].base, LineRange { start: 2, end: 7 });
    }

    #[test]
    fn same_point_insertions_are_one_conflict_chunk_with_empty_base_range() {
        let base = "l1\nl2\nl3\nl4\nl5\n";
        let ours = "l1\nl2\nl3\nl4\nOURS\nl5\n";
        let theirs = "l1\nl2\nl3\nl4\nTHEIRS\nl5\n";
        let chunks = run(base, ours, theirs);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Conflict);
        assert_eq!(chunks[0].base, LineRange { start: 4, end: 4 });
        assert_eq!(chunks[0].ours, LineRange { start: 4, end: 5 });
        assert_eq!(chunks[0].theirs, LineRange { start: 4, end: 5 });
    }

    #[test]
    fn adjacent_non_overlapping_edits_stay_separate() {
        // ours changes base line idx 1 ("l2"), theirs changes the immediately
        // following base line idx 2 ("l3"). They abut but never share a line, so
        // they should NOT be forced into one conflict (matches git's leniency).
        let base = "l1\nl2\nl3\nl4\n";
        let ours = "l1\nL2\nl3\nl4\n";
        let theirs = "l1\nl2\nL3\nl4\n";
        let chunks = run(base, ours, theirs);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::OursOnly);
        assert_eq!(chunks[1].kind, ChunkKind::TheirsOnly);
    }
}
