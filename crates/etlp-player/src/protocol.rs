//! The mpv JSON IPC wire protocol.
//!
//! mpv exposes a line-delimited JSON IPC channel: each message is a UTF-8 JSON
//! object terminated by `\n`. Requests carry a `command` array and a
//! `request_id`; mpv replies with a matching `request_id`, an `error` string
//! and optional `data`, and pushes unsolicited `event` objects.
//!
//! This module is pure: it (de)serializes messages and frames a byte stream
//! into messages, with no IO. The async transport (socket / named pipe) builds
//! on it.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// mpv's success sentinel in the `error` field.
const SUCCESS: &str = "success";
/// mpv's "property unavailable" error, mapped to `Ok(None)`.
const PROPERTY_UNAVAILABLE: &str = "property unavailable";

/// An error returned by mpv (a non-`success` `error` string).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("mpv error: {0}")]
pub struct MpvError(pub String);

/// A command request sent to mpv.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Request {
    /// The command name followed by its arguments.
    pub command: Vec<Value>,
    /// Correlates the reply; echoed back in the response.
    pub request_id: u64,
}

impl Request {
    /// Build a request from a command name and its arguments.
    #[must_use]
    pub fn new(request_id: u64, command: &str, args: &[Value]) -> Self {
        let mut list = Vec::with_capacity(args.len() + 1);
        list.push(Value::String(command.to_owned()));
        list.extend_from_slice(args);
        Self {
            command: list,
            request_id,
        }
    }

    /// Encode to a single newline-terminated JSON line.
    ///
    /// Returns an error only if the command arguments are not serializable,
    /// which cannot happen for the [`Value`] inputs accepted by [`Request::new`].
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// A reply to a [`Request`].
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Response {
    /// The id of the request this answers.
    pub request_id: u64,
    /// `"success"` or an error description.
    #[serde(default)]
    pub error: String,
    /// The command result, when any.
    #[serde(default)]
    pub data: Value,
}

impl Response {
    /// Whether mpv reported success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.error == SUCCESS
    }

    /// Interpret the reply: `Ok(Some(data))` on success, `Ok(None)` when the
    /// property is merely unavailable, and `Err` for any other mpv error.
    pub fn result(&self) -> Result<Option<Value>, MpvError> {
        if self.is_success() {
            Ok(Some(self.data.clone()))
        } else if self.error == PROPERTY_UNAVAILABLE {
            Ok(None)
        } else {
            Err(MpvError(self.error.clone()))
        }
    }
}

/// An unsolicited event pushed by mpv.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Event {
    /// The event name (e.g. `end-file`, `property-change`).
    pub event: String,
    /// The remaining fields of the event object.
    #[serde(flatten)]
    pub payload: Value,
}

/// A decoded incoming message: a reply, an event, or something unrecognized.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A reply correlated by `request_id`.
    Response(Response),
    /// An asynchronous event.
    Event(Event),
    /// A well-formed JSON object that is neither (ignored by the client).
    Other(Value),
}

/// Parse a single JSON line into a [`Message`].
///
/// A reply (`request_id` present) takes priority over an event. Blank lines
/// yield `None`.
pub fn parse_line(line: &[u8]) -> Result<Option<Message>, serde_json::Error> {
    let trimmed = line.strip_suffix(b"\r").unwrap_or(line);
    if trimmed.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let value: Value = serde_json::from_slice(trimmed)?;
    if value.get("request_id").is_some() {
        let response: Response = serde_json::from_value(value)?;
        return Ok(Some(Message::Response(response)));
    }
    if value.get("event").is_some() {
        let event: Event = serde_json::from_value(value)?;
        return Ok(Some(Message::Event(event)));
    }
    Ok(Some(Message::Other(value)))
}

/// A reframer that accumulates bytes and yields complete newline-delimited
/// messages, keeping any trailing partial line buffered.
#[derive(Debug, Default)]
pub struct LineFramer {
    buffer: Vec<u8>,
}

impl LineFramer {
    /// Create an empty framer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk and drain every complete line as a parsed [`Message`].
    ///
    /// Lines that fail to parse are skipped, so one malformed line cannot
    /// stall the stream.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Message> {
        self.buffer.extend_from_slice(chunk);
        let mut messages = Vec::new();
        while let Some(pos) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=pos).collect();
            let line = line.strip_suffix(b"\n").unwrap_or(&line);
            if let Ok(Some(message)) = parse_line(line) {
                messages.push(message);
            }
        }
        messages
    }

    /// Whether there is no buffered partial line.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_encodes_command_and_newline() {
        let req =
            Request::new(7, "loadfile", &[json!("a.mkv"), json!("append")]);
        let bytes = req.encode().expect("encode");
        assert_eq!(bytes.last().copied(), Some(b'\n'));
        let line = bytes.strip_suffix(b"\n").expect("newline");
        let value: Value = serde_json::from_slice(line).expect("json");
        assert_eq!(value.get("request_id"), Some(&json!(7)));
        assert_eq!(
            value.get("command"),
            Some(&json!(["loadfile", "a.mkv", "append"]))
        );
    }

    #[test]
    fn response_result_maps_errors() {
        let ok = Response {
            request_id: 1,
            error: "success".into(),
            data: json!(42),
        };
        assert_eq!(ok.result(), Ok(Some(json!(42))));

        let unavailable = Response {
            request_id: 2,
            error: "property unavailable".into(),
            data: Value::Null,
        };
        assert_eq!(unavailable.result(), Ok(None));

        let failed = Response {
            request_id: 3,
            error: "invalid parameter".into(),
            data: Value::Null,
        };
        assert_eq!(failed.result(), Err(MpvError("invalid parameter".into())));
    }

    #[test]
    fn parse_line_prefers_response_then_event() {
        let resp =
            parse_line(br#"{"request_id":5,"error":"success","data":1}"#)
                .expect("ok")
                .expect("some");
        assert!(matches!(resp, Message::Response(r) if r.request_id == 5));

        let ev = parse_line(br#"{"event":"end-file","reason":"eof"}"#)
            .expect("ok")
            .expect("some");
        match ev {
            Message::Event(e) => {
                assert_eq!(e.event, "end-file");
                assert_eq!(e.payload.get("reason"), Some(&json!("eof")));
            }
            _ => panic!("expected event"),
        }
    }

    #[test]
    fn parse_line_ignores_blank() {
        assert_eq!(parse_line(b"   ").expect("ok"), None);
        assert_eq!(parse_line(b"").expect("ok"), None);
    }

    #[test]
    fn framer_splits_and_buffers_partial() {
        let mut framer = LineFramer::new();
        // A full line plus a partial one.
        let first = framer.push(
            b"{\"event\":\"start-file\"}\n{\"request_id\":1,\"error\":\"suc",
        );
        assert_eq!(first.len(), 1);
        assert!(!framer.is_empty());
        // Completing the partial line yields the second message.
        let second = framer.push(b"cess\",\"data\":null}\n");
        assert_eq!(second.len(), 1);
        assert!(framer.is_empty());
        assert!(matches!(
            second.first(),
            Some(Message::Response(r)) if r.request_id == 1
        ));
    }

    #[test]
    fn framer_skips_malformed_lines() {
        let mut framer = LineFramer::new();
        let msgs = framer.push(b"not json\n{\"event\":\"seek\"}\n");
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            msgs.first(),
            Some(Message::Event(e)) if e.event == "seek"
        ));
    }
}
