//! Lockfile helpers: `go.sum` union merging and regenerating lockfiles with the project's
//! own tooling.
//!
//! Commands never run through a shell (arguments are split with `shell-words`), only after the
//! UI has shown the exact command and working directory and the user confirmed, output is
//! streamed, and the lockfile is staged only when the command exits 0.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::classify::LockfileKind;
use crate::error::{GitError, Result};
use crate::paths::PathToken;
use crate::repo::Repo;
use crate::write::AcceptSide;

/// The command that regenerates a lockfile, when there is one.
pub fn default_command(kind: LockfileKind) -> Option<&'static str> {
    Some(match kind {
        LockfileKind::Npm => "npm install --package-lock-only",
        LockfileKind::Pnpm => "pnpm install --lockfile-only",
        LockfileKind::Yarn => "yarn install --mode update-lockfile",
        LockfileKind::Poetry => "poetry lock --no-update",
        LockfileKind::Cargo => "cargo update --workspace",
        LockfileKind::Gradle => "./gradlew dependencies --write-locks",
        LockfileKind::GoSum => return None,
    })
}

// ---- go.sum ----

/// How a line of the merged `go.sum` came about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum GoSumMark {
    /// Present in the result and not changed by either side.
    Unchanged,
    /// Added by the left side only.
    LeftAdded,
    /// Added by the right side only.
    RightAdded,
    /// Dropped: one side removed it and the other left it unchanged.
    Removed,
}

/// One line of the `go.sum` merge preview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct GoSumLine {
    /// The line without its terminator.
    pub text: String,
    /// What happened to it.
    pub mark: GoSumMark,
}

/// The union merge of three `go.sum` versions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct GoSumMerge {
    /// Result lines (sorted) followed by the removed ones, each marked.
    pub lines: Vec<GoSumLine>,
    /// The merged file content.
    pub result: String,
}

/// A parsed `vMAJOR.MINOR.PATCH[-pre][+build]`.
struct Semver {
    nums: [u64; 3],
    pre: Option<String>,
}

fn parse_semver(v: &str) -> Option<Semver> {
    let v = v.strip_prefix('v')?;
    let v = v.split('+').next()?;
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p.to_string())),
        None => (v, None),
    };
    let mut parts = core.split('.');
    let mut nums = [0u64; 3];
    for n in &mut nums {
        *n = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(Semver { nums, pre })
}

fn cmp_pre(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.split('.'), b.split('.'));
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

fn cmp_version(a: &str, b: &str) -> Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(x), Some(y)) => x.nums.cmp(&y.nums).then_with(|| match (&x.pre, &y.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(p), Some(q)) => cmp_pre(p, q),
        }),
        _ => a.cmp(b),
    }
}

/// Orders `go.sum` lines like `go` does: by module path, then version, with a module's
/// `/go.mod` hash after its source hash.
fn cmp_sum_line(a: &str, b: &str) -> Ordering {
    let key = |l: &str| {
        let mut f = l.split_whitespace();
        let module = f.next().unwrap_or("").to_string();
        let version = f.next().unwrap_or("").to_string();
        let (v, gomod) = match version.strip_suffix("/go.mod") {
            Some(v) => (v.to_string(), true),
            None => (version, false),
        };
        (module, v, gomod)
    };
    let (ka, kb) = (key(a), key(b));
    ka.0.cmp(&kb.0)
        .then_with(|| cmp_version(&ka.1, &kb.1))
        .then(ka.2.cmp(&kb.2))
        .then_with(|| a.cmp(b))
}

/// Sorted, deduplicated union of both sides' lines, minus lines one side removed (and the
/// other left unchanged).
pub fn merge_go_sum(base: &str, ours: &str, theirs: &str) -> GoSumMerge {
    let set = |t: &str| -> BTreeSet<String> {
        t.lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    };
    let (b, o, t) = (set(base), set(ours), set(theirs));
    let mut result: Vec<GoSumLine> = Vec::new();
    let mut removed: Vec<GoSumLine> = Vec::new();
    for line in o.union(&t) {
        let in_base = b.contains(line);
        let (in_o, in_t) = (o.contains(line), t.contains(line));
        if in_base && !(in_o && in_t) {
            // Cannot happen for union members (present in at least one side) unless the
            // other side removed it.
            removed.push(GoSumLine {
                text: line.clone(),
                mark: GoSumMark::Removed,
            });
            continue;
        }
        let mark = match (in_base, in_o, in_t) {
            (false, true, false) => GoSumMark::LeftAdded,
            (false, false, true) => GoSumMark::RightAdded,
            _ => GoSumMark::Unchanged,
        };
        result.push(GoSumLine {
            text: line.clone(),
            mark,
        });
    }
    result.sort_by(|a, b| cmp_sum_line(&a.text, &b.text));
    removed.sort_by(|a, b| cmp_sum_line(&a.text, &b.text));
    let text: String = result.iter().map(|l| format!("{}\n", l.text)).collect();
    result.extend(removed);
    GoSumMerge {
        lines: result,
        result: text,
    }
}

// ---- running commands ----

