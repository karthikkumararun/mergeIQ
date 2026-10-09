pub mod cli;
mod commands;
mod ipc;
mod logging;
mod recents;
pub mod repo_service;
pub mod repos;
mod settings;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{Manager, WebviewWindowBuilder, WindowEvent};

use cli::args::{Parsed, Request, RequestKind};
use cli::host::{request_id, TauriHost};
use cli::ipc_socket::{serve, ClientError, Endpoint, Handler, Response, CONNECT_TIMEOUT};
use cli::prepare::{check_open, prepare, RequestPrepared};
use cli::requests::{Dispatcher, Registry};
use commands::repos::{open_repo_window, repo_id};
use commands::requests::Requests;
use repos::{RepoRegistry, Repos};

/// What the process was started to do, decided before Tauri starts.
#[derive(Default)]
struct Launch {
    /// Listener for later CLI invocations; `None` when binding failed.
    listener: Option<interprocess::local_socket::Listener>,
    /// The CLI merge/resolve request that started this instance.
    initial: Option<RequestPrepared>,
    /// The `mergeiq open` directory that started this instance.
    open_dir: Option<PathBuf>,
}

/// Tracks the instance's CLI-only lifetime.
struct Lifecycle {
    /// Started by a CLI request: exit when the last window it opened closes.
    for_cli: bool,
    /// The initial request's window id and, once closed, its exit code.
    own: Mutex<(Option<u32>, Option<i32>)>,
}

fn finish_client(response: Response) -> ! {
    if let Some(message) = response.message {
        cli::console::emit(&format!("{message}\n"), true);
    }
    std::process::exit(response.exit_code)
}

/// Serves socket requests: `open` focuses or opens a repository window and returns at once;
/// merge/resolve requests get a window and block until it closes.
struct AppHandler {
    dispatcher: Arc<Dispatcher<RequestPrepared>>,
    app: tauri::AppHandle,
    repos: Repos,
}

impl Handler for AppHandler {
    fn handle(&self, request: Request) -> Response {
        match request.kind {
            RequestKind::Open { dir } => match open_repo_window(&self.app, &self.repos, &dir) {
                Ok(_) => Response::new(0, None),
                Err(err) => Response::new(cli::prepare::EXIT_INVALID, Some(err.to_string())),
            },
            _ => self.dispatcher.handle(request),
        }
    }
}

/// Routes a CLI request: to the running instance if there is one, else becomes it.
fn route(request: Request) -> ! {
    let endpoint = match Endpoint::default_for_user() {
        Ok(endpoint) => endpoint,
        Err(err) => finish_client(Response::new(1, Some(format!("mergeiq: {err}")))),
    };
    let mut prepared = None;
    let mut open_dir = None;
    for attempt in 0..2 {
        match endpoint.send(&request, CONNECT_TIMEOUT) {
            Ok(response) => finish_client(response),
            Err(ClientError::NoInstance) => {}
            Err(err) => finish_client(Response::new(1, Some(format!("mergeiq: {err}")))),
        }
        if prepared.is_none() && open_dir.is_none() {
            if let RequestKind::Open { dir } = &request.kind {
                match check_open(dir) {
                    Ok(()) => open_dir = Some(dir.clone()),
                    Err(response) => finish_client(response),
                }
            } else {
                match prepare(&request) {
                    Ok(p) => prepared = Some(p),
                    Err(response) => finish_client(response),
                }
            }
        }
        match endpoint.bind() {
            Ok(listener) => start(Launch {
                listener: Some(listener),
                initial: prepared,
                open_dir,
            }),
            // Lost a start-up race with another invocation: talk to the winner instead.
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && attempt == 0 => {}
            Err(err) => {
                eprintln!("mergeiq: cannot listen for other invocations: {err}");
                start(Launch {
                    listener: None,
                    initial: prepared,
                    open_dir,
                })
            }
        }
    }
    finish_client(Response::new(
        1,
        Some("mergeiq: could not reach MergeIQ".into()),
    ))
}

fn handle_window_destroyed(
    app: &tauri::AppHandle,
    label: &str,
    requests: &Requests,
    repos: &Repos,
    lifecycle: &Lifecycle,
) {
    if let Some(id) = repo_id(label) {
        repos.remove(id);
    } else if let Some(id) = request_id(label) {
        let code = requests.close(id);
        let mut own = lifecycle.own.lock().expect("lifecycle lock");
        if own.0 == Some(id) {
            own.1 = code;
        }
    } else {
        return;
    }
    if lifecycle.for_cli && requests.open_count() == 0 && repos.is_empty() {
        let code = lifecycle.own.lock().expect("lifecycle lock").1;
        app.exit(code.unwrap_or(0));
    }
}

fn start(launch: Launch) -> ! {
    let builder = ipc::specta_builder();

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/bindings.ts"),
        )
        .expect("failed to export typescript bindings");

    let requests: Requests = Arc::new(Registry::default());
    let repos: Repos = Arc::new(RepoRegistry::default());
    let lifecycle = Arc::new(Lifecycle {
        for_cli: launch.initial.is_some() || launch.open_dir.is_some(),
        own: Mutex::new((None, None)),
    });
    let Launch {
        listener,
        initial,
        open_dir,
    } = launch;

    let events_requests = Arc::clone(&requests);
    let events_repos = Arc::clone(&repos);
    let setup_repos = Arc::clone(&repos);
    let events_lifecycle = Arc::clone(&lifecycle);
    let setup_requests = Arc::clone(&requests);
    let setup_lifecycle = Arc::clone(&lifecycle);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(repos)
        .manage(requests)
        .invoke_handler(builder.invoke_handler())
        .on_window_event(move |window, event| {
            if let WindowEvent::Destroyed = event {
                handle_window_destroyed(
                    window.app_handle(),
                    window.label(),
                    &events_requests,
                    &events_repos,
                    &events_lifecycle,
                );
            }
        })
        .setup(move |app| {
            let guard = logging::init();
            if let Some(guard) = guard {
                app.manage(guard);
            }
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "MergeIQ starting");
            builder.mount_events(app);

            let dispatcher = Arc::new(Dispatcher {
                registry: Arc::clone(&setup_requests),
                host: Arc::new(TauriHost {
                    app: app.handle().clone(),
                }),
                prepare: Arc::new(prepare),
            });
            if let Some(listener) = listener {
                let handler = Arc::new(AppHandler {
                    dispatcher: Arc::clone(&dispatcher),
                    app: app.handle().clone(),
                    repos: Arc::clone(&setup_repos),
                });
                serve(listener, handler as Arc<dyn Handler>);
            }
            match (initial, open_dir) {
                (_, Some(dir)) => {
                    open_repo_window(app.handle(), &setup_repos, &dir)
                        .map_err(|e| e.to_string())?;
                }
                (Some(prepared), None) => {
                    let (id, _) = dispatcher
                        .open_prepared(prepared)
                        .map_err(|r| r.message.unwrap_or_else(|| "could not open window".into()))?;
                    setup_lifecycle.own.lock().expect("lifecycle lock").0 = Some(id);
                }
                (None, None) => {
                    WebviewWindowBuilder::from_config(app.handle(), &app.config().app.windows[0])?
                        .build()?;
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    std::process::exit(0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_default();
    match cli::args::parse(std::env::args_os(), &cwd) {
        Parsed::Exit {
            code,
            text,
            to_stderr,
        } => {
            cli::console::emit(&text, to_stderr);
            std::process::exit(code);
        }
        Parsed::Request(request) => route(request),
        Parsed::Gui => start(Launch::default()),
    }
}
