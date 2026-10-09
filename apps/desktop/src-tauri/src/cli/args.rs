//! Command-line parsing (clap). `parse` never exits the process itself.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand};
use serde::{Deserialize, Serialize};

/// Exit code for invalid arguments or requests that cannot be served.
pub const EXIT_USAGE: i32 = 2;

#[derive(Parser)]
#[command(
    name = "mergeiq",
    version,
    about = "MergeIQ — three-way merge editor",
    disable_help_subcommand = true,
    arg_required_else_help = false
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// git mergetool mode: merge LOCAL and REMOTE against BASE and write MERGED.
    ///
    /// Exits 0 when resolved, 1 when cancelled or saved with conflict markers.
    Merge {
        base: PathBuf,
        local: PathBuf,
        remote: PathBuf,
        merged: PathBuf,
        /// Accepted for compatibility; the command always waits for the window to close.
        #[arg(long)]
        wait: bool,
    },
    /// Resolve one conflicted file (index stages) or a file containing conflict markers.
    Resolve { path: PathBuf },
    /// Open a repository window.
    Open {
        /// Defaults to the current directory.
        dir: Option<PathBuf>,
    },
}

/// What a CLI invocation asks the running (or new) instance to do. Paths are absolute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RequestKind {
    /// `mergeiq merge BASE LOCAL REMOTE MERGED`.
    Merge {
        /// Common ancestor (may not exist or be empty).
        base: PathBuf,
        /// Left side ("ours").
        local: PathBuf,
        /// Right side ("theirs").
        remote: PathBuf,
        /// File the result is written to.
        merged: PathBuf,
    },
    /// `mergeiq resolve PATH`.
    Resolve {
        /// The file to resolve.
        path: PathBuf,
    },
    /// `mergeiq open [DIR]`.
    Open {
        /// The directory to open.
        dir: PathBuf,
    },
}

/// A parsed request plus the caller's working directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// What to do.
    #[serde(flatten)]
    pub kind: RequestKind,
    /// The invoking process's working directory.
    pub cwd: PathBuf,
}

/// Result of parsing the process arguments.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    /// No CLI arguments: start the normal app.
    Gui,
    /// A request for the (new or running) instance.
    Request(Request),
    /// Print text and exit (`--help`, `--version`, usage errors).
    Exit {
        /// Process exit code.
        code: i32,
        /// Text to print.
        text: String,
        /// Print to stderr (errors) rather than stdout.
        to_stderr: bool,
    },
}

fn absolute(path: PathBuf, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

/// Parses `args` (including `argv[0]`). With no arguments (or a legacy macOS `-psn_`
/// argument) the normal GUI starts.
pub fn parse<I, T>(args: I, cwd: &Path) -> Parsed
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let gui = args.len() <= 1
        || args
            .get(1)
            .is_some_and(|a| a.to_string_lossy().starts_with("-psn_"));
    if gui {
        return Parsed::Gui;
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(err) => {
            let code = if err.use_stderr() { EXIT_USAGE } else { 0 };
            return Parsed::Exit {
                code,
                text: err.render().to_string(),
                to_stderr: err.use_stderr(),
            };
        }
    };
    let abs = |p: PathBuf| absolute(p, cwd);
    let kind = match cli.command {
        Command::Merge {
            base,
            local,
            remote,
            merged,
            wait: _,
        } => RequestKind::Merge {
            base: abs(base),
            local: abs(local),
            remote: abs(remote),
            merged: abs(merged),
        },
        Command::Resolve { path } => RequestKind::Resolve { path: abs(path) },
        Command::Open { dir } => RequestKind::Open {
            dir: abs(dir.unwrap_or_else(|| PathBuf::from("."))),
        },
    };
    Parsed::Request(Request {
        kind,
        cwd: cwd.to_path_buf(),
    })
}

/// The usage text for `mergeiq --help` (used by docs and tests).
pub fn usage() -> String {
    Cli::command().render_long_help().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Parsed {
        parse(args.iter().copied(), Path::new("/work"))
    }

    #[test]
    fn no_arguments_starts_the_gui() {
        assert_eq!(p(&["mergeiq"]), Parsed::Gui);
        assert_eq!(p(&["mergeiq", "-psn_0_1234"]), Parsed::Gui);
    }

    #[test]
    fn merge_command_with_relative_paths_is_made_absolute() {
        let parsed = p(&["mergeiq", "merge", "b", "/tmp/l", "r", "m", "--wait"]);
        let Parsed::Request(req) = parsed else {
            panic!("{parsed:?}")
        };
        assert_eq!(
            req.kind,
            RequestKind::Merge {
                base: "/work/b".into(),
                local: "/tmp/l".into(),
                remote: "/work/r".into(),
                merged: "/work/m".into(),
            }
        );
        assert_eq!(req.cwd, Path::new("/work"));
    }

    #[test]
    fn resolve_and_open_commands() {
        let Parsed::Request(req) = p(&["mergeiq", "resolve", "a.txt"]) else {
            panic!()
        };
        assert_eq!(
            req.kind,
            RequestKind::Resolve {
                path: "/work/a.txt".into()
            }
        );
        let Parsed::Request(req) = p(&["mergeiq", "open"]) else {
            panic!()
        };
        assert_eq!(
            req.kind,
            RequestKind::Open {
                dir: "/work/.".into()
            }
        );
    }

    #[test]
    fn invalid_arguments_exit_2_with_usage_on_stderr() {
        for args in [
            &["mergeiq", "merge", "only", "three", "args"][..],
            &["mergeiq", "frobnicate"],
            &["mergeiq", "--nope"],
            &["mergeiq", "resolve"],
        ] {
            match p(args) {
                Parsed::Exit {
                    code,
                    text,
                    to_stderr,
                } => {
                    assert_eq!(code, 2, "{args:?}");
                    assert!(to_stderr);
                    assert!(text.contains("Usage"), "{text}");
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn version_and_help_exit_0_on_stdout() {
        for flag in ["--version", "--help"] {
            match p(&["mergeiq", flag]) {
                Parsed::Exit {
                    code,
                    text,
                    to_stderr,
                } => {
                    assert_eq!(code, 0);
                    assert!(!to_stderr);
                    assert!(text.contains("mergeiq"), "{text}");
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(usage().contains("merge"));
    }
}
