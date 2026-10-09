//! Merging one container triple and assembling the merged interior text.
//!
//! Entry text is always copied from the source versions (formatting and comments preserved),
//! never re-printed from the syntax tree. Between entries the gap text of the entry's source
//! version is used; separators (commas in JSON and object literals) are regenerated so the
//! sequence stays valid and the base's trailing-separator style is kept.

use crate::entries::{Container, Doc, Entry};
use crate::imports::merge_names;
use crate::keyed3::{self, Change, Conflict, Item, Origin, Pos};
use crate::lang::Lang;
use std::ops::Range;

/// A merge event in base byte coordinates, for explanations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ev {
    /// What happened.
    pub change: Change,
    /// Human-readable entry name.
    pub name: String,
    /// Human-readable container name (empty for the file level).
    pub container: String,
    /// Byte offset in the base text.
    pub byte: usize,
}

/// The merged text of one container interior.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rendered {
    /// Replacement for the base interior (or, for nested entries, for the entry text).
    pub text: String,
    /// Base byte ranges that must not be covered by a proposal.
    pub protected: Vec<Range<usize>>,
    /// What happened, for explanations.
    pub events: Vec<Ev>,
}

/// The three versions being merged.
pub struct Ctx<'a, 'd> {
    /// The language.
    pub lang: Lang,
    /// The base version.
    pub base: &'d Doc<'a>,
    /// The ours version.
    pub ours: &'d Doc<'a>,
    /// The theirs version.
    pub theirs: &'d Doc<'a>,
}

/// Turns a key such as `method_declaration:run\u{1f}int` into `run(int)` for display.
pub fn display_key(key: &str) -> String {
    let rest = key.split_once(':').map_or(key, |(_, r)| r);
    let rest = rest.trim_start_matches('~');
    let mut parts = rest.split('\u{1f}');
    let name = parts.next().unwrap_or("").trim_matches('"');
    let params: Vec<&str> = parts.collect();
    if params.is_empty() {
        name.to_string()
    } else {
        format!("{name}({})", params.join(", "))
    }
}

fn merge_str<'s>(base: &'s str, ours: &'s str, theirs: &'s str) -> Option<&'s str> {
    if ours == theirs || theirs == base {
        Some(ours)
    } else if ours == base {
        Some(theirs)
    } else {
        None
    }
}

fn container_label(doc: &Doc, idx: usize) -> String {
    match doc.containers[idx].parent {
        Some((pc, pe)) => display_key(&doc.containers[pc].entries[pe].key),
        None => String::new(),
    }
}

fn items<'a>(doc: &'a Doc, c: &'a Container) -> Vec<Item<'a>> {
    c.entries
        .iter()
        .map(|e| Item {
            key: &e.key,
            text: doc.slice(&e.core),
            import: e.is_import,
            opaque: e.opaque,
        })
        .collect()
}

/// The gap text used between entries of `c`.
fn typical_gap<'a>(doc: &'a Doc, c: &Container) -> &'a str {
    match c.entries.get(1).or(c.entries.first()) {
        Some(e) => doc.slice(&e.lead),
        None => "",
    }
}

/// The gap in front of an entry that ends up at output position `pos`.
fn gap_for<'a>(doc: &'a Doc, c: &Container, entry: &Entry, idx: usize, pos: usize) -> &'a str {
    if pos == 0 {
        // Head gap: the base's if it has entries (handled by the caller passing `base`).
        doc.slice(&entry.lead)
    } else if idx == 0 {
        typical_gap(doc, c)
    } else {
        doc.slice(&entry.lead)
    }
}

