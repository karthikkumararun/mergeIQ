//! Golden fixture tests: every case under `tests/fixtures/<case>/{base,ours,theirs}`
//! is analyzed with default options and the chunk classification is snapshotted with
//! `insta`. Run `cargo insta review` (or `INSTA_UPDATE=always cargo test -p
//! mergeiq-core --test golden`) after intentionally changing merge behavior to accept
//! new snapshots.

use std::fs;
use std::path::Path;

use mergeiq_core::{analyze, MergeInput, Options};
use serde::Serialize;

#[derive(Serialize)]
struct ChunkSummary {
    kind: String,
    base: (u32, u32),
    ours: (u32, u32),
    theirs: (u32, u32),
}

#[derive(Serialize)]
struct GoldenSummary {
    dominant_eol: String,
    encoding: String,
    chunk_count: usize,
    chunks: Vec<ChunkSummary>,
}

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn golden_fixtures() {
    let dir = fixtures_dir();
    let mut cases: Vec<String> = fs::read_dir(&dir)
        .expect("tests/fixtures must exist")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                entry.file_name().into_string().ok()
            } else {
                None
            }
        })
        .collect();
    cases.sort();
    assert!(
        cases.len() >= 20,
        "expected at least 20 golden fixture cases, found {}",
        cases.len()
    );

    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(dir.join("..").join("snapshots"));
    settings.bind(|| {
        for case in &cases {
            let case_dir = dir.join(case);
            let base = fs::read(case_dir.join("base")).unwrap();
            let ours = fs::read(case_dir.join("ours")).unwrap();
            let theirs = fs::read(case_dir.join("theirs")).unwrap();

            let analysis = analyze(
                MergeInput {
                    base: &base,
                    ours: &ours,
                    theirs: &theirs,
                },
                &Options::default(),
            )
            .unwrap_or_else(|e| panic!("case {case}: analyze failed: {e}"));

            let summary = GoldenSummary {
                dominant_eol: format!("{:?}", analysis.dominant_eol),
                encoding: format!("{:?}", analysis.encoding.encoding),
                chunk_count: analysis.chunks.len(),
                chunks: analysis
                    .chunks
                    .iter()
                    .map(|c| ChunkSummary {
                        kind: format!("{:?}", c.kind),
                        base: (c.base.start, c.base.end),
                        ours: (c.ours.start, c.ours.end),
                        theirs: (c.theirs.start, c.theirs.end),
                    })
                    .collect(),
            };

            insta::assert_json_snapshot!(case.as_str(), summary);
        }
    });
}
