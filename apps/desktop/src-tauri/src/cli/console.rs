//! Text output for a binary that may be built for the Windows GUI subsystem.

use std::io::Write;

/// Prints `text` to stdout or stderr. On Windows (GUI subsystem, no console of its own)
/// it attaches to the parent console first so `--help`/`--version` and errors show up.
pub fn emit(text: &str, to_stderr: bool) {
    #[cfg(windows)]
    if windows::emit(text) {
        return;
    }
    let result = if to_stderr {
        std::io::stderr().write_all(text.as_bytes())
    } else {
        std::io::stdout().write_all(text.as_bytes())
    };
    let _ = result;
}

#[cfg(windows)]
mod windows {
    use std::io::Write;

    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };

    /// Writes to the parent console when this process has no usable stdout handle
    /// (the usual case for a GUI-subsystem exe started from a terminal). Returns `false`
    /// when stdout is already valid (redirected), so the caller uses it directly.
    pub fn emit(text: &str) -> bool {
        // SAFETY: plain Win32 calls with no pointers owned by us.
        let has_stdout = unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            !handle.is_null() && handle as isize != -1
        };
        if has_stdout {
            return false;
        }
        // SAFETY: as above.
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
            return false;
        }
        match std::fs::OpenOptions::new().write(true).open("CONOUT$") {
            Ok(mut console) => console.write_all(text.as_bytes()).is_ok(),
            Err(_) => false,
        }
    }
}
