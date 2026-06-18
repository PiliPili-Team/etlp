//! Async mpv IPC transport — Unix socket (macOS/Linux) and Windows named pipe.
//!
//! [`MpvClient`] wraps the wire-level concerns: request framing,
//! request→response correlation via `request_id`, and unsolicited event
//! dispatch. Callers issue commands through a single typed method and never
//! touch bytes directly. A background reader task owns the read half of the
//! connection; the write half is guarded by a `tokio::sync::Mutex` so
//! concurrent callers can each `await` their reply without blocking one another.

use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;

use crate::protocol::{
    Event, LineFramer, Message, MpvError, Request, Response,
};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors that can arise when communicating with mpv over its IPC socket.
#[derive(Debug, Error)]
pub enum ClientError {
    /// OS-level IO error (connect / read / write).
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    /// JSON encode / decode failure.
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// mpv replied with a non-success error string.
    #[error("mpv: {0}")]
    Mpv(#[from] MpvError),
    /// The IPC connection closed before a reply arrived.
    #[error("mpv IPC disconnected")]
    Disconnected,
}

// ── Public type aliases ───────────────────────────────────────────────────────

/// A callback invoked on the reader task for every unsolicited mpv event.
///
/// The closure is called synchronously inside the reader loop, so it must not
/// block; schedule heavy work onto a separate `tokio::spawn` if needed.
pub type EventHandler = Arc<dyn Fn(Event) + Send + Sync>;

// ── Internal types ────────────────────────────────────────────────────────────

type PendingMap = Mutex<HashMap<u64, oneshot::Sender<Response>>>;

struct Inner {
    /// Write half of the IPC connection, serialised across concurrent callers.
    write: tokio::sync::Mutex<Box<dyn AsyncWrite + Unpin + Send>>,
    /// Inflight requests keyed by `request_id`.
    pending: Arc<PendingMap>,
    /// Monotonically increasing source of unique request IDs.
    next_id: AtomicU64,
    /// Optional subscriber for unsolicited mpv events.
    event_handler: Option<EventHandler>,
}

// ── MpvClient ─────────────────────────────────────────────────────────────────

/// Async mpv IPC client.
///
/// Cheap to clone — all clones share the same underlying connection and
/// background reader task.
#[derive(Clone)]
pub struct MpvClient {
    inner: Arc<Inner>,
}

impl MpvClient {
    /// Connect to an mpv Unix domain socket and return a ready client.
    ///
    /// `event_handler`, when provided, is called from the reader task for every
    /// unsolicited event mpv pushes (e.g. `end-file`, `property-change`).
    #[cfg(unix)]
    pub async fn connect<P: AsRef<std::path::Path>>(
        socket_path: P,
        event_handler: Option<EventHandler>,
    ) -> Result<Self, ClientError> {
        let stream = tokio::net::UnixStream::connect(socket_path).await?;
        let (read, write) = tokio::io::split(stream);
        Ok(Self::from_halves(
            Box::new(read),
            Box::new(write),
            event_handler,
        ))
    }

    /// Connect to an mpv Windows named pipe and return a ready client.
    #[cfg(windows)]
    pub async fn connect<P: AsRef<std::path::Path>>(
        pipe_path: P,
        event_handler: Option<EventHandler>,
    ) -> Result<Self, ClientError> {
        let client = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(pipe_path.as_ref())?;
        let (read, write) = tokio::io::split(client);
        Ok(Self::from_halves(
            Box::new(read),
            Box::new(write),
            event_handler,
        ))
    }

    /// Build a client from pre-split async halves and spawn the reader task.
    ///
    /// Exposed as `pub(crate)` so unit tests can wire in a
    /// `tokio::io::duplex()` pair without opening a real socket.
    pub(crate) fn from_halves(
        reader: Box<dyn AsyncRead + Unpin + Send + 'static>,
        writer: Box<dyn AsyncWrite + Unpin + Send + 'static>,
        event_handler: Option<EventHandler>,
    ) -> Self {
        let pending: Arc<PendingMap> = Arc::new(Mutex::new(HashMap::new()));
        let inner = Arc::new(Inner {
            write: tokio::sync::Mutex::new(writer),
            pending: Arc::clone(&pending),
            next_id: AtomicU64::new(1),
            event_handler,
        });
        let client = Self { inner };

        // The reader task holds its own Arc refs so it outlives all clones of
        // the client handle.
        let pending_task = Arc::clone(&client.inner.pending);
        let handler_task = client.inner.event_handler.clone();
        tokio::spawn(reader_task(reader, pending_task, handler_task));

        client
    }

