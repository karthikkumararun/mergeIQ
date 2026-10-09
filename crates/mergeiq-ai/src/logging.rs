//! A log writer that removes credentials from every line before it is written.
//!
//! Wrap the real writer: `fmt().with_writer(Redacting(non_blocking))`. This is a safety net
//! behind the provider adapters, which never log headers themselves: even if a dependency logs
//! a request at `trace` level, `x-api-key` and `Authorization` values do not reach the log.

use std::io::{self, Write};

use tracing::Metadata;
use tracing_subscriber::fmt::MakeWriter;

use crate::secrets::redact;

/// Redacts what the wrapped writer receives.
#[derive(Debug, Clone)]
pub struct Redacting<M>(pub M);

/// The per-event writer of [`Redacting`].
#[derive(Debug)]
pub struct RedactingWriter<W>(W);

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // The fmt layer hands over one whole event per write, so patterns are never split.
        let text = String::from_utf8_lossy(buf);
        self.0.write_all(redact(&text).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for Redacting<M> {
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter(self.0.make_writer())
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        RedactingWriter(self.0.make_writer_for(meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Sink {
        type Writer = Sink;
        fn make_writer(&'a self) -> Sink {
            self.clone()
        }
    }

    #[test]
    fn events_are_redacted_before_they_reach_the_sink() {
        let sink = Sink::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(Redacting(sink.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!("sending x-api-key: sk-ant-api03-supersecretvalue0000 to the provider");
            tracing::trace!(headers = ?"authorization: Bearer ghp_abcdefghijklmnopqrstuvwxyz0123", "request");
            tracing::info!("model finished in 3 s");
        });
        let out = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(
            !out.contains("supersecret") && !out.contains("ghp_abc"),
            "{out}"
        );
        assert!(out.contains("x-api-key: [redacted]"), "{out}");
        assert!(out.contains("model finished in 3 s"));
    }
}
