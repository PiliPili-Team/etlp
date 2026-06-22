//! Media player launch and control for etlp.
//!
//! This crate ports `players.py` and `python_mpv_jsonipc.py`. It is built up
//! incrementally (see `docs/TODO.md`): the mpv JSON IPC wire protocol lands
//! first as a pure, testable codec, followed by the async transport and the
//! per-player launch/playlist/progress orchestration.

pub mod dandan;
pub mod executable;
pub mod mpc;
pub mod mpv;
pub mod player_manager;
pub mod pot;
pub mod protocol;
pub mod transport;
pub mod vlc;

pub use dandan::{DanDanConfig, DanDanError, DanDanHandle};
pub use executable::resolve_player_executable;
pub use mpc::{MpcError, MpcHandle};
pub use mpv::{
    IpcPath, LaunchArgs, LoadMode, LoadOptions, MpvHandle, MpvPlaylistEntry,
    PlayerError, build_args, connect_with_retry,
};
pub use player_manager::{
    PlayerHandle, PlayerManager, SyncEntry, realtime_playing_feedback_loop,
    redirect_next_ep_loop,
};
pub use pot::{PotError, PotHandle};
pub use protocol::{
    Event, LineFramer, Message, MpvError, Request, Response, parse_line,
};
pub use transport::{ClientError, EventHandler, MpvClient};
pub use vlc::{VlcError, VlcHandle};
