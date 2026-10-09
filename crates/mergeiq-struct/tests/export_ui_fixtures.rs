//! Exports real engine output for the desktop UI's mocked backend
//! (`apps/desktop/src/merge-editor/__fixtures__/structural/`).
//!
//! By default the test verifies the committed JSON still matches the engine. Regenerate with
//! `MERGEIQ_EXPORT_FIXTURES=1 cargo test -p mergeiq-struct --test export_ui_fixtures`.

use std::fs;
use std::path::{Path, PathBuf};

use mergeiq_core::{analyze, MergeInput, Options};
use mergeiq_struct::propose;

struct Case {
    name: &'static str,
    path: &'static str,
    base: &'static str,
    ours: &'static str,
    theirs: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "structural-package-json",
        path: "package.json",
        base: r#"{
  "name": "shop-web",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "vite build"
  },
  "dependencies": {
    "react": "^19.1.0",
    "react-dom": "^19.1.0"
  },
  "packageManager": "pnpm@9.12.0"
}
"#,
        ours: r#"{
  "name": "shop-web",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "lint": "eslint ."
  },
  "dependencies": {
    "react": "^19.1.0",
    "react-dom": "^19.1.0",
    "zustand": "^5.0.0"
  },
  "packageManager": "pnpm@9.15.0"
}
"#,
        theirs: r#"{
  "name": "shop-web",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "test": "vitest"
  },
  "dependencies": {
    "react": "^19.1.0",
    "react-dom": "^19.1.0",
    "zod": "^4.0.0"
  },
  "packageManager": "pnpm@10.0.0"
}
"#,
    },
    Case {
        name: "structural-ts",
        path: "src/options.ts",
        base: r#"import { a } from "./a";

export interface Options {
  name: string;
}

export enum Mode {
  Fast,
  Safe
}

export function run(): number {
  return 1;
}
"#,
        ours: r#"import { a, c } from "./a";

export interface Options {
  name: string;
  verbose: boolean;
}

export enum Mode {
  Fast,
  Safe,
  Auto
}

export function run(): number {
  return 2;
}
"#,
        theirs: r#"import { a, b } from "./a";

export interface Options {
  name: string;
  retries: number;
}

export enum Mode {
  Fast,
  Safe,
  Debug
}

export function run(): number {
  return 3;
}
"#,
    },
];

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/merge-editor/__fixtures__/structural")
}

#[test]
fn exported_ui_fixtures_match_engine_output() {
    let write = std::env::var_os("MERGEIQ_EXPORT_FIXTURES").is_some();
    let out = out_dir();
    if write {
        fs::create_dir_all(&out).unwrap();
    }
    for case in CASES {
        let analysis = analyze(
            MergeInput {
                base: case.base.as_bytes(),
                ours: case.ours.as_bytes(),
                theirs: case.theirs.as_bytes(),
            },
            &Options::default(),
        )
        .unwrap();
        let outcome = propose(case.path, &analysis).unwrap();
        let value = serde_json::json!({
            "path": case.path,
            "analysis": analysis,
            // `elapsed_ms` is fixed so the committed file is deterministic.
            "resolve": { "outcome": outcome, "elapsed_ms": 12 },
        });
        let json = serde_json::to_string_pretty(&value).unwrap() + "\n";
        let file = out.join(format!("{}.json", case.name));
        if write {
            fs::write(&file, json).unwrap();
        } else {
            let committed = fs::read_to_string(&file)
                .unwrap_or_else(|_| panic!("missing {file:?}; run with MERGEIQ_EXPORT_FIXTURES=1"));
            assert_eq!(committed, json, "{file:?} is stale; re-export it");
        }
    }
}