/// Merges container `cb` (base) with `co` (ours) and `ct` (theirs), which must correspond.
///
/// Returns `Err(reason)` if the container as a whole cannot be merged.
pub fn merge_container(ctx: &Ctx, cb: usize, co: usize, ct: usize) -> Result<Rendered, String> {
    let (b, o, t) = (
        &ctx.base.containers[cb],
        &ctx.ours.containers[co],
        &ctx.theirs.containers[ct],
    );
    for (name, c) in [("base", b), ("ours", o), ("theirs", t)] {
        if let Some(reason) = &c.bail {
            return Err(format!("{name}: {reason}"));
        }
    }
    let seps: Vec<&str> = [b.sep.as_deref(), o.sep.as_deref(), t.sep.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    if seps.windows(2).any(|w| w[0] != w[1]) {
        return Err("separator mismatch".into());
    }
    let sep: Option<&str> = seps.first().copied();
    let label = container_label(ctx.base, cb);

    let (bi, oi, ti) = (items(ctx.base, b), items(ctx.ours, o), items(ctx.theirs, t));

    let mut customs: Vec<Rendered> = Vec::new();
    let mut hook = |be: usize, oe: usize, te: usize| -> Option<usize> {
        let (eb, eo, et) = (&b.entries[be], &o.entries[oe], &t.entries[te]);
        // Same import statement edited on both sides: merge the name lists.
        if eb.is_import && eo.is_import && et.is_import {
            if let Some(text) = merge_names(
                ctx.lang,
                ctx.base.slice(&eb.core),
                ctx.ours.slice(&eo.core),
                ctx.theirs.slice(&et.core),
            ) {
                customs.push(Rendered {
                    text,
                    ..Rendered::default()
                });
                return Some(customs.len() - 1);
            }
        }
        // Both edited a container entry: recurse into the nested containers.
        let (Some(nb), Some(no), Some(nt)) = (eb.child, eo.child, et.child) else {
            return None;
        };
        let (cbn, con, ctn) = (
            &ctx.base.containers[nb],
            &ctx.ours.containers[no],
            &ctx.theirs.containers[nt],
        );
        let pre = |doc: &Doc, e: &Entry, c: &Container| {
            doc.text[e.core.start..c.interior.start].to_string()
        };
        let post =
            |doc: &Doc, e: &Entry, c: &Container| doc.text[c.interior.end..e.core.end].to_string();
        let (pre_b, pre_o, pre_t) = (
            pre(ctx.base, eb, cbn),
            pre(ctx.ours, eo, con),
            pre(ctx.theirs, et, ctn),
        );
        let (post_b, post_o, post_t) = (
            post(ctx.base, eb, cbn),
            post(ctx.ours, eo, con),
            post(ctx.theirs, et, ctn),
        );
        let pre_m = merge_str(&pre_b, &pre_o, &pre_t)?.to_string();
        let post_m = merge_str(&post_b, &post_o, &post_t)?.to_string();
        let nested = merge_container(ctx, nb, no, nt).ok()?;
        customs.push(Rendered {
            text: format!("{pre_m}{}{post_m}", nested.text),
            protected: nested.protected,
            events: nested.events,
        });
        Some(customs.len() - 1)
    };

    let merged = keyed3::merge3(&bi, &oi, &ti, &mut hook).map_err(|e| e.to_string())?;

    // Assemble the interior text.
    let trailing_sep = if !b.entries.is_empty() {
        b.trailing_sep
    } else {
        o.trailing_sep || t.trailing_sep
    };
    let n = merged.items.len();
    let mut text = String::new();
    for (pos, m) in merged.items.iter().enumerate() {
        let (gap, core): (&str, &str) = match m.origin {
            Origin::Base(i) => {
                let e = &b.entries[i];
                (gap_for(ctx.base, b, e, i, pos), ctx.base.slice(&e.core))
            }
            Origin::Ours(i) => {
                let e = &o.entries[i];
                (gap_for(ctx.ours, o, e, i, pos), ctx.ours.slice(&e.core))
            }
            Origin::Theirs(i) => {
                let e = &t.entries[i];
                (gap_for(ctx.theirs, t, e, i, pos), ctx.theirs.slice(&e.core))
            }
            Origin::Custom(i) => {
                let be = m.base.unwrap_or(0);
                let e = &b.entries[be];
                (gap_for(ctx.base, b, e, be, pos), customs[i].text.as_str())
            }
        };
        let gap = if pos == 0 && !b.entries.is_empty() {
            ctx.base.slice(&b.entries[0].lead)
        } else {
            gap
        };
        text.push_str(gap);
        text.push_str(core);
        if let Some(s) = sep {
            if pos + 1 < n || trailing_sep {
                text.push_str(s);
            }
        }
    }
    text.push_str(ctx.base.slice(&b.tail));

    // Protected ranges and events (base coordinates).
    let mut protected: Vec<Range<usize>> = Vec::new();
    let slot_range = |anchor: Option<usize>| -> Range<usize> {
        let start = match anchor {
            Some(i) => b.entries[i].core.end,
            None => b.interior.start,
        };
        let next = anchor.map_or(0, |i| i + 1);
        let end = b.entries.get(next).map_or(b.interior.end, |e| e.core.start);
        start..end.max(start)
    };
    for c in &merged.conflicts {
        match c {
            Conflict::Entry { base } => protected.push(b.entries[*base].core.clone()),
            Conflict::Slot { anchors, .. } => {
                for a in anchors {
                    protected.push(slot_range(*a));
                }
            }
        }
    }
    let mut events: Vec<Ev> = Vec::new();
    for ev in &merged.events {
        let byte = match ev.pos {
            Pos::Entry(i) => b.entries[i].core.start,
            Pos::Slot(a) => slot_range(a).start,
        };
        events.push(Ev {
            change: ev.change,
            name: display_key(&ev.key),
            container: label.clone(),
            byte,
        });
    }
    for m in &merged.items {
        if let Origin::Custom(i) = m.origin {
            protected.extend(customs[i].protected.iter().cloned());
            events.extend(customs[i].events.iter().cloned());
        }
    }
    Ok(Rendered {
        text,
        protected,
        events,
    })
}
