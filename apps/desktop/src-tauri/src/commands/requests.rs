//! IPC commands for windows opened by CLI requests (`mergeiq merge/resolve/open`).

use std::sync::Arc;

use mergeiq_core::{Analysis, EncodingInfo, WhitespacePolicy};
use tauri::{AppHandle, Manager, State};

use crate::cli::host::request_id;
use crate::cli::prepare::{MergeRequestDoc, RequestPrepared, SaveMode};
use crate::cli::requests::{Registry, WindowKind};
use crate::ipc::IpcError;

/// Registry of open CLI request windows, shared with the socket server.
pub type Requests = Arc<Registry<RequestPrepared>>;

fn get(requests: &Requests, id: u32) -> Result<Arc<RequestPrepared>, IpcError> {
    requests
        .get(id)
        .ok_or_else(|| IpcError::Request(format!("request {id} is no longer open")))
}

#[tauri::command]
#[specta::specta]
pub fn merge_request_load(
    requests: State<'_, Requests>,
    id: u32,
) -> Result<MergeRequestDoc, IpcError> {
    get(&requests, id)?
        .merge_doc()
        .cloned()
        .ok_or_else(|| IpcError::Request(format!("request {id} is not a merge")))
}

#[tauri::command]
#[specta::specta]
pub fn merge_request_analyze(
    requests: State<'_, Requests>,
    id: u32,
    whitespace: WhitespacePolicy,
) -> Result<Analysis, IpcError> {
    get(&requests, id)?
        .reanalyze(whitespace)
        .map_err(IpcError::Request)
}

/// Writes the result and records the exit code for the waiting CLI process. The window
/// stays open; the UI calls [`merge_request_close`] afterwards.
#[tauri::command]
#[specta::specta]
pub fn merge_request_save(
    requests: State<'_, Requests>,
    id: u32,
    text: String,
    encoding: EncodingInfo,
    mode: SaveMode,
) -> Result<(), IpcError> {
    get(&requests, id)?
        .save(&text, &encoding, mode)
        .map(|_| ())
        .map_err(IpcError::Request)
}

/// Closes a request's window; its client gets the recorded exit code (1 if none).
#[tauri::command]
#[specta::specta]
pub fn request_close(app: AppHandle, id: u32) -> Result<(), IpcError> {
    let label = format!("{}-{id}", WindowKind::Merge.prefix());
    debug_assert_eq!(request_id(&label), Some(id));
    if let Some(window) = app.get_webview_window(&label) {
        window
            .destroy()
            .map_err(|e| IpcError::Request(e.to_string()))?;
    }
    Ok(())
}
