//! Single-instance routing over a per-user local socket (Unix domain socket / Windows
//! named pipe). Frames are a big-endian `u32` length followed by JSON; every message
//! carries a protocol version.

use std::io::{self, Read, Write};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::{prelude::*, ListenerOptions, Stream};
use serde::{Deserialize, Serialize};

use super::args::Request;

/// Current wire protocol version.
pub const PROTOCOL_VERSION: u32 = 1;
/// How long a client waits for the socket to accept before starting its own instance.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 1024 * 1024;

/// Client → server message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    /// Protocol version.
    pub v: u32,
    /// The CLI request.
    pub request: Request,
}

/// Server → client message, sent once the request's window has closed (or failed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    /// Protocol version.
    pub v: u32,
    /// Exit code for the invoking process.
    pub exit_code: i32,
    /// Text for the invoking process's stderr, if any.
    pub message: Option<String>,
}

impl Response {
    /// A response with the current protocol version.
    pub fn new(exit_code: i32, message: Option<String>) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            exit_code,
            message,
        }
    }
}

/// Serves requests; `handle` blocks until the request is finished (e.g. its window closed).
pub trait Handler: Send + Sync + 'static {
    /// Handles one request and returns the outcome for the invoking process.
    fn handle(&self, request: Request) -> Response;
}

/// Why a request could not be delivered.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Nothing is listening (missing socket, refused, or no answer within the timeout).
    #[error("no running instance")]
    NoInstance,
    /// The server closed the connection before answering (e.g. it crashed).
    #[error("MergeIQ closed the connection without a result")]
    Disconnected,
    /// The reply was malformed.
    #[error("invalid reply from MergeIQ: {0}")]
    Protocol(String),
    /// Any other I/O failure.
    #[error("{0}")]
    Io(#[from] io::Error),
}

pub(crate) fn write_frame(w: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

pub(crate) fn read_frame(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame exceeds the 1 MiB limit",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Where this user's instance listens.
#[derive(Debug, Clone)]
pub struct Endpoint {
    #[cfg(unix)]
    dir: std::path::PathBuf,
    #[cfg(unix)]
    path: std::path::PathBuf,
    #[cfg(windows)]
    pipe: String,
}

#[cfg(unix)]
impl Endpoint {
    /// `$MERGEIQ_SOCKET_DIR` (test hook) or the per-user runtime directory.
    pub fn default_for_user() -> io::Result<Self> {
        let uid = unsafe { libc::geteuid() };
        let dir = match std::env::var_os("MERGEIQ_SOCKET_DIR") {
            Some(dir) => std::path::PathBuf::from(dir),
            None => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(runtime) => std::path::PathBuf::from(runtime).join("mergeiq"),
                None => std::env::temp_dir().join(format!("mergeiq-{uid}")),
            },
        };
        Ok(Self::in_dir(dir))
    }

    /// An endpoint whose socket lives in `dir` (created 0700 on bind).
    pub fn in_dir(dir: std::path::PathBuf) -> Self {
        let path = dir.join("mergeiq.sock");
        Self { dir, path }
    }

    /// Path of the socket file.
    pub fn socket_path(&self) -> &std::path::Path {
        &self.path
    }

    fn name(&self) -> io::Result<interprocess::local_socket::Name<'_>> {
        self.path
            .as_path()
            .to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }

    fn ensure_private_dir(&self) -> io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)?;
        let meta = std::fs::metadata(&self.dir)?;
        if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a directory owned by you", self.dir.display()),
            ));
        }
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    fn connect(&self) -> io::Result<Stream> {
        Stream::connect(self.name()?)
    }

    /// Becomes the listening instance. A stale socket file (nobody answering) is removed;
    /// a live one yields `AddrInUse`.
    pub fn bind(&self) -> io::Result<interprocess::local_socket::Listener> {
        use interprocess::os::unix::local_socket::ListenerOptionsExt;
        self.ensure_private_dir()?;
        if self.path.exists() {
            match self.connect() {
                Ok(_) => return Err(io::Error::from(io::ErrorKind::AddrInUse)),
                Err(_) => std::fs::remove_file(&self.path)?,
            }
        }
        // Not every Unix lets us choose the socket's mode up front (macOS does not). The
        // containing directory is 0700 and ours, so nobody else can reach the socket in the
        // moment before the fallback `chmod`.
        match ListenerOptions::new()
            .name(self.name()?)
            .mode(0o600)
            .create_sync()
        {
            Err(err) if err.kind() == io::ErrorKind::Unsupported => {
                use std::os::unix::fs::PermissionsExt;
                let listener = ListenerOptions::new().name(self.name()?).create_sync()?;
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))?;
                Ok(listener)
            }
            other => other,
        }
    }
}

