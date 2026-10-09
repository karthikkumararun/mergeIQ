//! Generic keyed 3-way merge of entry lists.
//!
//! The merge works on keys and text fingerprints only; it knows nothing about syntax trees.
//! Entries both sides changed differently are handed to a caller-supplied hook (which may
//! recurse into nested containers or merge import name lists). Everything the merger cannot
//! resolve is reported as a [`Conflict`] so the caller can protect the affected base range
//! instead of failing the whole container.

use std::collections::{HashMap, HashSet};

/// Which side of the merge a change came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The current / working side.
    Ours,
    /// The incoming side.
    Theirs,
}

/// One entry as seen by the merger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item<'a> {
    /// Identity within its container. Must be unique per list.
    pub key: &'a str,
    /// Fingerprint of the entry's text; equal text means "unchanged".
    pub text: &'a str,
    /// Imports use set-union semantics and sorted insertion.
    pub import: bool,
    /// The key is positional/textual rather than semantic.
    pub opaque: bool,
}

/// Where a merged entry's content comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Entry `i` of the base list.
    Base(usize),
    /// Entry `i` of the ours list.
    Ours(usize),
    /// Entry `i` of the theirs list.
    Theirs(usize),
    /// Caller-defined payload `i` returned by the hook.
    Custom(usize),
}

/// One entry of the merged sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Out {
    /// The content source.
    pub origin: Origin,
    /// The entry's key.
    pub key: String,
    /// The entry is an import.
    pub import: bool,
    /// The base entry this one corresponds to, if any.
    pub base: Option<usize>,
}

/// Something the merger could not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    /// Base entry `base` was changed incompatibly (changed by both, or changed vs deleted).
    /// Its base text is kept in the output and the caller must protect its range.
    Entry {
        /// Index into the base list.
        base: usize,
    },
    /// Both sides added `key` with different text. Neither addition is emitted; the caller
    /// must protect the slots after the listed base anchors.
    Slot {
        /// The conflicting key.
        key: String,
        /// Base entry each addition followed (`None` = at the start).
        anchors: Vec<Option<usize>>,
    },
}

/// What happened to an entry, for explanations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A side added the entry.
    Added(Side),
    /// A side changed the entry.
    Changed(Side),
    /// A side deleted the entry.
    Removed(Side),
    /// Both sides made the same change or the same addition.
    BothSame,
    /// Both sides changed the entry and the changes were combined.
    Combined,
}

/// Position of an [`Event`] in base coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pos {
    /// At base entry `i`.
    Entry(usize),
    /// In the slot after base entry `i` (`None` = before the first entry).
    Slot(Option<usize>),
}

/// A merge event used to explain a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// What happened.
    pub change: Change,
    /// The entry's key.
    pub key: String,
    /// Where, in base coordinates.
    pub pos: Pos,
}

/// The result of a keyed merge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Merged {
    /// The merged sequence.
    pub items: Vec<Out>,
    /// Unresolved spots.
    pub conflicts: Vec<Conflict>,
    /// What happened, for explanations.
    pub events: Vec<Event>,
}

/// Reasons a whole container cannot be merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Bail {
    /// A key appears twice in one list.
    #[error("duplicate key")]
    Duplicate,
    /// A side reordered entries that exist in the base.
    #[error("entries were reordered")]
    Reordered,
}

/// The hook invoked for entries both sides changed differently: `(base, ours, theirs)`
/// indices. Returns a payload id, or `None` if the changes cannot be combined.
pub type BothChangedHook<'h> = dyn FnMut(usize, usize, usize) -> Option<usize> + 'h;

fn index<'a>(items: &[Item<'a>]) -> Result<HashMap<&'a str, usize>, Bail> {
    let mut map = HashMap::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        if map.insert(it.key, i).is_some() {
            return Err(Bail::Duplicate);
        }
    }
    Ok(map)
}

