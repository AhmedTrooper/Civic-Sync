//! Delta-trigger engine and 30-second background ticker (data.md §5B).
//!
//! data.md §5B requires that the orchestrator re-evaluate the dispatch plan
//! in three situations:
//!
//! 1. A periodic 30-second tick (the operator dashboard expectation).
//! 2. A new incident matching trigger semantics: severity 5 (highest
//!    escalation) OR `casualty_count >= 10` (mass-casualty threshold).
//! 3. A resource status transition to `STUCK` (the resource can no longer
//!    complete its assigned leg and must be re-routed).
//!
//! All three funnel through the same evaluation cycle
//! ([`evaluate_cycle`]): plan via the orchestrator (LLM-enriched) when an
//! LLM is configured, otherwise via the deterministic heuristic, and — in
//! Autopilot mode — commit the plan immediately. Since the orchestrator
//! already owns the §5B 3-tasks/30s semaphore gate, throttling is
//! inherited: a single 30-second tick can absorb any number of triggers
//! fired inside the same window — no LLM call cascade.
//!
//! The driver is spawned once from `main.rs` after the listener is bound.
//! It honours a `tokio::sync::watch::Receiver<bool>` shutdown signal so the
//! server can drain cleanly on SIGINT / SIGTERM.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};

use crate::state::AppState;

/// A signal emitted by the rest of the system when something interesting
/// happens that the trigger driver should react to. The driver is
/// permissive: it always runs on the 30-second tick regardless of events,
/// so payloads are best-effort telemetry.
#[derive(Debug, Clone)]
pub enum TriggerEvent {
    /// A resource transitioned to `STUCK` (data.md §5B delta condition).
    ResourceStuck(uuid::Uuid),
    /// A new incident was created with severity 5 (max escalation).
    NewSeverityFiveIncident(uuid::Uuid),
    /// A new incident was created with `casualty_count >= 10`.
    NewMassCasualtyIncident(uuid::Uuid),
    /// An existing incident's `casualty_count` jumped by at least 10
    /// (data.md §5B intra-incident delta).
    DeltaCasualtyBurst { incident_id: uuid::Uuid, delta: u32 },
    /// An existing incident was escalated from below 5 to severity 5
    /// (data.md §5B intra-incident delta).
    SeverityEscalatedToFive(uuid::Uuid),
}

/// Buffer size for the trigger event channel. Bigger than 1 so we don't
/// drop signals during bursty periods; the docs note that callers must
/// treat the channel as advisory — the 30s tick is the safety net.
const CHANNEL_CAPACITY: usize = 64;

/// Data.md §5B: dispatch re-evaluates on a 30-second interval.
pub const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Construct a trigger event channel pair. The receiver is owned by the
/// driver task; the sender is stored on `AppState` for other modules to
/// fire events through.
pub fn channel() -> (mpsc::Sender<TriggerEvent>, mpsc::Receiver<TriggerEvent>) {
    mpsc::channel(CHANNEL_CAPACITY)
}

/// Long-running driver task. Tick → call orchestrator → reset the gate.
/// Shutdown is signalled by setting the watch channel to `true`.
pub async fn run(
    state: AppState,
    mut events: mpsc::Receiver<TriggerEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(TICK_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let orchestrator = Arc::clone(&state.orchestrator);
    if orchestrator.is_none() {
        tracing::info!("triggers driver running in heuristic-only mode (no orchestrator)");
    }
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                run_tick(&state, &orchestrator).await;
            }
            Some(event) = events.recv() => {
                run_event(&state, &orchestrator, event).await;
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("triggers driver shutting down");
                    break;
                }
            }
        }
    }
}

