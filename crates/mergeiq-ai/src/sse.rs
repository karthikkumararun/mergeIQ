//! Incremental parsers for streamed responses: Server-Sent Events and newline-delimited JSON.

/// One Server-Sent Event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseEvent {
    /// The `event:` name, if any.
    pub event: Option<String>,
    /// The joined `data:` lines.
    pub data: String,
}

/// Feeds bytes in arbitrary chunks and yields complete events.
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    /// A new parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes` and returns the events completed by them.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some((end, sep_len)) = find_blank_line(&self.buffer) {
            let block: Vec<u8> = self.buffer.drain(..end + sep_len).collect();
            if let Some(event) = parse_block(&block[..end]) {
                events.push(event);
            }
        }
        events
    }

    /// Flushes a final event that was not followed by a blank line.
    pub fn finish(&mut self) -> Option<SseEvent> {
        if self.buffer.iter().all(u8::is_ascii_whitespace) {
            self.buffer.clear();
            return None;
        }
        let block = std::mem::take(&mut self.buffer);
        parse_block(&block)
    }
}

/// The end index of the first event block and the length of its blank-line separator.
fn find_blank_line(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < buf.len() {
        if buf[i] == b'\n' {
            if buf.get(i + 1) == Some(&b'\n') {
                return Some((i, 2));
            }
            if buf.get(i + 1) == Some(&b'\r') && buf.get(i + 2) == Some(&b'\n') {
                return Some((i, 3));
            }
        } else if buf[i] == b'\r' && buf.get(i + 1) == Some(&b'\n') {
            if buf.get(i + 2) == Some(&b'\n') {
                return Some((i, 3));
            }
            if buf.get(i + 2) == Some(&b'\r') && buf.get(i + 3) == Some(&b'\n') {
                return Some((i, 4));
            }
        }
        i += 1;
    }
    None
}

fn parse_block(block: &[u8]) -> Option<SseEvent> {
    let text = String::from_utf8_lossy(block);
    let mut event = SseEvent::default();
    let mut data: Vec<&str> = Vec::new();
    let mut any = false;
    for line in text.lines() {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => {
                event.event = Some(value.to_string());
                any = true;
            }
            "data" => {
                data.push(value);
                any = true;
            }
            _ => {}
        }
    }
    event.data = data.join("\n");
    any.then_some(event)
}

/// Splits a byte stream into complete lines (for newline-delimited JSON).
#[derive(Debug, Default)]
pub struct LineParser {
    buffer: Vec<u8>,
}

impl LineParser {
    /// A new parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes` and returns the lines completed by them (without terminators).
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(nl) = self.buffer.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = self.buffer.drain(..=nl).collect();
            let text = String::from_utf8_lossy(&raw[..raw.len() - 1]);
            let text = text.trim_end_matches('\r');
            if !text.trim().is_empty() {
                lines.push(text.to_string());
            }
        }
        lines
    }

    /// A final line that was not newline-terminated.
    pub fn finish(&mut self) -> Option<String> {
        let rest = String::from_utf8_lossy(&std::mem::take(&mut self.buffer)).into_owned();
        (!rest.trim().is_empty()).then_some(rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &str = "event: message_start\ndata: {\"a\":1}\n\nevent: content_block_delta\ndata: {\"b\":2}\n\n: keep-alive\n\ndata: only data\n\n";

    #[test]
    fn parses_named_and_unnamed_events() {
        let mut p = SseParser::new();
        let events = p.feed(STREAM.as_bytes());
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: Some("message_start".into()),
                    data: "{\"a\":1}".into()
                },
                SseEvent {
                    event: Some("content_block_delta".into()),
                    data: "{\"b\":2}".into()
                },
                SseEvent {
                    event: None,
                    data: "only data".into()
                },
            ]
        );
    }

    #[test]
    fn any_chunking_gives_the_same_events() {
        let whole = SseParser::new().feed(STREAM.as_bytes());
        for size in 1..STREAM.len() {
            let mut p = SseParser::new();
            let mut got = Vec::new();
            for chunk in STREAM.as_bytes().chunks(size) {
                got.extend(p.feed(chunk));
            }
            assert_eq!(got, whole, "chunk size {size}");
        }
    }

    #[test]
    fn crlf_separators_and_multi_line_data() {
        let mut p = SseParser::new();
        let events = p.feed(b"event: x\r\ndata: one\r\ndata: two\r\n\r\ndata: three\n\n");
        assert_eq!(events[0].data, "one\ntwo");
        assert_eq!(events[0].event.as_deref(), Some("x"));
        assert_eq!(events[1].data, "three");
    }

    #[test]
    fn multibyte_characters_split_across_chunks_survive() {
        let text = "data: {\"t\":\"héllo wörld π\"}\n\n";
        let bytes = text.as_bytes();
        let mut p = SseParser::new();
        let mut got = Vec::new();
        for b in bytes.chunks(1) {
            got.extend(p.feed(b));
        }
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].data, "{\"t\":\"héllo wörld π\"}");
    }

    #[test]
    fn an_unterminated_final_event_is_flushed_by_finish() {
        let mut p = SseParser::new();
        assert!(p.feed(b"data: last").is_empty());
        assert_eq!(p.finish().unwrap().data, "last");
        assert!(p.finish().is_none());
    }

    #[test]
    fn line_parser_splits_ndjson() {
        let mut p = LineParser::new();
        assert_eq!(p.feed(b"{\"a\":1}\n{\"b\""), vec!["{\"a\":1}"]);
        assert_eq!(p.feed(b":2}\r\n\n"), vec!["{\"b\":2}"]);
        assert_eq!(p.feed(b"tail"), Vec::<String>::new());
        assert_eq!(p.finish().as_deref(), Some("tail"));
    }
}