    /// Send a command to mpv and wait for its reply.
    ///
    /// Returns `Ok(Some(data))` on success, `Ok(None)` when the property is
    /// unavailable (`"property unavailable"` error), and `Err` for IO, JSON,
    /// or mpv-level errors.
    pub async fn command(
        &self,
        cmd: &str,
        args: &[Value],
    ) -> Result<Option<Value>, ClientError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let bytes = Request::new(id, cmd, args).encode()?;

        let (tx, rx) = oneshot::channel::<Response>();
        self.inner
            .pending
            .lock()
            .map_err(|_| ClientError::Disconnected)?
            .insert(id, tx);

        let write_result = {
            let mut w = self.inner.write.lock().await;
            w.write_all(&bytes).await
        };
        if let Err(e) = write_result {
            // Remove the dangling pending entry so it doesn't linger until the
            // reader task eventually clears the map on connection close.
            if let Ok(mut map) = self.inner.pending.lock() {
                map.remove(&id);
            }
            return Err(ClientError::Io(e));
        }

        let response = rx.await.map_err(|_| ClientError::Disconnected)?;
        Ok(response.result()?)
    }
}

// ── Background reader task ────────────────────────────────────────────────────

async fn reader_task(
    mut reader: Box<dyn AsyncRead + Unpin + Send>,
    pending: Arc<PendingMap>,
    event_handler: Option<EventHandler>,
) {
    let mut buf = vec![0u8; 8192];
    let mut framer = LineFramer::new();

    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if let Some(chunk) = buf.get(..n) {
                    for msg in framer.push(chunk) {
                        dispatch(msg, &pending, &event_handler);
                    }
                }
            }
        }
    }

    // Wake up every pending caller by dropping their senders; each
    // `rx.await` then resolves to `Err(RecvError)`, which `command` maps to
    // `ClientError::Disconnected`.
    if let Ok(mut map) = pending.lock() {
        map.clear();
    }
}

