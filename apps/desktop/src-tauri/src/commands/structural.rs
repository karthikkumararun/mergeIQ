//! IPC for syntax-aware conflict resolution (`structural-merge`).

use std::time::{Duration, Instant};

use mergeiq_core::Analysis;
use mergeiq_struct::{propose_until, Outcome};

use crate::ipc::IpcError;

/// Proposals are computed in the background; past this the editor shows nothing.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// The answer to [`structural_resolve`].
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct StructuralResolve {
    /// Proposals, or why there are none.
    pub outcome: Outcome,
    /// Wall-clock time spent computing, in milliseconds.
    pub elapsed_ms: u32,
}

/// Computes structural proposals for `analysis` of the file at `path` (blocking).
pub fn resolve_blocking(
    path: &str,
    analysis: &Analysis,
    timeout: Duration,
) -> Result<StructuralResolve, IpcError> {
    let started = Instant::now();
    let outcome = propose_until(path, analysis, Some(started + timeout))
        .map_err(|e| IpcError::Request(e.to_string()))?;
    let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
    if outcome == Outcome::TimedOut {
        tracing::warn!(path, elapsed_ms, "structural analysis timed out");
    }
    Ok(StructuralResolve {
        outcome,
        elapsed_ms,
    })
}

/// Syntax-aware resolution proposals for the conflict chunks of `analysis`. Runs on the
/// blocking pool; never blocks the UI thread.
#[tauri::command]
#[specta::specta]
pub async fn structural_resolve(
    path: String,
    analysis: Analysis,
) -> Result<StructuralResolve, IpcError> {
    tauri::async_runtime::spawn_blocking(move || resolve_blocking(&path, &analysis, TIMEOUT))
        .await
        .map_err(|e| IpcError::Request(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use mergeiq_core::{analyze, MergeInput, Options};

    fn analysis() -> Analysis {
        let base = "{\n  \"a\": 1\n}\n";
        let ours = "{\n  \"a\": 1,\n  \"o\": 2\n}\n";
        let theirs = "{\n  \"a\": 1,\n  \"t\": 3\n}\n";
        analyze(
            MergeInput {
                base: base.as_bytes(),
                ours: ours.as_bytes(),
                theirs: theirs.as_bytes(),
            },
            &Options::default(),
        )
        .unwrap()
    }

    #[test]
    fn resolves_a_json_conflict() {
        let r = resolve_blocking("package.json", &analysis(), TIMEOUT).unwrap();
        let Outcome::Proposals(p) = r.outcome else {
            panic!("expected proposals");
        };
        assert_eq!(p.len(), 1);
        assert!(p[0].text.contains("\"o\": 2") && p[0].text.contains("\"t\": 3"));
    }

    /// The editor sends back the `Analysis` it received, as JSON. The committed UI fixtures are
    /// real serialised analyses, so they pin the wire format the command must accept.
    #[test]
    fn accepts_the_analysis_json_the_ui_sends_back() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/merge-editor/__fixtures__/structural/structural-ts.json");
        let fixture: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let analysis: Analysis = serde_json::from_value(fixture["analysis"].clone()).unwrap();
        let r = resolve_blocking("src/options.ts", &analysis, TIMEOUT).unwrap();
        let Outcome::Proposals(p) = r.outcome else {
            panic!("expected proposals");
        };
        assert_eq!(p.len(), 3, "3 of the 4 conflicts have proposals");
        // The result matches what the mocked backend replays in the UI tests.
        let expected = serde_json::to_value(&fixture["resolve"]["outcome"]["Proposals"]).unwrap();
        assert_eq!(serde_json::to_value(&p).unwrap(), expected);
    }

    #[test]
    fn unsupported_files_are_reported() {
        let r = resolve_blocking("README.md", &analysis(), TIMEOUT).unwrap();
        assert_eq!(r.outcome, Outcome::Unsupported);
    }

    #[test]
    fn a_zero_timeout_reports_a_timeout() {
        let r = resolve_blocking("package.json", &analysis(), Duration::ZERO).unwrap();
        assert_eq!(r.outcome, Outcome::TimedOut);
    }
}