/// Output stream a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// How a command run ended.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// The exit code (`None` if killed by a signal or cancelled).
    pub exit_code: Option<i32>,
    /// The user cancelled the run.
    pub cancelled: bool,
    /// Wall-clock duration in milliseconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub duration_ms: u64,
}

impl RunResult {
    /// Whether the command exited with status 0.
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.cancelled
    }
}

/// Splits `command` into arguments without involving a shell.
pub fn split_command(command: &str) -> Result<Vec<String>> {
    let parts = shell_words::split(command).map_err(|e| GitError::Unsupported {
        what: format!("the command is not valid: {e}"),
    })?;
    if parts.is_empty() {
        return Err(GitError::Unsupported {
            what: "the command is empty".into(),
        });
    }
    Ok(parts)
}

/// Finds the program of `command` (searching `PATH`, or `cwd` for `./tool`).
fn resolve_program(argv: &[String], cwd: &Path) -> Result<std::path::PathBuf> {
    which::which_in(&argv[0], std::env::var_os("PATH"), cwd).map_err(|_| GitError::Unsupported {
        what: format!("`{}` was not found", argv[0]),
    })
}

/// Checks that `command` is valid and its program exists, without running anything.
pub fn check_command(command: &str, cwd: &Path) -> Result<()> {
    resolve_program(&split_command(command)?, cwd).map(|_| ())
}