fn dispatch(
    msg: Message,
    pending: &Arc<PendingMap>,
    event_handler: &Option<EventHandler>,
) {
    match msg {
        Message::Response(resp) => {
            if let Ok(mut map) = pending.lock()
                && let Some(tx) = map.remove(&resp.request_id)
            {
                // Receiver already dropped (caller cancelled) — discard silently.
                let _ = tx.send(resp);
            }
        }
        Message::Event(event) => {
            if let Some(handler) = event_handler {
                handler(event);
            }
        }
        Message::Other(_) => {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering as AOrdering};
    use tokio::io::{AsyncBufReadExt, BufReader};

    /// Spin up a `MpvClient` wired to a `tokio::io::duplex()` pair.
    /// Returns the client and the server-side duplex end for manual driving.
    fn make_client(
        event_handler: Option<EventHandler>,
    ) -> (MpvClient, tokio::io::DuplexStream) {
        let (client_io, server_io) = tokio::io::duplex(16 * 1024);
        let (r, w) = tokio::io::split(client_io);
        let client =
            MpvClient::from_halves(Box::new(r), Box::new(w), event_handler);
        (client, server_io)
    }

    /// Simulate a minimal mpv: read one JSON request line and send a reply.
    async fn serve_one(server: tokio::io::DuplexStream, reply_data: Value) {
        let (read_half, mut write_half) = tokio::io::split(server);
        let mut reader = BufReader::new(read_half);
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("read request");
        let req: Value = serde_json::from_str(line.trim_end_matches('\n'))
            .expect("parse request");
        let id = req
            .get("request_id")
            .and_then(Value::as_u64)
            .expect("request_id");
        let reply = serde_json::json!({
            "request_id": id,
            "error": "success",
            "data": reply_data,
        });
        let mut bytes = serde_json::to_vec(&reply).expect("encode reply");
        bytes.push(b'\n');
        write_half.write_all(&bytes).await.expect("write reply");
    }

    /// Like `serve_one` but replies with a non-success error string.
    async fn serve_error(server: tokio::io::DuplexStream, error: &'static str) {
        let (read_half, mut write_half) = tokio::io::split(server);
        let mut reader = BufReader::new(read_half);
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("read request");
        let req: Value = serde_json::from_str(line.trim_end_matches('\n'))
            .expect("parse request");
        let id = req
            .get("request_id")
            .and_then(Value::as_u64)
            .expect("request_id");
        let reply = format!(
            "{{\"request_id\":{id},\"error\":\"{error}\",\"data\":null}}\n"
        );
        write_half
            .write_all(reply.as_bytes())
            .await
            .expect("write error reply");
    }

    #[tokio::test]
    async fn command_round_trip() {
        let (client, server) = make_client(None);
        tokio::spawn(serve_one(server, json!(42)));
        let result = client
            .command("get_property", &[json!("time-pos")])
            .await
            .expect("command ok");
        assert_eq!(result, Some(json!(42)));
    }

    #[tokio::test]
    async fn command_success_with_null_data() {
        let (client, server) = make_client(None);
        tokio::spawn(serve_one(server, json!(null)));
        let result = client
            .command("set_property", &[json!("pause"), json!(true)])
            .await
            .expect("command ok");
        // success + null data → Some(Value::Null)
        assert_eq!(result, Some(Value::Null));
    }

    #[tokio::test]
    async fn command_returns_none_for_property_unavailable() {
        let (client, server) = make_client(None);
        tokio::spawn(serve_error(server, "property unavailable"));
        let result = client
            .command("get_property", &[json!("chapter")])
            .await
            .expect("property unavailable → Ok(None)");
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn command_returns_mpv_error() {
        let (client, server) = make_client(None);
        tokio::spawn(serve_error(server, "invalid parameter"));
        let result = client.command("bad_cmd", &[]).await;
        assert!(matches!(result, Err(ClientError::Mpv(_))));
    }

    #[tokio::test]
    async fn event_handler_is_called() {
        use tokio::sync::Notify;

        let notify = Arc::new(Notify::new());
        let notify_clone = Arc::clone(&notify);

        let (client, mut server) =
            make_client(Some(Arc::new(move |e: Event| {
                if e.event == "end-file" {
                    notify_clone.notify_one();
                }
            })));

        server
            .write_all(b"{\"event\":\"end-file\"}\n")
            .await
            .expect("write event");

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            notify.notified(),
        )
        .await
        .expect("timed out waiting for event handler");

        drop(client);
    }

    #[tokio::test]
    async fn event_handler_not_triggered_by_response() {
        let called = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&called);

        let (client, server) = make_client(Some(Arc::new(move |_: Event| {
            flag.store(true, AOrdering::SeqCst);
        })));

        tokio::spawn(serve_one(server, json!(false)));

        client
            .command("get_property", &[json!("pause")])
            .await
            .expect("ok");

        assert!(!called.load(AOrdering::SeqCst));
    }

    #[tokio::test]
    async fn disconnection_before_response_returns_error() {
        let (client, server) = make_client(None);
        // Close the server side immediately so the reader task sees EOF.
        drop(server);
        // Yield to let the reader task run and clear the pending map.
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let result = client.command("get_property", &[json!("time-pos")]).await;
        // May surface as Disconnected (if reader cleared map first) or Io
        // (if the write to the closed pipe fails first).
        assert!(matches!(
            result,
            Err(ClientError::Disconnected | ClientError::Io(_))
        ));
    }

    #[tokio::test]
    async fn concurrent_commands_resolved_by_id() {
        let (client, server) = make_client(None);

        // The server echoes back each request_id with a matching numeric data value.
        tokio::spawn(async move {
            let (read_half, mut write_half) = tokio::io::split(server);
            let mut reader = BufReader::new(read_half);
            for _ in 0..3u8 {
                let mut line = String::new();
                reader.read_line(&mut line).await.expect("read");
                if line.is_empty() {
                    break;
                }
                let req: Value =
                    serde_json::from_str(line.trim_end_matches('\n'))
                        .expect("parse");
                let id =
                    req.get("request_id").and_then(Value::as_u64).expect("id");
                let reply = format!(
                    "{{\"request_id\":{id},\"error\":\"success\",\"data\":{id}}}\n"
                );
                write_half.write_all(reply.as_bytes()).await.expect("write");
            }
        });

        // Issue 3 concurrent commands from cloned handles.
        let c1 = client.clone();
        let c2 = client.clone();
        let c3 = client.clone();
        let (r1, r2, r3) = tokio::join!(
            c1.command("cmd_a", &[]),
            c2.command("cmd_b", &[]),
            c3.command("cmd_c", &[]),
        );
        assert!(r1.is_ok(), "cmd_a failed: {r1:?}");
        assert!(r2.is_ok(), "cmd_b failed: {r2:?}");
        assert!(r3.is_ok(), "cmd_c failed: {r3:?}");
    }
}
