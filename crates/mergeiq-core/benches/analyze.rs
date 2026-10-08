use criterion::{criterion_group, criterion_main, Criterion};
use mergeiq_core::{analyze, MergeInput, Options};

/// Builds a base of `n` unique lines plus ours/theirs variants with `num_chunks`
/// non-conflicting single-line edits scattered evenly (alternating sides), matching
/// the spec's performance scenario shape ("10,000-line inputs with 50 chunks").
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

fn bench_analyze(c: &mut Criterion) {
    let (base10k, ours10k, theirs10k) = make_fixture(10_000, 50);
    c.bench_function("analyze_10k_lines_50_chunks", |b| {
        b.iter(|| {
            analyze(
                MergeInput {
                    base: base10k.as_bytes(),
                    ours: ours10k.as_bytes(),
                    theirs: theirs10k.as_bytes(),
                },
                &Options::default(),
            )
            .unwrap()
        })
    });

    let (base100k, ours100k, theirs100k) = make_fixture(100_000, 50);
    c.bench_function("analyze_100k_lines_50_chunks", |b| {
        b.iter(|| {
            analyze(
                MergeInput {
                    base: base100k.as_bytes(),
                    ours: ours100k.as_bytes(),
                    theirs: theirs100k.as_bytes(),
                },
                &Options::default(),
            )
            .unwrap()
        })
    });
}

criterion_group!(benches, bench_analyze);
criterion_main!(benches);
