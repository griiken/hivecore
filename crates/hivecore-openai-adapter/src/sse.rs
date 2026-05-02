//! Minimal SSE line decoder for `text/event-stream` bodies.
//!
//! OpenAI's chat-completion stream is a thin SSE: each event is `data: <json>`
//! lines separated by blank lines, plus a sentinel `data: [DONE]`.
//! Rolling our own avoids the full reqwest-eventsource dependency tree.

use bytes::{Bytes, BytesMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseEvent {
    Data(String),
    Done,
}

#[derive(Debug, Default)]
pub struct SseDecoder {
    buf: BytesMut,
    pending_data: String,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk of bytes; drain any complete events.
    pub fn feed(&mut self, chunk: Bytes) -> Vec<SseEvent> {
        self.buf.extend_from_slice(&chunk);
        let mut out = Vec::new();
        while let Some(pos) = find_newline(&self.buf) {
            let line = self.buf.split_to(pos.end);
            let line = &line[..pos.line_end];
            self.handle_line(line, &mut out);
        }
        out
    }

    fn handle_line(&mut self, line: &[u8], out: &mut Vec<SseEvent>) {
        if line.is_empty() {
            // Blank line — dispatch.
            if !self.pending_data.is_empty() {
                let data = std::mem::take(&mut self.pending_data);
                if data.trim() == "[DONE]" {
                    out.push(SseEvent::Done);
                } else {
                    out.push(SseEvent::Data(data));
                }
            }
            return;
        }

        let line = std::str::from_utf8(line).unwrap_or("");
        if let Some(rest) = line.strip_prefix(":") {
            // SSE comment — ignore.
            let _ = rest;
            return;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if !self.pending_data.is_empty() {
                self.pending_data.push('\n');
            }
            // SSE allows an optional leading space after the colon.
            self.pending_data
                .push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // event: / id: / retry: ignored — OpenAI doesn't use them here.
    }
}

struct Newline {
    /// Total bytes to consume (line + terminator).
    end: usize,
    /// Bytes of payload (excluding terminator).
    line_end: usize,
}

fn find_newline(buf: &[u8]) -> Option<Newline> {
    for (i, b) in buf.iter().enumerate() {
        if *b == b'\n' {
            let line_end = if i > 0 && buf[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            return Some(Newline {
                end: i + 1,
                line_end,
            });
        }
    }
    None
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