#[cfg(windows)]
impl Endpoint {
    /// `$MERGEIQ_PIPE_NAME` (test hook) or a per-user pipe name.
    pub fn default_for_user() -> io::Result<Self> {
        let pipe = std::env::var("MERGEIQ_PIPE_NAME").unwrap_or_else(|_| {
            let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
            format!("mergeiq-{user}")
        });
        Ok(Self { pipe })
    }

    fn name(&self) -> io::Result<interprocess::local_socket::Name<'_>> {
        self.pipe
            .as_str()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    }

    fn connect(&self) -> io::Result<Stream> {
        Stream::connect(self.name()?)
    }

    /// Creates the pipe with a DACL granting access to its owner (the current user) and
    /// SYSTEM only.
    pub fn bind(&self) -> io::Result<interprocess::local_socket::Listener> {
        use interprocess::os::windows::{
            local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
        };
        let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;OW)(A;;GA;;;SY)")
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let descriptor = SecurityDescriptor::deserialize(&sddl)?;
        ListenerOptions::new()
            .name(self.name()?)
            .security_descriptor(descriptor)
            .create_sync()
    }
}

impl Endpoint {
    /// Sends `request` to a running instance and waits for its result.
    ///
    /// Returns [`ClientError::NoInstance`] when nothing answers within `timeout`.
    pub fn send(&self, request: &Request, timeout: Duration) -> Result<Response, ClientError> {
        let (tx, rx) = mpsc::channel();
        let endpoint = self.clone();
        std::thread::spawn(move || {
            let _ = tx.send(endpoint.connect());
        });
        let mut stream = match rx.recv_timeout(timeout) {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                return Err(match err.kind() {
                    io::ErrorKind::NotFound
                    | io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::ConnectionReset => ClientError::NoInstance,
                    _ => ClientError::Io(err),
                })
            }
            Err(_) => return Err(ClientError::NoInstance),
        };
        let envelope = RequestEnvelope {
            v: PROTOCOL_VERSION,
            request: request.clone(),
        };
        let payload =
            serde_json::to_vec(&envelope).map_err(|e| ClientError::Protocol(e.to_string()))?;
        write_frame(&mut stream, &payload)?;
        let reply = match read_frame(&mut stream) {
            Ok(reply) => reply,
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(ClientError::Disconnected)
            }
            Err(err) => return Err(err.into()),
        };
        let response: Response =
            serde_json::from_slice(&reply).map_err(|e| ClientError::Protocol(e.to_string()))?;
        if response.v != PROTOCOL_VERSION {
            return Err(ClientError::Protocol(format!(
                "unsupported protocol version {}",
                response.v
            )));
        }
        Ok(response)
    }
}

/// Accepts connections on `listener` in a background thread, one handler thread each.
pub fn serve(
    listener: interprocess::local_socket::Listener,
    handler: Arc<dyn Handler>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { continue };
            let handler = Arc::clone(&handler);
            std::thread::spawn(move || {
                let response = match read_frame(&mut stream) {
                    Ok(frame) => answer(&frame, handler.as_ref()),
                    Err(err) => Response::new(2, Some(format!("bad request: {err}"))),
                };
                if let Ok(payload) = serde_json::to_vec(&response) {
                    let _ = write_frame(&mut stream, &payload);
                }
            });
        }
    })
}

