//! 60-second conditional batch flush (data.md §5C).
//!
//! §5C: "Backend updates buffer in-memory inside Rust. Updates flush to the
//! Next.js or Flutter Admin UI once every 60 seconds ONLY if underlying
//! data has actually changed, preventing unnecessary client re-renders."
//!
//! ## Design
//!
//! Every state-mutating endpoint pushes a [`FlushMark`] onto an mpsc
//! channel. The marks are buffered in an in-memory ring buffer with a
//! monotonically increasing sequence number. A driver task runs on a
//! 60-second ticker. On each tick the driver:
//!
//! 1. Drains all pending marks from the channel.
//! 2. If the buffer is empty, no-op (skip the DB UPDATE).
//! 3. Otherwise, update `server_synced_at = NOW()` for every distinct row
//!    id in the buffer (collapsing repeated marks), and broadcast a
//!    [`FlushNotice`] to any subscriber (WebSocket sync, Redis pub/sub,
//!    log).
//!
//! The contract is "only flush on change" — the driver never issues a
//! write to the DB when there were no mutations in the last 60 seconds.
//! This keeps the dashboard re-render cheap during quiet periods and
//! preserves the row-level `server_synced_at` semantic from §5C.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast, mpsc};

use uuid::Uuid;

use crate::state::AppState;

/// Named row kinds the flush protocol understands. Adding a new table to
/// §5C means adding an arm here AND a Postgres UPDATE in the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlushKind {
    Incident,
    Resource,
    Center,
}

impl FlushKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlushKind::Incident => "incidents",
            FlushKind::Resource => "resources",
            FlushKind::Center => "centers",
        }
    }

    /// Lower-case identifier used as a Prometheus label. Same spelling as
    /// `as_str` but kept separate so the metric surface can diverge from
    /// the SQL table name without affecting driver logic.
    pub fn kind_label(&self) -> &'static str {
        self.as_str()
    }
}

/// One mutation mark emitted by the rest of the system when a row is
/// inserted or updated. Marks are deduplicated by `(kind, id)` inside the
/// buffer so a row that is mutated five times in a 60-second window still
/// produces exactly one UPDATE on flush.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlushMark {
    pub kind: FlushKind,
    pub id: Uuid,
    pub sequence: u64,
    pub enqueued_at: DateTime<Utc>,
}

impl FlushMark {
    pub fn new(kind: FlushKind, id: Uuid, sequence: u64) -> Self {
        Self {
            kind,
            id,
            sequence,
            enqueued_at: Utc::now(),
        }
    }
}

/// Broadcast to subscribers (e.g. WebSocket sync layer, Redis pub/sub
/// relay) at the end of a flush window. The payload lists the rows whose
/// `server_synced_at` was just updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlushNotice {
    pub flushed_at: DateTime<Utc>,
    pub marks: Vec<FlushMark>,
}

/// Buffer size for the mark channel. Larger than the expected mutation
/// rate in a 60s window by 2 orders of magnitude to absorb bursts; if
/// the channel ever fills we drop on the sender side (mutation writers
/// are best-effort). The driver always drains on its ticker so the
/// channel resets every 60s.
pub const MARK_CHANNEL_CAPACITY: usize = 1024;

/// data.md §5C: flush interval is 60 seconds.
pub const FLUSH_INTERVAL: Duration = Duration::from_secs(60);

/// Construct the mark channel and the broadcast channel that the driver
/// publishes to. The receiver ends are owned by the driver task and any
/// downstream subscribers.
pub fn channels() -> (
    mpsc::Sender<FlushMark>,
    mpsc::Receiver<FlushMark>,
    broadcast::Sender<FlushNotice>,
) {
    let (mark_tx, mark_rx) = mpsc::channel(MARK_CHANNEL_CAPACITY);
    let (notice_tx, _) = broadcast::channel(64);
    (mark_tx, mark_rx, notice_tx)
}

