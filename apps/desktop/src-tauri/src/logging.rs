use std::path::PathBuf;

use mergeiq_ai::logging::Redacting;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Initializes structured logging to a daily-rotating file in the OS log dir, keeping at
/// most 7 files, with credentials redacted from every line. Level defaults to `info`, overridable with the `MERGEIQ_LOG` env var.
/// Returns a guard that must be held for the lifetime of the app to flush buffered logs.
pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("MERGEIQ_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let Some(dir) = log_dir() else {
        eprintln!("mergeiq: could not determine log directory; logging to stderr only");
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(Redacting(std::io::stderr))
            .init();
        return None;
    };

    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "mergeiq: failed to create log directory {}: {err}; logging to stderr only",
            dir.display()
        );
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(Redacting(std::io::stderr))
            .init();
        return None;
    }

    let appender = match RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("mergeiq")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
    {
        Ok(appender) => appender,
        Err(err) => {
            eprintln!("mergeiq: failed to set up log rotation: {err}; logging to stderr only");
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(Redacting(std::io::stderr))
                .init();
            return None;
        }
    };

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        // Credentials never reach the log, whatever a dependency prints at trace level.
        .with_writer(Redacting(non_blocking))
        .init();
    Some(guard)
}

fn log_dir() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    #[cfg(target_os = "macos")]
    {
        Some(base.home_dir().join("Library/Logs/MergeIQ"))
    }
    #[cfg(target_os = "windows")]
    {
        Some(base.data_local_dir().join("MergeIQ").join("logs"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        base.state_dir()
            .map(|p| p.to_path_buf())
            .or_else(|| Some(base.cache_dir().to_path_buf()))
            .map(|p| p.join("MergeIQ").join("logs"))
    }
}
