//! CI-friendly smoke test for the performance requirement (see `benches/analyze.rs`
//! for the full criterion benchmarks used during local performance work).
//!
//! Debug builds (what `cargo test`/CI normally run) are much slower than release for
//! CPU-bound Rust code, so the spec's exact release-build budgets (100ms / 1s) are
//! only enforced in release builds; debug builds use a generously scaled-up budget
//! that still catches gross regressions (e.g. an accidental O(n^2) path).

use std::time::{Duration, Instant};

use mergeiq_core::{analyze, MergeInput, Options};

fn make_fixture(n: usize, num_chunks: usize) -> (String, String, String) {
    let base_lines: Vec<String> = (0..n).map(|i| format!("line_{i}_payload_value")).collect();
    let mut ours_lines = base_lines.clone();
    let mut theirs_lines = base_lines.clone();
    let step = (n / num_chunks).max(2);
    for k in 0..num_chunks {
        let idx = (k * step + step / 2).min(n - 1);
        if k % 2 == 0 {
            ours_lines[idx] = format!("{}_OURS", base_lines[idx]);
        } else {
            theirs_lines[idx] = format!("{}_THEIRS", base_lines[idx]);
        }
    }
    let join = |v: &[String]| -> String { v.iter().map(|s| format!("{s}\n")).collect() };
    (join(&base_lines), join(&ours_lines), join(&theirs_lines))
}

fn budget(release_budget: Duration) -> Duration {
    if cfg!(debug_assertions) {
        release_budget * 20
    } else {
        release_budget
    }
}

#[test]
fn ten_thousand_lines_within_budget() {
    let (base, ours, theirs) = make_fixture(10_000, 50);
    let start = Instant::now();
    let analysis = analyze(
        MergeInput {
            base: base.as_bytes(),
            ours: ours.as_bytes(),
            theirs: theirs.as_bytes(),
        },
        &Options::default(),
    )
    .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(analysis.chunks.len(), 50);
    let max = budget(Duration::from_millis(100));
    assert!(
        elapsed < max,
        "analyze(10k lines) took {elapsed:?}, budget {max:?}"
    );
}

#[test]
fn hundred_thousand_lines_within_budget() {
    let (base, ours, theirs) = make_fixture(100_000, 50);
    let start = Instant::now();
    let analysis = analyze(
        MergeInput {
            base: base.as_bytes(),
            ours: ours.as_bytes(),
            theirs: theirs.as_bytes(),
        },
        &Options::default(),
    )
    .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(analysis.chunks.len(), 50);
    let max = budget(Duration::from_secs(1));
    assert!(
        elapsed < max,
        "analyze(100k lines) took {elapsed:?}, budget {max:?}"
    );
}
