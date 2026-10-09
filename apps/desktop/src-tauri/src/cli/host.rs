//! Tauri implementation of [`WindowHost`]: one webview window per request.

use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

use super::requests::{WindowHost, WindowKind};

/// Opens request windows on the running app.
pub struct TauriHost {
    pub app: AppHandle,
}

impl WindowHost for TauriHost {
    fn open(&self, id: u32, kind: WindowKind, title: &str) -> Result<(), String> {
        let prefix = kind.prefix();
        let window = WebviewWindowBuilder::new(
            &self.app,
            format!("{prefix}-{id}"),
            WebviewUrl::App(format!("{prefix}/{id}").into()),
        )
        .title(title)
        .inner_size(1280.0, 800.0)
        .min_inner_size(1024.0, 640.0)
        .build()
        .map_err(|e| e.to_string())?;
        let _ = window.set_focus();
        Ok(())
    }
}

/// Parses a request window label (`merge-3`) back into its request id.
pub fn request_id(label: &str) -> Option<u32> {
    label.strip_prefix("merge-")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_map_back_to_request_ids() {
        assert_eq!(request_id("merge-12"), Some(12));
        assert_eq!(request_id("repo-3"), None);
        assert_eq!(request_id("main"), None);
        assert_eq!(request_id("merge-x"), None);
    }
}
