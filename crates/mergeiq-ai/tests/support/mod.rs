//! A tiny HTTP server for provider tests: it answers each connection with the next canned reply
//! and records what it was sent.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One canned response.
#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Body pieces, each followed by a pause. Splitting lets tests exercise streaming.
    pub chunks: Vec<(Duration, Vec<u8>)>,
}

impl Reply {
    pub fn sse(body: &str) -> Reply {
        Reply {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            // Split mid-event on purpose.
            chunks: split(body.as_bytes(), 97),
        }
    }

    pub fn json(status: u16, body: &str) -> Reply {
        Reply {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            chunks: vec![(Duration::ZERO, body.as_bytes().to_vec())],
        }
    }

    pub fn ndjson(body: &str) -> Reply {
        Reply {
            status: 200,
            headers: vec![("content-type".into(), "application/x-ndjson".into())],
            chunks: split(body.as_bytes(), 53),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Reply {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Streams `first`, then waits `pause`, then `rest`.
    pub fn slow(status: u16, first: &str, pause: Duration, rest: &str) -> Reply {
        Reply {
            status,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            chunks: vec![
                (Duration::ZERO, first.as_bytes().to_vec()),
                (pause, rest.as_bytes().to_vec()),
            ],
        }
    }
}

fn split(bytes: &[u8], size: usize) -> Vec<(Duration, Vec<u8>)> {
    bytes
        .chunks(size)
        .map(|c| (Duration::ZERO, c.to_vec()))
        .collect()
}

/// A request as received.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    /// Lower-cased header names.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("request body is JSON")
    }
}

pub struct FakeServer {
    pub url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl FakeServer {
    /// Serves `replies` in order; once exhausted, every further request gets a 500.
    pub async fn start(replies: Vec<Reply>) -> FakeServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(std::collections::VecDeque::from(replies)));
        let rec = requests.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let (rec, queue) = (rec.clone(), queue.clone());
                tokio::spawn(async move {
                    let Some(request) = read_request(&mut sock).await else {
                        return;
                    };
                    rec.lock().unwrap().push(request);
                    let reply = queue.lock().unwrap().pop_front().unwrap_or_else(|| {
                        Reply::json(500, r#"{"error":{"message":"no more replies"}}"#)
                    });
                    let mut head = format!("HTTP/1.1 {} X\r\nconnection: close\r\n", reply.status);
                    for (n, v) in &reply.headers {
                        head.push_str(&format!("{n}: {v}\r\n"));
                    }
                    head.push_str("\r\n");
                    if sock.write_all(head.as_bytes()).await.is_err() {
                        return;
                    }
                    for (pause, chunk) in reply.chunks {
                        tokio::time::sleep(pause).await;
                        if sock.write_all(&chunk).await.is_err() || sock.flush().await.is_err() {
                            return;
                        }
                    }
                    let _ = sock.shutdown().await;
                });
            }
        });
        FakeServer { url, requests }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_request(sock: &mut tokio::net::TcpStream) -> Option<Recorded> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let head_end = loop {
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.lines();
    let first = lines.next()?;
    let mut parts = first.split_whitespace();
    let (method, path) = (parts.next()?.to_string(), parts.next()?.to_string());
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let len: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    Some(Recorded {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

/// Reads a fixture file from `tests/fixtures`.
pub fn fixture(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