fn answer(frame: &[u8], handler: &dyn Handler) -> Response {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(frame) else {
        return Response::new(2, Some("malformed request".into()));
    };
    match value.get("v").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(PROTOCOL_VERSION) => {}
        other => {
            return Response::new(
                2,
                Some(format!(
                    "unsupported protocol version {}",
                    other.map_or_else(|| "(missing)".to_string(), |v| v.to_string())
                )),
            )
        }
    }
    match serde_json::from_value::<RequestEnvelope>(value) {
        Ok(envelope) => handler.handle(envelope.request),
        Err(err) => Response::new(2, Some(format!("malformed request: {err}"))),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::cli::args::RequestKind;

    struct Echo;
    impl Handler for Echo {
        fn handle(&self, request: Request) -> Response {
            match request.kind {
                RequestKind::Resolve { path } => {
                    Response::new(7, Some(format!("resolved {}", path.display())))
                }
                _ => Response::new(0, None),
            }
        }
    }

    fn request() -> Request {
        Request {
            kind: RequestKind::Resolve {
                path: "/x/a.txt".into(),
            },
            cwd: "/x".into(),
        }
    }

    fn short_dir() -> tempfile::TempDir {
        // Unix socket paths are limited to ~100 bytes.
        tempfile::Builder::new()
            .prefix("mq")
            .tempdir_in("/tmp")
            .unwrap()
    }

    #[test]
    fn round_trip() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("s"));
        let listener = endpoint.bind().unwrap();
        serve(listener, Arc::new(Echo));
        let response = endpoint.send(&request(), CONNECT_TIMEOUT).unwrap();
        assert_eq!(response, Response::new(7, Some("resolved /x/a.txt".into())));
    }

    #[test]
    fn version_mismatch_is_rejected() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("s"));
        serve(endpoint.bind().unwrap(), Arc::new(Echo));
        let mut stream = endpoint.connect().unwrap();
        write_frame(&mut stream, br#"{"v":99,"request":{}}"#).unwrap();
        let reply: Response = serde_json::from_slice(&read_frame(&mut stream).unwrap()).unwrap();
        assert_eq!(reply.exit_code, 2);
        assert!(reply
            .message
            .unwrap()
            .contains("unsupported protocol version 99"));
    }

    #[test]
    fn stale_socket_is_removed_and_rebound() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("s"));
        endpoint.ensure_private_dir().unwrap();
        // A socket file left behind by a dead process: bound, never unlinked.
        drop(std::os::unix::net::UnixListener::bind(endpoint.socket_path()).unwrap());
        assert!(endpoint.socket_path().exists());
        assert!(matches!(
            endpoint.send(&request(), CONNECT_TIMEOUT),
            Err(ClientError::NoInstance)
        ));
        serve(endpoint.bind().unwrap(), Arc::new(Echo));
        assert_eq!(
            endpoint
                .send(&request(), CONNECT_TIMEOUT)
                .unwrap()
                .exit_code,
            7
        );
    }

    #[test]
    fn live_socket_is_not_stolen() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("s"));
        let _listener = endpoint.bind().unwrap();
        // `serve` is not running, but the socket accepts connections (backlog).
        let err = endpoint.bind().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AddrInUse);
    }

    #[test]
    fn no_instance_when_socket_missing() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("missing"));
        assert!(matches!(
            endpoint.send(&request(), CONNECT_TIMEOUT),
            Err(ClientError::NoInstance)
        ));
    }

    #[test]
    fn permissions() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("s"));
        let _listener = endpoint.bind().unwrap();
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(endpoint.socket_path()), 0o600);
        assert_eq!(mode(endpoint.socket_path().parent().unwrap()), 0o700);
    }

    #[test]
    fn insecure_existing_directory_is_tightened() {
        let tmp = short_dir();
        let dir = tmp.path().join("s");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let endpoint = Endpoint::in_dir(dir.clone());
        let _listener = endpoint.bind().unwrap();
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn server_crash_is_reported_as_disconnect() {
        let tmp = short_dir();
        let endpoint = Endpoint::in_dir(tmp.path().join("s"));
        let listener = endpoint.bind().unwrap();
        std::thread::spawn(move || {
            // Accept, read the request, then drop the connection without replying.
            if let Some(Ok(mut s)) = listener.incoming().next() {
                let _ = read_frame(&mut s);
            }
        });
        assert!(matches!(
            endpoint.send(&request(), CONNECT_TIMEOUT),
            Err(ClientError::Disconnected)
        ));
    }
}
