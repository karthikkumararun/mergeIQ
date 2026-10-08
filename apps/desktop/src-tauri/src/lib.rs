mod ipc;
mod logging;
mod settings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = ipc::specta_builder();

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/bindings.ts"),
        )
        .expect("failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let guard = logging::init();
            if let Some(guard) = guard {
                app.manage(guard);
            }
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "MergeIQ starting");
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
