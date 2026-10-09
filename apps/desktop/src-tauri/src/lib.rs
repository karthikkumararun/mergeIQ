pub mod cli;
mod commands;
mod ipc;
mod logging;
mod settings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_default();
    if let cli::args::Parsed::Exit {
        code,
        text,
        to_stderr,
    } = cli::args::parse(std::env::args_os(), &cwd)
    {
        cli::console::emit(&text, to_stderr);
        std::process::exit(code);
    }
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
        .manage(commands::git::GitState::default())
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
