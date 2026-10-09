//! Exports engine `Analysis` JSON for UI tests (`apps/desktop/src/merge-editor/__fixtures__/`).
//!
//! By default the test verifies the committed JSON still matches the engine output.
//! Regenerate with `MERGEIQ_EXPORT_FIXTURES=1 cargo test -p mergeiq-core --test export_fixtures`.

use std::fs;
use std::path::{Path, PathBuf};

use mergeiq_core::{analyze, MergeInput, Options};

const CASES: &[&str] = &[
    "simple-conflict",
    "multi-conflict-file",
    "non-overlapping",
    "ours-only-insertion",
    "theirs-only-deletion",
    "identical-edit",
    "no-changes",
    "crlf-line-endings",
    "java-sample",
    "kotlin-sample",
    "python-sample",
];

/// Cases built inline: `(name, base, ours, theirs)`.
const INLINE: &[(&str, &str, &str, &str)] = &[
    (
        "mixed-changes",
        "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n",
        "l1\nOURS2\nl3\nl4\nOURS5\nl6\nl7\nl8\nl9\nl10\n",
        "l1\nl2\nl3\nl4\nTHEIRS5\nl6\nl7\nl8\nTHEIRS9\nl10\n",
    ),
    (
        "simple-resolvable",
        "a\nfoo(a, b)\nb\nbar(a, b)\nc\nk = 1\nd\n",
        "a\nfoo(x, b)\nb\nbar(x, b)\nc\nk = 2\nd\n",
        "a\nfoo(a, y)\nb\nbar(a, y)\nc\nk = 3\nd\n",
    ),
];

type Case = (String, Vec<u8>, Vec<u8>, Vec<u8>);

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/merge-editor/__fixtures__")
}

#[test]
fn exported_fixtures_match_engine_output() {
    let write = std::env::var_os("MERGEIQ_EXPORT_FIXTURES").is_some();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = out_dir();
    if write {
        fs::create_dir_all(&out).unwrap();
    }
    let mut cases: Vec<Case> = CASES
        .iter()
        .map(|case| {
            let dir = fixtures.join(case);
            (
                case.to_string(),
                fs::read(dir.join("base")).unwrap(),
                fs::read(dir.join("ours")).unwrap(),
                fs::read(dir.join("theirs")).unwrap(),
            )
        })
        .collect();
    for (name, b, o, t) in INLINE {
        cases.push((
            name.to_string(),
            b.as_bytes().to_vec(),
            o.as_bytes().to_vec(),
            t.as_bytes().to_vec(),
        ));
    }
    for (case, base, ours, theirs) in &cases {
        let analysis = analyze(MergeInput { base, ours, theirs }, &Options::default()).unwrap();
        let json = serde_json::to_string_pretty(&analysis).unwrap() + "\n";
        let path = out.join(format!("{case}.json"));
        if write {
            fs::write(&path, json).unwrap();
        } else {
            let existing = fs::read_to_string(&path).unwrap_or_else(|_| {
                panic!("missing {path:?}; regenerate with MERGEIQ_EXPORT_FIXTURES=1")
            });
            assert_eq!(existing, json, "{case}.json is stale; regenerate");
        }
    }
}
