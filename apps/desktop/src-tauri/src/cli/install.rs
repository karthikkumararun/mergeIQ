//! Installing the `mergeiq` command: a symlink to the bundled binary in a user-chosen
//! folder (macOS/Linux), plus PATH checks and hints.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Name of the installed command.
pub const COMMAND: &str = if cfg!(windows) {
    "mergeiq.exe"
} else {
    "mergeiq"
};

/// A folder the command can be installed into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallDir {
    pub path: String,
    /// Shown in the picker, e.g. `~/.local/bin`.
    pub label: String,
    /// Installing here needs an administrator prompt.
    pub needs_admin: bool,
}

/// A line to add to a shell start-up file when the folder is not on PATH.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PathHint {
    pub rc_file: String,
    pub line: String,
}

/// Result of installing the command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub link_path: String,
    pub on_path: bool,
    pub path_hint: Option<PathHint>,
}

/// The folders offered by default (macOS/Linux).
pub fn default_dirs(home: &Path) -> Vec<InstallDir> {
    vec![
        InstallDir {
            path: home.join(".local/bin").display().to_string(),
            label: "~/.local/bin".to_string(),
            needs_admin: false,
        },
        InstallDir {
            path: "/usr/local/bin".to_string(),
            label: "/usr/local/bin (asks for admin password)".to_string(),
            needs_admin: true,
        },
    ]
}

fn normalize(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| path.components().collect())
}

/// True when `dir` is one of the entries of `path_var`.
pub fn is_on_path(dir: &Path, path_var: &OsStr) -> bool {
    let wanted = normalize(dir);
    std::env::split_paths(path_var)
        .filter(|p| !p.as_os_str().is_empty())
        .any(|p| normalize(&p) == wanted)
}

/// The shell line that puts `dir` on PATH, and the file it belongs in.
pub fn path_hint(dir: &Path, home: &Path, shell: Option<&str>) -> PathHint {
    let shown = match dir.strip_prefix(home) {
        Ok(rel) => format!("$HOME/{}", rel.display()),
        Err(_) => dir.display().to_string(),
    };
    let shell = shell.unwrap_or("");
    if shell.ends_with("fish") {
        return PathHint {
            rc_file: "~/.config/fish/config.fish".to_string(),
            line: format!("fish_add_path {}", shown.replace("$HOME", "~")),
        };
    }
    let rc_file = if shell.ends_with("bash") {
        if cfg!(target_os = "macos") {
            "~/.bash_profile"
        } else {
            "~/.bashrc"
        }
    } else {
        "~/.zshrc"
    };
    PathHint {
        rc_file: rc_file.to_string(),
        line: format!("export PATH=\"{shown}:$PATH\""),
    }
}

/// The link in `dir` that points at `exe`, if the command is installed there.
pub fn installed_link(dir: &Path, exe: &Path) -> Option<PathBuf> {
    let link = dir.join(COMMAND);
    let target = std::fs::read_link(&link).ok()?;
    (normalize(&target) == normalize(exe)).then_some(link)
}

/// Creates (or refreshes) `dir/mergeiq` → `exe`. Never replaces a regular file.
#[cfg(unix)]
pub fn install_link(exe: &Path, dir: &Path) -> Result<PathBuf, String> {
    let link = dir.join(COMMAND);
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::remove_file(&link)
            .map_err(|e| format!("cannot replace {}: {e}", link.display()))?,
        Ok(_) => {
            return Err(format!(
                "{} already exists and is not a link; remove it first",
                link.display()
            ))
        }
        Err(_) => {}
    }
    std::os::unix::fs::symlink(exe, &link)
        .map_err(|e| format!("cannot link {}: {e}", link.display()))?;
    Ok(link)
}

#[cfg(not(unix))]
pub fn install_link(_exe: &Path, _dir: &Path) -> Result<PathBuf, String> {
    Err("linking is not used on this platform; add the install folder to PATH instead".into())
}

