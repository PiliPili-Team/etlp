//! Lightweight telemetry for etlp.
//!
//! [`Span`] measures the wall-clock duration of a named operation and emits a
//! structured `tracing::info!` event when [`Span::finish`] is called.
//!
//! [`PlayMetrics`] aggregates per-session timing events collected during the
//! full play chain (request parse → episode fetch → M3U8 write → player
//! spawn) into a single summary event for easy analysis.
//!
//! All output goes through `tracing` so it inherits the application's log
//! level filter and masking layer.

use std::time::Instant;

// ── Span ──────────────────────────────────────────────────────────────────────

/// A single named timing span.
///
/// Created with [`Span::new`], optionally annotated with a session id via
/// [`Span::with_session`], and closed with [`Span::finish`], which emits an
/// `INFO`-level tracing event.
///
/// # Example
///
/// ```
/// use etlp_metrics::Span;
/// let ms = Span::new("parse").finish();
/// assert!(ms < 1000, "parse must be fast");
/// ```
pub struct Span {
    label: &'static str,
    session_id: Option<usize>,
    start: Instant,
}

impl Span {
    /// Start a new span with the given label.
    #[must_use]
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            session_id: None,
            start: Instant::now(),
        }
    }

    /// Attach a session id so the event can be correlated with other events
    /// from the same play chain.
    #[must_use]
    pub fn with_session(mut self, id: usize) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Finish the span, emit a tracing event, and return the elapsed
    /// milliseconds.
    pub fn finish(self) -> u128 {
        let ms = self.start.elapsed().as_millis();
        match self.session_id {
            Some(id) => tracing::info!(
                session_id = id,
                span = self.label,
                elapsed_ms = ms,
                "metrics"
            ),
            None => {
                tracing::info!(span = self.label, elapsed_ms = ms, "metrics")
            }
        }
        ms
    }

    /// Return elapsed milliseconds without finishing the span.
    #[must_use]
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

// ── PlayMetrics ───────────────────────────────────────────────────────────────

/// Per-session timing summary emitted at the end of a play chain.
///
/// Aggregates key milestones — from the HTTP request arriving to the player
/// process being spawned — into a single structured log event.
///
/// # Fields
///
/// All timing values are in milliseconds.
#[derive(Debug, Default)]
pub struct PlayMetrics {
    /// Unique session identifier, matching the `session_id` field in all
    /// [`Span`] events for this play chain.
    pub session_id: usize,
    /// Time from the HTTP request landing to `ReceivedData` being parsed.
    pub parse_ms: Option<u128>,
    /// Time to fetch the episode list from the media server (network + parse).
    pub episode_fetch_ms: Option<u128>,
    /// Time to write the M3U8 playlist file to disk.
    pub m3u8_write_ms: Option<u128>,
    /// Time from play chain entry to the player process being spawned.
    pub player_spawn_ms: Option<u128>,
    /// Total time from request received to player process running.
    pub total_ms: Option<u128>,
}

impl PlayMetrics {
    /// Create a new, empty metrics record for the given session.
    #[must_use]
    pub fn new(session_id: usize) -> Self {
        Self {
            session_id,
            ..Default::default()
        }
    }

    /// Emit a single structured tracing event summarising all collected spans.
    ///
    /// Call this at the point the player is confirmed running (or on error
    /// after `active_players` is decremented).
    pub fn report(&self) {
        tracing::info!(
            session_id          = self.session_id,
            parse_ms            = ?self.parse_ms,
            episode_fetch_ms    = ?self.episode_fetch_ms,
            m3u8_write_ms       = ?self.m3u8_write_ms,
            player_spawn_ms     = ?self.player_spawn_ms,
            total_ms            = ?self.total_ms,
            "play_metrics"
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_finish_returns_elapsed() {
        let ms = Span::new("test").finish();
        assert!(ms < 1000, "span should finish quickly: {ms}ms");
    }

    #[test]
    fn span_with_session_does_not_panic() {
        let ms = Span::new("test").with_session(42).finish();
        assert!(ms < 1000);
    }

    #[test]
    fn span_elapsed_ms_before_finish() {
        let s = Span::new("query");
        let early = s.elapsed_ms();
        let total = s.finish();
        assert!(total >= early);
    }

    #[test]
    fn play_metrics_report_does_not_panic() {
        let mut m = PlayMetrics::new(1);
        m.parse_ms = Some(10);
        m.episode_fetch_ms = Some(200);
        m.player_spawn_ms = Some(50);
        m.total_ms = Some(300);
        m.report(); // must not panic
    }
}
