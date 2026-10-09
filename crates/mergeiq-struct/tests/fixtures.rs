//! Fixture runner: `tests/fixtures/<lang>/<case>/{base,ours,theirs,expected|unresolvable}`.
//!
//! * `expected` – every conflict chunk must be resolved by proposals, and applying all
//!   proposals plus the non-conflicting chunks to the base must reproduce this file exactly.
//! * `unresolved` (optional, next to `expected`) – the number of conflict chunks that proposals
//!   are expected to leave unresolved (default 0).
//! * `unresolvable` – no proposal may be produced for the case.
//!
//! Set `BLESS=1` to (re)write `expected` / `unresolvable` for cases that have neither, and
//! `BLESS=all` to rewrite them for every case. Review the printed results before committing.

use mergeiq_core::{analyze, Analysis, ChunkKind, MergeInput, Options};
use mergeiq_struct::{propose, Outcome, Proposal};
use std::fs;
use std::path::{Path, PathBuf};

fn ext_for(lang_dir: &str) -> &'static str {
    match lang_dir {
        "java" => "java",
        "kotlin" => "kt",
        "python" => "py",
        "yaml" => "yaml",
        "json" => "json",
        "javascript" => "js",
        "typescript" => "ts",
        "tsx" => "tsx",
        "go" => "go",
        other => panic!("unknown fixture language directory `{other}`"),
    }
}

fn line_start(a: &Analysis, side: &str, idx: u32) -> usize {
    let (text, lines) = match side {
        "base" => (&a.base.text, &a.base.lines),
        "ours" => (&a.ours.text, &a.ours.lines),
        _ => (&a.theirs.text, &a.theirs.lines),
    };
    lines.get(idx as usize).map_or(text.len(), |l| l.start)
}

/// Applies proposals and non-conflicting chunks to the base. Returns the text and the
/// number of conflict chunks no proposal covered.
fn apply_all(a: &Analysis, proposals: &[Proposal]) -> (String, usize) {
    struct Edit {
        start: usize,
        end: usize,
        order: u32,
        text: String,
    }
    let mut edits: Vec<Edit> = Vec::new();
    let mut unresolved = 0;
    for p in proposals {
        edits.push(Edit {
            start: line_start(a, "base", p.base_range.start),
            end: line_start(a, "base", p.base_range.end),
            order: p.base_range.start,
            text: p.text.clone(),
        });
    }
    for c in &a.chunks {
        if proposals.iter().any(|p| p.chunk_ids.contains(&c.id)) {
            continue;
        }
        let (side, range) = match c.kind {
            ChunkKind::OursOnly | ChunkKind::BothSame => ("ours", &c.ours),
            ChunkKind::TheirsOnly => ("theirs", &c.theirs),
            ChunkKind::Conflict => {
                unresolved += 1;
                continue;
            }
        };
        let text = match side {
            "ours" => &a.ours.text,
            _ => &a.theirs.text,
        };
        let (s, e) = (
            line_start(a, side, range.start),
            line_start(a, side, range.end),
        );
        edits.push(Edit {
            start: line_start(a, "base", c.base.start),
            end: line_start(a, "base", c.base.end),
            order: c.base.start,
            text: text[s..e].to_string(),
        });
    }
    edits.sort_by_key(|e| (e.start, e.end, e.order));
    let mut out = String::new();
    let mut pos = 0;
    for e in &edits {
        assert!(e.start >= pos, "overlapping edits");
        out.push_str(&a.base.text[pos..e.start]);
        out.push_str(&e.text);
        pos = e.end;
    }
    out.push_str(&a.base.text[pos..]);
    (out, unresolved)
}

fn case_dirs() -> Vec<(String, String, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut cases = Vec::new();
    for lang in fs::read_dir(&root).unwrap().flatten() {
        if !lang.path().is_dir() {
            continue;
        }
        let lang_name = lang.file_name().to_string_lossy().to_string();
        for case in fs::read_dir(lang.path()).unwrap().flatten() {
            if case.path().join("base").exists() {
                cases.push((
                    lang_name.clone(),
                    case.file_name().to_string_lossy().to_string(),
                    case.path(),
                ));
            }
        }
    }
    cases.sort();
    cases
}

#[test]
fn fixtures() {
    let bless = std::env::var("BLESS").unwrap_or_default();
    let mut failures: Vec<String> = Vec::new();
    let mut count = 0;
    for (lang, name, dir) in case_dirs() {
        count += 1;
        let read = |n: &str| fs::read(dir.join(n)).unwrap();
        let (b, o, t) = (read("base"), read("ours"), read("theirs"));
        let a = analyze(
            MergeInput {
                base: &b,
                ours: &o,
                theirs: &t,
            },
            &Options::default(),
        )
        .unwrap();
        let path = format!("fixture.{}", ext_for(&lang));
        let outcome = propose(&path, &a).unwrap();
        let proposals = match &outcome {
            Outcome::Unsupported | Outcome::TimedOut => {
                failures.push(format!("{lang}/{name}: {outcome:?}"));
                continue;
            }
            Outcome::Proposals(p) => p.clone(),
        };
        let (result, unresolved) = apply_all(&a, &proposals);
        let expected_path = dir.join("expected");
        let unresolvable_path = dir.join("unresolvable");
        let has_marker = expected_path.exists() || unresolvable_path.exists();
        if bless == "all" || (!bless.is_empty() && !has_marker) {
            let _ = fs::remove_file(&expected_path);
            let _ = fs::remove_file(&unresolvable_path);
            let _ = fs::remove_file(dir.join("unresolved"));
            if proposals.is_empty() {
                fs::write(&unresolvable_path, "").unwrap();
                println!("=== {lang}/{name}: UNRESOLVABLE ({unresolved} conflicts)");
            } else if unresolved == 0 {
                fs::write(&expected_path, &result).unwrap();
                println!("=== {lang}/{name}: EXPECTED\n{result}");
            } else {
                fs::write(&expected_path, &result).unwrap();
                fs::write(dir.join("unresolved"), format!("{unresolved}\n")).unwrap();
                println!("=== {lang}/{name}: PARTIAL ({unresolved} conflicts left)\n{result}");
            }
            continue;
        }
        if expected_path.exists() {
            let want = fs::read_to_string(&expected_path).unwrap();
            let allowed: usize = fs::read_to_string(dir.join("unresolved"))
                .map(|s| s.trim().parse().unwrap())
                .unwrap_or(0);
            if unresolved != allowed {
                failures.push(format!(
                    "{lang}/{name}: {unresolved} conflict(s) left unresolved, expected {allowed}"
                ));
            } else if want != result {
                failures.push(format!(
                    "{lang}/{name}: result differs\n--- expected\n{want}\n--- actual\n{result}"
                ));
            }
        } else if unresolvable_path.exists() {
            if !proposals.is_empty() {
                failures.push(format!(
                    "{lang}/{name}: expected no proposals, got {}",
                    proposals.len()
                ));
            }
        } else {
            failures.push(format!("{lang}/{name}: no expected/unresolvable marker"));
        }
    }
    assert!(count > 0, "no fixtures found");
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n\n"));
}