/// Handle a single ticker fire. Always resets the gate so the next window
/// starts clean, then re-evaluates the dispatch plan. With an orchestrator
/// the LLM enrichment path runs; without one the deterministic heuristic
/// keeps the grid moving (data.md §7 fallback contract). In Autopilot mode
/// the resulting plan is committed immediately — that is the §5D
/// "Autopilot Mode" — otherwise the plan is computed hot so
/// `POST /dispatch/recommendations` and the operator dashboard see fresh
/// envelopes.
async fn run_tick(
    state: &AppState,
    orchestrator: &Arc<Option<crate::features::orchestrator::Orchestrator>>,
) {
    if let Some(orch) = orchestrator.as_ref() {
        orch.gate.reset_window().await;
    }
    evaluate_cycle(state, orchestrator, "tick").await;
}

/// Handle a discrete trigger event. The 30-second tick is the authoritative
/// re-evaluation point — events just make the reaction immediate: the gate
/// is reset so this event is never throttled away, and a fresh dispatch
/// cycle runs right now.
async fn run_event(
    state: &AppState,
    orchestrator: &Arc<Option<crate::features::orchestrator::Orchestrator>>,
    event: TriggerEvent,
) {
    // Always reset the gate so this event isn't dropped by the rolling
    // window. Without this, a STUCK event late in the window would be
    // disabled from re-dispatching until the next tick reset.
    if let Some(orch) = orchestrator.as_ref() {
        orch.gate.reset_window().await;
    }
    let event_label = match &event {
        TriggerEvent::ResourceStuck(id) => {
            tracing::info!(resource_id = %id, "trigger: resource STUCK");
            "resource_stuck"
        }
        TriggerEvent::NewSeverityFiveIncident(id) => {
            tracing::info!(incident_id = %id, "trigger: severity 5 incident");
            "incident_severity_five"
        }
        TriggerEvent::NewMassCasualtyIncident(id) => {
            tracing::info!(incident_id = %id, "trigger: mass-casualty incident");
            "incident_mass_casualty"
        }
        TriggerEvent::DeltaCasualtyBurst { incident_id, delta } => {
            tracing::info!(
                incident_id = %incident_id,
                delta = delta,
                "trigger: Δcasualty ≥ 10 burst"
            );
            "incident_delta_casualty"
        }
        TriggerEvent::SeverityEscalatedToFive(id) => {
            tracing::info!(incident_id = %id, "trigger: severity escalated to 5");
            "incident_severity_escalated"
        }
    };
    evaluate_cycle(state, orchestrator, event_label).await;
}

/// One dispatch evaluation cycle shared by the ticker and the event path.
/// Plans via the orchestrator (LLM-enriched) when configured, otherwise via
/// the deterministic heuristic, then commits the plan when Autopilot mode
/// is enabled. Errors are logged, never fatal: the next tick is the safety
/// net.
async fn evaluate_cycle(
    state: &AppState,
    orchestrator: &Arc<Option<crate::features::orchestrator::Orchestrator>>,
    context: &str,
) {
    let envelopes = match orchestrator.as_ref() {
        Some(orch) => match orch.dispatch(state).await {
            Ok(envelopes) => envelopes,
            Err(error) => {
                tracing::warn!(error = %error, context, "dispatch planning failed");
                return;
            }
        },
        None => match crate::features::dispatch::heuristic_dispatch(state).await {
            Ok(envelopes) => envelopes,
            Err(error) => {
                tracing::warn!(error = %error, context, "heuristic planning failed");
                return;
            }
        },
    };
    if envelopes.is_empty() {
        tracing::debug!(context, "dispatch cycle: nothing to plan");
        return;
    }
    if !state.config.autopilot {
        tracing::debug!(
            context,
            envelopes = envelopes.len(),
            "dispatch cycle planned (human-in-the-loop: awaiting /dispatch/apply)"
        );
        return;
    }
    match crate::features::dispatch::apply_envelopes(state, &envelopes).await {
        Ok(summaries) => {
            let attached: usize = summaries.iter().map(|s| s.resources_attached).sum();
            tracing::info!(
                context,
                envelopes = summaries.len(),
                resources_attached = attached,
                "autopilot dispatch cycle applied"
            );
        }
        Err(error) => {
            tracing::warn!(error = %error, context, "autopilot apply failed");
        }
    }
}