/// Runs `command` in `cwd`, streaming output lines to `on_output`, until it exits or `cancel`
/// is set (the process is then killed).
pub fn run_command(
    command: &str,
    cwd: &Path,
    cancel: &AtomicBool,
    mut on_output: impl FnMut(OutputStream, &str),
) -> Result<RunResult> {
    let argv = split_command(command)?;
    let program = resolve_program(&argv, cwd)?;
    let mut cmd = std::process::Command::new(program);
    cmd.args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let started = Instant::now();
    let mut child = cmd.spawn()?;
    let (tx, rx) = mpsc::channel::<(OutputStream, String)>();
    let mut readers = Vec::new();
    for (stream, pipe) in [
        (
            OutputStream::Stdout,
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        ),
        (
            OutputStream::Stderr,
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        ),
    ] {
        let Some(mut pipe) = pipe else { continue };
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let mut pending: Vec<u8> = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                pending.extend_from_slice(&buf[..n]);
                while let Some(nl) = pending.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = pending.drain(..=nl).collect();
                    let text = String::from_utf8_lossy(&line[..line.len() - 1]);
                    let _ = tx.send((stream, text.trim_end_matches('\r').to_string()));
                }
            }
            if !pending.is_empty() {
                let _ = tx.send((stream, String::from_utf8_lossy(&pending).into_owned()));
            }
        }));
    }
    drop(tx);

    let mut cancelled = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((stream, line)) => on_output(stream, &line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if cancel.load(AtomicOrdering::Relaxed) && !cancelled {
            cancelled = true;
            let _ = child.kill();
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait()?;
    Ok(RunResult {
        exit_code: if cancelled { None } else { status.code() },
        cancelled,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// The outcome of "take a side and regenerate".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RegenerateResult {
    /// How the command ended.
    pub run: RunResult,
    /// The lockfile was staged (only when the command exited 0).
    pub staged: bool,
}

impl Repo {
    fn stage_text(&self, token: &PathToken, stage: u8) -> Result<String> {
        let path = token.decode()?;
        let entry = self
            .conflict_entry(&path)?
            .ok_or_else(|| GitError::NoSuchConflict {
                path: path.display(),
            })?;
        Ok(match entry.stage(stage) {
            Some(s) => {
                String::from_utf8_lossy(&self.git(["cat-file", "blob", &s.oid])?).into_owned()
            }
            None => String::new(),
        })
    }

    /// The `go.sum` union merge of a conflicted file (for the preview).
    pub fn go_sum_preview(&self, token: &PathToken) -> Result<GoSumMerge> {
        Ok(merge_go_sum(
            &self.stage_text(token, 1)?,
            &self.stage_text(token, 2)?,
            &self.stage_text(token, 3)?,
        ))
    }

    /// Writes the `go.sum` union merge and stages it.
    pub fn go_sum_union(&self, token: &PathToken) -> Result<()> {
        let merged = self.go_sum_preview(token)?;
        self.save_resolved(&token.decode()?, merged.result.as_bytes())
    }

    /// Takes `side`'s lockfile exactly, runs `command` in the lockfile's directory and stages
    /// the file only if it exits 0. A cancelled or failed run leaves the file unstaged and the
    /// conflict listed.
    pub fn regenerate_lockfile(
        &self,
        token: &PathToken,
        side: AcceptSide,
        command: &str,
        cancel: &AtomicBool,
        on_output: impl FnMut(OutputStream, &str),
    ) -> Result<RegenerateResult> {
        let path = token.decode()?;
        let file = path.in_root(&self.root)?;
        let dir = file.parent().unwrap_or(&self.root).to_path_buf();
        // A bad command must not change anything on disk.
        check_command(command, &dir)?;
        self.write_side_unstaged(&path, side)?;
        let run = run_command(command, &dir, cancel, on_output)?;
        self.note_mutation();
        let staged = if run.success() {
            self.git(["add".into(), "--".into(), path.to_os_string()?])
                .is_ok()
        } else {
            false
        };
        self.note_mutation();
        Ok(RegenerateResult { run, staged })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(m: &GoSumMerge, mark: GoSumMark) -> Vec<&str> {
        m.lines
            .iter()
            .filter(|l| l.mark == mark)
            .map(|l| l.text.as_str())
            .collect()
    }

    const A: &str = "github.com/a/a v1.0.0 h1:aaa=";
    const A_MOD: &str = "github.com/a/a v1.0.0/go.mod h1:aaaa=";
    const B: &str = "github.com/b/b v1.2.0 h1:bbb=";
    const C: &str = "github.com/c/c v0.1.0 h1:ccc=";

    #[test]
    fn both_sides_add_different_checksums() {
        let m = merge_go_sum(
            &format!("{A}\n"),
            &format!("{A}\n{B}\n"),
            &format!("{A}\n{C}\n"),
        );
        assert_eq!(m.result, format!("{A}\n{B}\n{C}\n"));
        assert_eq!(lines(&m, GoSumMark::LeftAdded), [B]);
        assert_eq!(lines(&m, GoSumMark::RightAdded), [C]);
        assert_eq!(lines(&m, GoSumMark::Unchanged), [A]);
    }

    #[test]
    fn lines_added_by_both_are_kept_once() {
        let m = merge_go_sum(A, &format!("{A}\n{B}\n"), &format!("{B}\n{A}\n"));
        assert_eq!(m.result, format!("{A}\n{B}\n"));
        assert_eq!(lines(&m, GoSumMark::Unchanged), [A, B]);
    }

    #[test]
    fn removal_by_one_side_wins_over_an_unchanged_other_side() {
        let m = merge_go_sum(
            &format!("{A}\n{B}\n"),
            &format!("{A}\n"),
            &format!("{A}\n{B}\n"),
        );
        assert_eq!(m.result, format!("{A}\n"));
        assert_eq!(lines(&m, GoSumMark::Removed), [B]);
        // Symmetric.
        let m = merge_go_sum(
            &format!("{A}\n{B}\n"),
            &format!("{A}\n{B}\n"),
            &format!("{B}\n"),
        );
        assert_eq!(m.result, format!("{B}\n"));
        assert_eq!(lines(&m, GoSumMark::Removed), [A]);
    }

    #[test]
    fn removal_by_both_sides_is_just_gone() {
        let m = merge_go_sum(&format!("{A}\n{B}\n"), &format!("{A}\n"), &format!("{A}\n"));
        assert_eq!(m.result, format!("{A}\n"));
        assert!(lines(&m, GoSumMark::Removed).is_empty());
    }

    #[test]
    fn sorted_like_go_with_semver_and_go_mod_variants() {
        let m = merge_go_sum(
            "",
            &format!(
                "{A_MOD}\n{A}\nx.io/m v1.10.0 h1:z=\nx.io/m v1.9.0 h1:y=\nx.io/m v1.9.0-rc.1 h1:r=\n"
            ),
            "",
        );
        assert_eq!(
            m.result,
            format!(
                "{A}\n{A_MOD}\nx.io/m v1.9.0-rc.1 h1:r=\nx.io/m v1.9.0 h1:y=\nx.io/m v1.10.0 h1:z=\n"
            )
        );
    }

    #[test]
    fn crlf_and_blank_lines_are_normalised() {
        let m = merge_go_sum("", &format!("{A}\r\n\r\n"), &format!("{B}\r\n"));
        assert_eq!(m.result, format!("{A}\n{B}\n"));
    }

    #[test]
    fn default_commands_match_the_spec() {
        assert_eq!(
            default_command(LockfileKind::Npm),
            Some("npm install --package-lock-only")
        );
        assert_eq!(
            default_command(LockfileKind::Pnpm),
            Some("pnpm install --lockfile-only")
        );
        assert_eq!(
            default_command(LockfileKind::Yarn),
            Some("yarn install --mode update-lockfile")
        );
        assert_eq!(
            default_command(LockfileKind::Poetry),
            Some("poetry lock --no-update")
        );
        assert_eq!(
            default_command(LockfileKind::Cargo),
            Some("cargo update --workspace")
        );
        assert_eq!(
            default_command(LockfileKind::Gradle),
            Some("./gradlew dependencies --write-locks")
        );
        assert_eq!(default_command(LockfileKind::GoSum), None);
    }

    #[test]
    fn commands_split_without_a_shell() {
        assert_eq!(
            split_command("npm install --package-lock-only").unwrap(),
            ["npm", "install", "--package-lock-only"]
        );
        assert_eq!(
            split_command(r#"tool "a b" 'c d'"#).unwrap(),
            ["tool", "a b", "c d"]
        );
        // Shell syntax is just text: nothing is interpreted.
        assert_eq!(
            split_command("echo a && rm -rf /").unwrap(),
            ["echo", "a", "&&", "rm", "-rf", "/"]
        );
        assert!(split_command("").is_err());
        assert!(split_command("unterminated 'quote").is_err());
    }
}
