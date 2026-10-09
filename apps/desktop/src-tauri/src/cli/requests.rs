//! Registry of in-flight CLI requests: one window per request, and the exit code that
//! unblocks the waiting client when that window closes.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use super::args::Request;
use super::ipc_socket::{Handler, Response};

/// Exit code reported when a window closes without a recorded result (cancel).
pub const EXIT_CANCELLED: i32 = 1;

/// Which UI a request opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// The merge editor (`/merge/<id>`).
    Merge,
}

impl WindowKind {
    /// Window label prefix (`merge-<id>`).
    pub fn prefix(self) -> &'static str {
        match self {
            WindowKind::Merge => "merge",
        }
    }
}

/// Opens and closes the window for a request. The Tauri implementation lives in
/// [`super::host`]; tests substitute their own.
pub trait WindowHost: Send + Sync + 'static {
    /// Opens the window for request `id`.
    fn open(&self, id: u32, kind: WindowKind, title: &str) -> Result<(), String>;
}

/// A request's prepared state, as the dispatcher needs to see it.
pub trait Prepared: Send + Sync + 'static {
    /// Which window to open.
    fn kind(&self) -> WindowKind;
    /// Window title.
    fn title(&self) -> String;
    /// The exit code recorded by the user's action in the window, if any.
    fn outcome(&self) -> Option<i32>;
}

/// Prepares a request (reads files, runs the engine) or explains why it cannot be served.
pub type PrepareFn<P> = dyn Fn(&Request) -> Result<P, Response> + Send + Sync;

struct Slot<P> {
    prepared: Arc<P>,
    kind: WindowKind,
    done: Sender<i32>,
}

/// Tracks request state by id. `P` is the prepared request payload.
pub struct Registry<P> {
    next: Mutex<u32>,
    slots: Mutex<HashMap<u32, Slot<P>>>,
}

impl<P> Default for Registry<P> {
    fn default() -> Self {
        Self {
            next: Mutex::new(1),
            slots: Mutex::new(HashMap::new()),
        }
    }
}

impl<P> Registry<P> {
    /// Registers a request, returning its id and the channel its exit code arrives on.
    pub fn register(&self, prepared: P, kind: WindowKind) -> (u32, Receiver<i32>) {
        let id = {
            let mut next = self.next.lock().expect("registry lock");
            let id = *next;
            *next += 1;
            id
        };
        let (done, rx) = mpsc::channel();
        self.slots.lock().expect("registry lock").insert(
            id,
            Slot {
                prepared: Arc::new(prepared),
                kind,
                done,
            },
        );
        (id, rx)
    }

    /// The prepared payload for `id`.
    pub fn get(&self, id: u32) -> Option<Arc<P>> {
        self.slots
            .lock()
            .expect("registry lock")
            .get(&id)
            .map(|s| Arc::clone(&s.prepared))
    }

    /// Number of requests whose window is still open.
    pub fn open_count(&self) -> usize {
        self.slots.lock().expect("registry lock").len()
    }

    /// Completes request `id` with `code`, unblocking its client. Idempotent: only the
    /// first call for an id has an effect. Returns the window kind that was finished.
    pub fn finish(&self, id: u32, code: i32) -> Option<WindowKind> {
        let slot = self.slots.lock().expect("registry lock").remove(&id)?;
        let _ = slot.done.send(code);
        Some(slot.kind)
    }
}

impl<P: Prepared> Registry<P> {
    /// Completes request `id` with the code its window recorded (cancelled if none).
    /// Returns that code, or `None` if `id` was already finished.
    pub fn close(&self, id: u32) -> Option<i32> {
        let code = self.get(id)?.outcome().unwrap_or(EXIT_CANCELLED);
        self.finish(id, code).map(|_| code)
    }
}

/// Serves socket requests (and the primary's own request) by opening one window each.
pub struct Dispatcher<P: Prepared> {
    pub registry: Arc<Registry<P>>,
    pub host: Arc<dyn WindowHost>,
    pub prepare: Arc<PrepareFn<P>>,
}

impl<P: Prepared> Dispatcher<P> {
    /// Prepares and opens `request`, returning its id and exit-code channel.
    pub fn open(&self, request: &Request) -> Result<(u32, Receiver<i32>), Response> {
        self.open_prepared((self.prepare)(request)?)
    }

    /// Opens a window for an already prepared request.
    pub fn open_prepared(&self, prepared: P) -> Result<(u32, Receiver<i32>), Response> {
        let (kind, title) = (prepared.kind(), prepared.title());
        let (id, rx) = self.registry.register(prepared, kind);
        if let Err(message) = self.host.open(id, kind, &title) {
            self.registry.finish(id, 1);
            return Err(Response::new(
                1,
                Some(format!("could not open window: {message}")),
            ));
        }
        Ok((id, rx))
    }
}

impl<P: Prepared> Handler for Dispatcher<P> {
    fn handle(&self, request: Request) -> Response {
        match self.open(&request) {
            Ok((_, rx)) => Response::new(rx.recv().unwrap_or(1), None),
            Err(response) => response,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::RequestKind;

    struct Fake {
        outcome: Mutex<Option<i32>>,
    }

    impl Prepared for Fake {
        fn kind(&self) -> WindowKind {
            WindowKind::Merge
        }
        fn title(&self) -> String {
            "t".into()
        }
        fn outcome(&self) -> Option<i32> {
            *self.outcome.lock().unwrap()
        }
    }

    struct Host;
    impl WindowHost for Host {
        fn open(&self, _: u32, _: WindowKind, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn dispatcher() -> Dispatcher<Fake> {
        Dispatcher {
            registry: Arc::new(Registry::default()),
            host: Arc::new(Host),
            prepare: Arc::new(|_| {
                Ok(Fake {
                    outcome: Mutex::new(None),
                })
            }),
        }
    }

    fn request() -> Request {
        Request {
            kind: RequestKind::Open { dir: "/x".into() },
            cwd: "/x".into(),
        }
    }

    #[test]
    fn closing_without_a_result_reports_cancel() {
        let d = dispatcher();
        let (id, rx) = d.open(&request()).unwrap();
        assert_eq!(d.registry.open_count(), 1);
        assert_eq!(d.registry.close(id), Some(1));
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(d.registry.open_count(), 0);
        // Idempotent.
        assert_eq!(d.registry.close(id), None);
    }

    #[test]
    fn recorded_result_is_reported() {
        let d = dispatcher();
        let (id, rx) = d.open(&request()).unwrap();
        *d.registry.get(id).unwrap().outcome.lock().unwrap() = Some(0);
        d.registry.close(id);
        assert_eq!(rx.recv().unwrap(), 0);
    }

    #[test]
    fn handler_blocks_until_the_window_closes() {
        let d = Arc::new(dispatcher());
        let d2 = Arc::clone(&d);
        let t = std::thread::spawn(move || d2.handle(request()));
        // Wait for registration, then close.
        while d.registry.open_count() == 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        d.registry.close(1);
        assert_eq!(t.join().unwrap().exit_code, 1);
    }

    #[test]
    fn prepare_failure_is_returned_without_opening_a_window() {
        let mut d = dispatcher();
        d.prepare = Arc::new(|_| Err(Response::new(2, Some("no conflicts in x".into()))));
        let err = d.open(&request()).unwrap_err();
        assert_eq!(err.exit_code, 2);
        assert_eq!(d.registry.open_count(), 0);
    }
}