/// Long-running driver task. Ticks every 60s. Drains pending marks, runs
/// the UPDATE on the database if configured, then publishes a
/// [`FlushNotice`] to the broadcast channel.
pub async fn run(
    state: AppState,
    mut marks: mpsc::Receiver<FlushMark>,
    notices: broadcast::Sender<FlushNotice>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let pending: Arc<RwLock<Vec<FlushMark>>> = Arc::new(RwLock::new(Vec::new()));
    let next_sequence = Arc::new(std::sync::atomic::AtomicU64::new(1));

    // Bridge: keep the in-memory `pending` buffer up to date from the
    // mpsc::Receiver without ever holding a lock across an await.
    let pending_for_bridge = Arc::clone(&pending);
    let next_for_bridge = Arc::clone(&next_sequence);
    let state_for_bridge = state.clone();
    let bridge_handle = tokio::spawn(async move {
        while let Some(mark) = marks.recv().await {
            let mut buf = pending_for_bridge.write().await;
            buf.push(mark);
            let _ = next_for_bridge.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = state_for_bridge;
        }
    });

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                flush_once(&state, &pending, &notices).await;
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("flush driver shutting down");
                    break;
                }
            }
        }
    }

    bridge_handle.abort();
    let _ = bridge_handle.await;
}

/// Drain the pending buffer, run the conditional UPDATE, broadcast the
/// notice. The buffer is reset to empty even when no DB is configured so
/// the in-memory mode still respects the "flush on change" semantics.
async fn flush_once(
    state: &AppState,
    pending: &Arc<RwLock<Vec<FlushMark>>>,
    notices: &broadcast::Sender<FlushNotice>,
) {
    let marks: Vec<FlushMark> = {
        let mut buf = pending.write().await;
        std::mem::take(&mut *buf)
    };
    if marks.is_empty() {
        crate::observability::record_flush_tick("noop");
        return;
    }
    // Collapse duplicates: one UPDATE per (kind, id) regardless of how many
    // marks were enqueued in the window.
    let mut seen: HashSet<(FlushKind, Uuid)> = HashSet::new();
    let mut deduped: Vec<FlushMark> = Vec::with_capacity(marks.len());
    for mark in marks.into_iter().rev() {
        if seen.insert((mark.kind, mark.id)) {
            deduped.push(mark);
        }
    }
    let flushed_at = Utc::now();
    if let Some(pool) = &state.database {
        if let Err(error) = apply_postgres(pool, &deduped, flushed_at).await {
            tracing::warn!(
                error = %error,
                "60s conditional flush UPDATE failed; will retry next tick"
            );
            crate::observability::record_flush_tick("failed");
            // Re-queue the marks so the next tick retries them.
            let mut buf = pending.write().await;
            buf.extend(deduped);
            return;
        }
    } else {
        // In-memory mode: stamp the live maps' `server_synced_at` so the
        // spec semantic is preserved even without a database.
        apply_in_memory(state, &deduped, flushed_at).await;
    }
    let deduped_for_log = deduped.clone();
    let notice = FlushNotice {
        flushed_at,
        marks: deduped,
    };
    let _ = notices.send(notice);
    crate::observability::record_flush_tick("applied");
    tracing::debug!(
        count = deduped_for_log.len(),
        "60s conditional flush completed"
    );
}

async fn apply_postgres(
    pool: &sqlx::PgPool,
    marks: &[FlushMark],
    flushed_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for mark in marks {
        sqlx::query(&format!(
            "UPDATE {} SET server_synced_at = $1 WHERE id = $2",
            mark.kind.as_str()
        ))
        .bind(flushed_at)
        .bind(mark.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn apply_in_memory(state: &AppState, marks: &[FlushMark], flushed_at: DateTime<Utc>) {
    for mark in marks {
        match mark.kind {
            FlushKind::Incident => {
                if let Some(incident) = state.incidents.write().await.get_mut(&mark.id) {
                    incident.server_synced_at = Some(flushed_at);
                }
            }
            FlushKind::Resource => {
                if let Some(resource) = state.resources.write().await.get_mut(&mark.id) {
                    resource.server_synced_at = Some(flushed_at);
                }
            }
            FlushKind::Center => {
                if let Some(center) = state.centers.write().await.get_mut(&mark.id) {
                    center.server_synced_at = Some(flushed_at);
                }
            }
        }
    }
}