fn keeps_base_order(side: &[Item], base_pos: &HashMap<&str, usize>) -> bool {
    let mut last: Option<usize> = None;
    for it in side {
        if let Some(&b) = base_pos.get(it.key) {
            if last.is_some_and(|l| b < l) {
                return false;
            }
            last = Some(b);
        }
    }
    true
}

fn imports_sorted(items: &[Item]) -> bool {
    let keys: Vec<&str> = items.iter().filter(|i| i.import).map(|i| i.key).collect();
    keys.windows(2).all(|w| w[0] <= w[1])
}

struct Addition {
    side: Side,
    idx: usize,
    anchor: Option<usize>,
}

/// Merges `ours` and `theirs` against `base` by entry key.
///
/// Rules (see the `structural-merge` spec): additions by one side are included; both sides'
/// additions with different keys are included, ours first, at the position where they were
/// added; identical additions are included once; a change by one side wins; a deletion wins
/// over an unchanged entry; changes by both are combined by `both_changed` or reported as a
/// conflict; deletion against change is a conflict.
pub fn merge3(
    base: &[Item],
    ours: &[Item],
    theirs: &[Item],
    both_changed: &mut BothChangedHook,
) -> Result<Merged, Bail> {
    let bpos = index(base)?;
    let opos = index(ours)?;
    let tpos = index(theirs)?;
    if !keeps_base_order(ours, &bpos) || !keeps_base_order(theirs, &bpos) {
        return Err(Bail::Reordered);
    }

    // Collect additions anchored after the nearest preceding base-keyed entry.
    let mut additions: Vec<Addition> = Vec::new();
    for (side, items) in [(Side::Ours, ours), (Side::Theirs, theirs)] {
        let mut anchor: Option<usize> = None;
        for (idx, it) in items.iter().enumerate() {
            match bpos.get(it.key) {
                Some(&b) => anchor = Some(b),
                None => additions.push(Addition { side, idx, anchor }),
            }
        }
    }

    let mut merged = Merged::default();
    let mut dropped: HashSet<(Side, usize)> = HashSet::new();
    // Additions present on both sides.
    for a in additions.iter().filter(|a| a.side == Side::Ours) {
        let key = ours[a.idx].key;
        if let Some(&ti) = tpos.get(key) {
            if bpos.contains_key(key) {
                continue;
            }
            let t_anchor = additions
                .iter()
                .find(|t| t.side == Side::Theirs && t.idx == ti)
                .and_then(|t| t.anchor);
            dropped.insert((Side::Theirs, ti));
            if ours[a.idx].text == theirs[ti].text {
                merged.events.push(Event {
                    change: Change::BothSame,
                    key: key.to_string(),
                    pos: Pos::Slot(a.anchor),
                });
            } else {
                dropped.insert((Side::Ours, a.idx));
                merged.conflicts.push(Conflict::Slot {
                    key: key.to_string(),
                    anchors: vec![a.anchor, t_anchor],
                });
            }
        }
    }

    let sorted_mode = imports_sorted(base) && imports_sorted(ours) && imports_sorted(theirs);
    let mut deferred: Vec<usize> = Vec::new();
    // Output position where each anchor's additions go (for the sorted-mode fallback).
    let mut slot_pos: HashMap<Option<usize>, usize> = HashMap::new();

    let emit_adds = |anchor: Option<usize>,
                     merged: &mut Merged,
                     deferred: &mut Vec<usize>,
                     slot_pos: &mut HashMap<Option<usize>, usize>| {
        slot_pos.insert(anchor, merged.items.len());
        for side in [Side::Ours, Side::Theirs] {
            for (ai, a) in additions
                .iter()
                .enumerate()
                .filter(|(_, a)| a.side == side && a.anchor == anchor)
            {
                if dropped.contains(&(a.side, a.idx)) {
                    continue;
                }
                let it = match side {
                    Side::Ours => &ours[a.idx],
                    Side::Theirs => &theirs[a.idx],
                };
                if sorted_mode && it.import {
                    deferred.push(ai);
                    continue;
                }
                merged.events.push(Event {
                    change: Change::Added(side),
                    key: it.key.to_string(),
                    pos: Pos::Slot(anchor),
                });
                merged.items.push(Out {
                    origin: match side {
                        Side::Ours => Origin::Ours(a.idx),
                        Side::Theirs => Origin::Theirs(a.idx),
                    },
                    key: it.key.to_string(),
                    import: it.import,
                    base: None,
                });
            }
        }
    };

    emit_adds(None, &mut merged, &mut deferred, &mut slot_pos);
    for (b, bi) in base.iter().enumerate() {
        let o = opos.get(bi.key).copied();
        let t = tpos.get(bi.key).copied();
        let keep_base = |merged: &mut Merged| {
            merged.items.push(Out {
                origin: Origin::Base(b),
                key: bi.key.to_string(),
                import: bi.import,
                base: Some(b),
            });
        };
        let event = |merged: &mut Merged, change: Change| {
            merged.events.push(Event {
                change,
                key: bi.key.to_string(),
                pos: Pos::Entry(b),
            });
        };
        match (o, t) {
            (Some(oi), Some(ti)) => {
                let o_changed = ours[oi].text != bi.text;
                let t_changed = theirs[ti].text != bi.text;
                match (o_changed, t_changed) {
                    (false, false) => keep_base(&mut merged),
                    (true, false) => {
                        event(&mut merged, Change::Changed(Side::Ours));
                        merged.items.push(Out {
                            origin: Origin::Ours(oi),
                            key: bi.key.to_string(),
                            import: bi.import,
                            base: Some(b),
                        });
                    }
                    (false, true) => {
                        event(&mut merged, Change::Changed(Side::Theirs));
                        merged.items.push(Out {
                            origin: Origin::Theirs(ti),
                            key: bi.key.to_string(),
                            import: bi.import,
                            base: Some(b),
                        });
                    }
                    (true, true) => {
                        if ours[oi].text == theirs[ti].text {
                            event(&mut merged, Change::BothSame);
                            merged.items.push(Out {
                                origin: Origin::Ours(oi),
                                key: bi.key.to_string(),
                                import: bi.import,
                                base: Some(b),
                            });
                        } else if let Some(c) = both_changed(b, oi, ti) {
                            event(&mut merged, Change::Combined);
                            merged.items.push(Out {
                                origin: Origin::Custom(c),
                                key: bi.key.to_string(),
                                import: bi.import,
                                base: Some(b),
                            });
                        } else {
                            merged.conflicts.push(Conflict::Entry { base: b });
                            keep_base(&mut merged);
                        }
                    }
                }
            }
            (None, Some(ti)) => {
                if theirs[ti].text == bi.text && !bi.opaque {
                    event(&mut merged, Change::Removed(Side::Ours));
                } else {
                    merged.conflicts.push(Conflict::Entry { base: b });
                    keep_base(&mut merged);
                }
            }
            (Some(oi), None) => {
                if ours[oi].text == bi.text && !bi.opaque {
                    event(&mut merged, Change::Removed(Side::Theirs));
                } else {
                    merged.conflicts.push(Conflict::Entry { base: b });
                    keep_base(&mut merged);
                }
            }
            (None, None) => {
                if bi.opaque {
                    merged.conflicts.push(Conflict::Entry { base: b });
                    keep_base(&mut merged);
                } else {
                    event(&mut merged, Change::BothSame);
                }
            }
        }
        emit_adds(Some(b), &mut merged, &mut deferred, &mut slot_pos);
    }

    // Sorted-mode import additions.
    if !deferred.is_empty() {
        let has_imports = merged.items.iter().any(|o| o.import);
        if has_imports {
            let mut adds: Vec<(&str, Side, usize, Option<usize>)> = deferred
                .iter()
                .map(|&ai| {
                    let a = &additions[ai];
                    let key = match a.side {
                        Side::Ours => ours[a.idx].key,
                        Side::Theirs => theirs[a.idx].key,
                    };
                    (key, a.side, a.idx, a.anchor)
                })
                .collect();
            adds.sort_by(|x, y| x.0.cmp(y.0));
            for (key, side, idx, anchor) in adds {
                let at = merged
                    .items
                    .iter()
                    .position(|o| o.import && o.key.as_str() > key)
                    .unwrap_or_else(|| {
                        merged
                            .items
                            .iter()
                            .rposition(|o| o.import)
                            .map_or(merged.items.len(), |p| p + 1)
                    });
                merged.items.insert(
                    at,
                    Out {
                        origin: match side {
                            Side::Ours => Origin::Ours(idx),
                            Side::Theirs => Origin::Theirs(idx),
                        },
                        key: key.to_string(),
                        import: true,
                        base: None,
                    },
                );
                merged.events.push(Event {
                    change: Change::Added(side),
                    key: key.to_string(),
                    pos: Pos::Slot(anchor),
                });
            }
        } else {
            // No import entry left to order against: fall back to anchored positions.
            let mut by_anchor: HashMap<Option<usize>, Vec<&Addition>> = HashMap::new();
            for &ai in &deferred {
                let a = &additions[ai];
                by_anchor.entry(a.anchor).or_default().push(a);
            }
            let mut anchors: Vec<Option<usize>> = by_anchor.keys().copied().collect();
            anchors.sort_by_key(|a| std::cmp::Reverse(slot_pos.get(a).copied().unwrap_or(0)));
            for anchor in anchors {
                let at = slot_pos.get(&anchor).copied().unwrap_or(merged.items.len());
                let group: Vec<Out> = by_anchor[&anchor]
                    .iter()
                    .map(|a| {
                        let it = match a.side {
                            Side::Ours => &ours[a.idx],
                            Side::Theirs => &theirs[a.idx],
                        };
                        Out {
                            origin: match a.side {
                                Side::Ours => Origin::Ours(a.idx),
                                Side::Theirs => Origin::Theirs(a.idx),
                            },
                            key: it.key.to_string(),
                            import: true,
                            base: None,
                        }
                    })
                    .collect();
                for a in &by_anchor[&anchor] {
                    let it = match a.side {
                        Side::Ours => &ours[a.idx],
                        Side::Theirs => &theirs[a.idx],
                    };
                    merged.events.push(Event {
                        change: Change::Added(a.side),
                        key: it.key.to_string(),
                        pos: Pos::Slot(anchor),
                    });
                }
                merged.items.splice(at..at, group);
            }
        }
    }

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items<'a>(spec: &[&'a str]) -> Vec<Item<'a>> {
        spec.iter()
            .map(|s| {
                let (key, text) = s.split_once('=').unwrap_or((s, s));
                let import = key.starts_with("imp:");
                Item {
                    key,
                    text,
                    import,
                    opaque: key.starts_with('~'),
                }
            })
            .collect()
    }

    /// Runs a merge and renders the output as `key=text` strings (`!` marks conflicts).
    fn run(
        base: &[&str],
        ours: &[&str],
        theirs: &[&str],
    ) -> Result<(Vec<String>, Vec<Conflict>), Bail> {
        let (b, o, t) = (items(base), items(ours), items(theirs));
        let mut hook = |_b: usize, _o: usize, _t: usize| None;
        let m = merge3(&b, &o, &t, &mut hook)?;
        let out = m
            .items
            .iter()
            .map(|x| match x.origin {
                Origin::Base(i) => format!("{}={}", b[i].key, b[i].text),
                Origin::Ours(i) => format!("{}={}", o[i].key, o[i].text),
                Origin::Theirs(i) => format!("{}={}", t[i].key, t[i].text),
                Origin::Custom(i) => format!("custom{i}"),
            })
            .collect();
        Ok((out, m.conflicts))
    }

    fn ok(base: &[&str], ours: &[&str], theirs: &[&str]) -> Vec<String> {
        let (out, conflicts) = run(base, ours, theirs).unwrap();
        assert!(conflicts.is_empty(), "unexpected conflicts: {conflicts:?}");
        out
    }

    #[test]
    fn untouched_entries_stay() {
        assert_eq!(
            ok(&["a=1", "b=2"], &["a=1", "b=2"], &["a=1", "b=2"]),
            ["a=1", "b=2"]
        );
    }

    #[test]
    fn addition_by_one_side_is_included() {
        assert_eq!(ok(&["a=1"], &["a=1", "b=2"], &["a=1"]), ["a=1", "b=2"]);
        assert_eq!(ok(&["a=1"], &["a=1"], &["a=1", "c=3"]), ["a=1", "c=3"]);
    }

    #[test]
    fn additions_at_the_same_position_are_ours_then_theirs() {
        assert_eq!(
            ok(&["a=1"], &["a=1", "x=1"], &["a=1", "y=2"]),
            ["a=1", "x=1", "y=2"]
        );
    }

    #[test]
    fn additions_keep_their_anchor() {
        assert_eq!(
            ok(
                &["a=1", "b=2"],
                &["a=1", "x=9", "b=2"],
                &["a=1", "b=2", "y=8"]
            ),
            ["a=1", "x=9", "b=2", "y=8"]
        );
    }

    #[test]
    fn additions_at_the_start_come_first() {
        assert_eq!(
            ok(&["a=1"], &["x=1", "a=1"], &["y=1", "a=1"]),
            ["x=1", "y=1", "a=1"]
        );
    }

    #[test]
    fn identical_additions_are_included_once() {
        assert_eq!(
            ok(&["a=1"], &["a=1", "n=7"], &["a=1", "n=7"]),
            ["a=1", "n=7"]
        );
    }

    #[test]
    fn same_key_added_differently_is_a_slot_conflict() {
        let (out, conflicts) = run(&["a=1"], &["a=1", "n=7"], &["a=1", "n=8"]).unwrap();
        assert_eq!(out, ["a=1"]);
        assert_eq!(
            conflicts,
            [Conflict::Slot {
                key: "n".into(),
                anchors: vec![Some(0), Some(0)]
            }]
        );
    }

    #[test]
    fn change_by_one_side_wins() {
        assert_eq!(
            ok(&["a=1", "b=2"], &["a=1", "b=3"], &["a=1", "b=2"]),
            ["a=1", "b=3"]
        );
        assert_eq!(
            ok(&["a=1", "b=2"], &["a=1", "b=2"], &["a=5", "b=2"]),
            ["a=5", "b=2"]
        );
    }

    #[test]
    fn identical_change_on_both_sides_is_taken_once() {
        assert_eq!(ok(&["a=1"], &["a=2"], &["a=2"]), ["a=2"]);
    }

    #[test]
    fn deletion_of_unchanged_entry_is_honored() {
        assert_eq!(ok(&["a=1", "b=2"], &["a=1"], &["a=1", "b=2"]), ["a=1"]);
        assert_eq!(ok(&["a=1", "b=2"], &["a=1", "b=2"], &["b=2"]), ["b=2"]);
        assert_eq!(ok(&["a=1", "b=2"], &["a=1"], &["a=1"]), ["a=1"]);
    }

    #[test]
    fn delete_versus_change_is_an_entry_conflict() {
        let (out, conflicts) = run(&["a=1", "b=2"], &["a=1"], &["a=1", "b=3"]).unwrap();
        assert_eq!(out, ["a=1", "b=2"]);
        assert_eq!(conflicts, [Conflict::Entry { base: 1 }]);
    }

    #[test]
    fn both_changed_without_hook_is_an_entry_conflict() {
        let (out, conflicts) = run(&["a=1"], &["a=2"], &["a=3"]).unwrap();
        assert_eq!(out, ["a=1"]);
        assert_eq!(conflicts, [Conflict::Entry { base: 0 }]);
    }

    #[test]
    fn both_changed_hook_result_is_used() {
        let (b, o, t) = (items(&["a=1"]), items(&["a=2"]), items(&["a=3"]));
        let mut hook = |bi: usize, oi: usize, ti: usize| {
            assert_eq!((bi, oi, ti), (0, 0, 0));
            Some(42)
        };
        let m = merge3(&b, &o, &t, &mut hook).unwrap();
        assert_eq!(m.items[0].origin, Origin::Custom(42));
        assert!(m.conflicts.is_empty());
    }

    #[test]
    fn additions_survive_next_to_a_conflict() {
        let (out, conflicts) =
            run(&["a=1", "b=2"], &["a=9", "b=2", "x=1"], &["a=8", "b=2"]).unwrap();
        assert_eq!(out, ["a=1", "b=2", "x=1"]);
        assert_eq!(conflicts, [Conflict::Entry { base: 0 }]);
    }

    #[test]
    fn addition_after_a_deleted_anchor_keeps_its_slot() {
        assert_eq!(
            ok(
                &["a=1", "b=2", "c=3"],
                &["a=1", "b=2", "x=5", "c=3"],
                &["a=1", "c=3"]
            ),
            ["a=1", "x=5", "c=3"]
        );
    }

    #[test]
    fn reordering_bails() {
        assert_eq!(
            run(&["a=1", "b=2"], &["b=2", "a=1"], &["a=1", "b=2"]).unwrap_err(),
            Bail::Reordered
        );
    }

    #[test]
    fn duplicate_keys_bail() {
        assert_eq!(
            run(&["a=1", "a=2"], &["a=1"], &["a=1"]).unwrap_err(),
            Bail::Duplicate
        );
    }

    #[test]
    fn opaque_entry_deleted_by_both_is_a_conflict() {
        let (_, conflicts) = run(&["~x=1"], &[], &[]).unwrap();
        assert_eq!(conflicts, [Conflict::Entry { base: 0 }]);
    }

    #[test]
    fn sorted_imports_insert_in_sorted_order() {
        assert_eq!(
            ok(
                &["imp:a=a", "imp:c=c", "imp:e=e"],
                &["imp:a=a", "imp:b=b", "imp:c=c", "imp:e=e"],
                &["imp:a=a", "imp:c=c", "imp:d=d", "imp:e=e"]
            ),
            ["imp:a=a", "imp:b=b", "imp:c=c", "imp:d=d", "imp:e=e"]
        );
    }

    #[test]
    fn sorted_imports_append_after_the_last_import() {
        assert_eq!(
            ok(
                &["imp:a=a", "imp:b=b"],
                &["imp:a=a", "imp:b=b", "imp:z=z"],
                &["imp:a=a", "imp:b=b"]
            ),
            ["imp:a=a", "imp:b=b", "imp:z=z"]
        );
    }

    #[test]
    fn unsorted_imports_use_anchored_order() {
        assert_eq!(
            ok(
                &["imp:z=z", "imp:a=a"],
                &["imp:z=z", "imp:a=a", "imp:m=m"],
                &["imp:z=z", "imp:a=a", "imp:b=b"]
            ),
            ["imp:z=z", "imp:a=a", "imp:m=m", "imp:b=b"]
        );
    }

    #[test]
    fn sorted_import_additions_fall_back_when_no_import_remains() {
        assert_eq!(
            ok(
                &["x=1", "imp:a=a"],
                &["x=1"],
                &["x=1", "imp:a=a", "imp:b=b"]
            ),
            ["x=1", "imp:b=b"]
        );
    }

    #[test]
    fn non_import_entries_are_never_sorted() {
        // Both sides append after the last (alphabetically "late") member.
        assert_eq!(
            ok(
                &["m=1", "z=2"],
                &["m=1", "z=2", "a=3"],
                &["m=1", "z=2", "b=4"]
            ),
            ["m=1", "z=2", "a=3", "b=4"]
        );
    }
}
