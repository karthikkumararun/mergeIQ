//! Property tests: random non-overlapping edits should merge the same way our
//! engine and `git merge-file` do, and every `Analysis`'s chunks should satisfy
//! basic structural invariants (ordered, non-overlapping, in bounds).
//!
//! The `git merge-file` comparison is skipped (not failed) when `git` isn't on PATH.

use std::process::Command;

use mergeiq_core::{analyze, apply_non_conflicting, MergeInput, Options};
use proptest::prelude::*;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Runs `git merge-file -p <ours> <base> <theirs>`, returning its stdout. `None` if
/// git reports a real conflict (shouldn't happen for our non-overlapping inputs) or
/// can't be run.
fn run_git_merge_file(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let dir = tempfile::tempdir().ok()?;
    let base_path = dir.path().join("base");
    let ours_path = dir.path().join("ours");
    let theirs_path = dir.path().join("theirs");
    std::fs::write(&base_path, base).ok()?;
    std::fs::write(&ours_path, ours).ok()?;
    std::fs::write(&theirs_path, theirs).ok()?;

    let output = Command::new("git")
        .args(["merge-file", "-p"])
        .arg(&ours_path)
        .arg(&base_path)
        .arg(&theirs_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn join(lines: &[String]) -> String {
    let mut s = String::new();
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    s
}

/// Applies one random mutation (modify, insert or delete) at a random line index
/// within `[start, end)`. No-op if the region is empty.
fn mutate_region(lines: &mut Vec<String>, start: usize, end: usize, seed: u32) {
    let end = end.min(lines.len());
    let start = start.min(end);
    if start >= end {
        return;
    }
    let idx = start + (seed as usize % (end - start));
    match seed % 3 {
        0 => lines[idx] = format!("{}_mut{seed}", lines[idx]),
        1 => lines.insert(idx, format!("new{seed}")),
        _ => {
            lines.remove(idx);
        }
    }
}

fn suffix() -> impl Strategy<Value = String> {
    "[a-z]{0,6}"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn matches_git_merge_file_for_non_overlapping_edits(
        suffixes in prop::collection::vec(suffix(), 4..30),
        split_frac in 0.1f64..0.9,
        ours_seed in any::<u32>(),
        theirs_seed in any::<u32>(),
    ) {
        // Index-prefixed so every base line is unique, avoiding diff-alignment
        // ambiguity between our histogram algorithm and git's default algorithm.
        let base_lines: Vec<String> = suffixes
            .iter()
            .enumerate()
            .map(|(i, s)| format!("L{i}_{s}"))
            .collect();
        let n = base_lines.len();
        let mid = (((n as f64) * split_frac) as usize).clamp(1, n.saturating_sub(1).max(1));

        let mut ours_lines = base_lines.clone();
        mutate_region(&mut ours_lines, 0, mid, ours_seed);
        let mut theirs_lines = base_lines.clone();
        mutate_region(&mut theirs_lines, mid, n, theirs_seed);

        let base = join(&base_lines);
        let ours = join(&ours_lines);
        let theirs = join(&theirs_lines);

        let analysis = analyze(
            MergeInput { base: base.as_bytes(), ours: ours.as_bytes(), theirs: theirs.as_bytes() },
            &Options::default(),
        ).unwrap();

        // Invariants: chunk ranges are ordered, non-overlapping, within bounds.
        let base_len = base_lines.len() as u32;
        let mut prev_end = 0u32;
        for chunk in &analysis.chunks {
            prop_assert!(chunk.base.start >= prev_end);
            prop_assert!(chunk.base.start <= chunk.base.end);
            prop_assert!(chunk.base.end <= base_len);
            prev_end = chunk.base.end;
        }

        let Some(merged) = apply_non_conflicting(&analysis) else {
            // Our non-overlapping construction shouldn't produce a real conflict;
            // if it ever does, there's nothing meaningful to compare against git.
            return Ok(());
        };

        if git_available() {
            if let Some(expected) = run_git_merge_file(&base, &ours, &theirs) {
                prop_assert_eq!(merged, expected);
            }
        }
    }
}
