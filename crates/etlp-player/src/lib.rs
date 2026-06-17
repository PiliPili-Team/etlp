//! Media player launch and control for etlp.
//!
//! This crate ports `players.py` and `python_mpv_jsonipc.py`. It is built up
//! incrementally (see `docs/TODO.md`): the mpv JSON IPC wire protocol lands
//! first as a pure, testable codec, followed by the async transport and the
//! per-player launch/playlist/progress orchestration.

pub mod protocol;

pub use protocol::{
    Event, LineFramer, Message, MpvError, Request, Response, parse_line,
};