/// Same as [`install_link`] but through an administrator prompt (macOS: `osascript`;
/// elsewhere: `pkexec`). Only used for folders the user cannot write to.
#[cfg(unix)]
pub fn install_link_as_admin(exe: &Path, dir: &Path) -> Result<PathBuf, String> {
    let link = dir.join(COMMAND);
    let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "'\\''"));
    let script = format!(
        "mkdir -p {dir} && ln -sf {exe} {link}",
        dir = quote(dir),
        exe = quote(exe),
        link = quote(&link)
    );
    let output = if cfg!(target_os = "macos") {
        let applescript = format!(
            "do shell script \"{}\" with administrator privileges",
            script.replace('\\', "\\\\").replace('"', "\\\"")
        );
        std::process::Command::new("osascript")
            .args(["-e", &applescript])
            .output()
    } else {
        std::process::Command::new("pkexec")
            .args(["sh", "-c", &script])
            .output()
    }
    .map_err(|e| format!("cannot ask for administrator rights: {e}"))?;
    if output.status.success() {
        Ok(link)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(not(unix))]
pub fn install_link_as_admin(_exe: &Path, _dir: &Path) -> Result<PathBuf, String> {
    Err("administrator install is not used on this platform".into())
}

/// Windows: adds `dir` to the user PATH (and tells running apps). No-op elsewhere.
#[cfg(windows)]
pub fn add_to_user_path(dir: &Path) -> Result<(), String> {
    let script = "$d = $env:MERGEIQ_DIR; \
        $p = [Environment]::GetEnvironmentVariable('Path','User'); \
        if (($p -split ';') -notcontains $d) { \
          [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';') + ';' + $d), 'User') }";
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("MERGEIQ_DIR", dir)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(not(windows))]
pub fn add_to_user_path(_dir: &Path) -> Result<(), String> {
    Err("adding to PATH is only available on Windows".into())
}

/// Windows: the user's PATH as stored in the registry (the process PATH may be stale).
#[cfg(windows)]
pub fn user_path() -> Option<std::ffi::OsString> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path','User')",
        ])
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string()
            .into()
    })
}

#[cfg(not(windows))]
pub fn user_path() -> Option<std::ffi::OsString> {
    std::env::var_os("PATH")
}

/// The PATH a login shell would give a terminal (GUI apps started from Finder get a
/// minimal PATH). Best effort: `None` if the shell does not answer within 3 seconds.
#[cfg(unix)]
pub fn login_shell_path() -> Option<std::ffi::OsString> {
    let shell = std::env::var("SHELL").ok()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let output = std::process::Command::new(shell)
            .args(["-ilc", "printf %s \"$PATH\""])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let _ = tx.send(output);
    });
    let output = rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .ok()?
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string()
            .into()
    })
}

#[cfg(not(unix))]
pub fn login_shell_path() -> Option<std::ffi::OsString> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn install_to_a_user_folder_links_the_bundled_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("MergeIQ.app/Contents/MacOS/mergeiq");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "bin").unwrap();
        let dir = tmp.path().join(".local/bin");

        let link = install_link(&exe, &dir).unwrap();
        assert_eq!(link, dir.join("mergeiq"));
        assert_eq!(std::fs::read_link(&link).unwrap(), exe);
        assert_eq!(installed_link(&dir, &exe), Some(link.clone()));
        // Re-installing refreshes the link.
        assert_eq!(install_link(&exe, &dir).unwrap(), link);
    }

    #[test]
    fn never_overwrites_a_regular_file() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("mergeiq-bin");
        std::fs::write(&exe, "bin").unwrap();
        let dir = tmp.path().join("bin");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mergeiq"), "other tool").unwrap();
        let err = install_link(&exe, &dir).unwrap_err();
        assert!(err.contains("not a link"), "{err}");
        assert_eq!(
            std::fs::read_to_string(dir.join("mergeiq")).unwrap(),
            "other tool"
        );
    }

    #[test]
    fn path_membership() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let var = std::env::join_paths([&a, Path::new("/usr/bin")]).unwrap();
        assert!(is_on_path(&a, &var));
        assert!(!is_on_path(&b, &var));
    }

    #[test]
    fn hint_for_the_default_folder() {
        let home = Path::new("/Users/me");
        let hint = path_hint(&home.join(".local/bin"), home, Some("/bin/zsh"));
        assert_eq!(hint.rc_file, "~/.zshrc");
        assert_eq!(hint.line, r#"export PATH="$HOME/.local/bin:$PATH""#);
        let fish = path_hint(&home.join(".local/bin"), home, Some("/usr/bin/fish"));
        assert_eq!(fish.line, "fish_add_path ~/.local/bin");
    }

    #[test]
    fn default_folders() {
        let dirs = default_dirs(Path::new("/Users/me"));
        assert_eq!(dirs[0].path, "/Users/me/.local/bin");
        assert!(!dirs[0].needs_admin);
        assert!(dirs[1].needs_admin);
    }
}
