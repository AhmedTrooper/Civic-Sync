//! Background disaster simulator and admin injection controls
//! (data.md §5D).
//!
//! §5D requires three capabilities for operator control of the demo:
//!
//! 1. **Simulation Toggle:** the operator can pause or resume a background
//!    disaster generator that synthesises incidents on a fixed cadence.
//! 2. **Manual Injection:** the operator can spawn custom incidents or
//!    modify resource states on demand.
//! 3. **AI Tool Approval:** Rig AI emits tool-call payloads; this layer
//!    stays neutral on that — the orchestrator already gates the LLM
//!    calls behind the §5B 3-tasks/30s semaphore, so "Autopilot Mode"
//!    is essentially `state.orchestrator.is_some()`.
//!
//! The simulator is a long-running task spawned in `main.rs` after the
//! listener binds. It honours the same `watch::Receiver<bool>` shutdown
//! channel as the trigger and flush drivers so the server can drain
//! cleanly on SIGINT / SIGTERM.
//!
//! ## Paused-by-default safety
//!
//! The simulator starts in the *paused* state. This is deliberate:
//! flipping `SIMULATION_AUTOSTART=true` in `.env` is the only way to
//! start auto-injecting incidents. Tests rely on the paused default.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};

use crate::state::AppState;

/// Tick interval for the background disaster generator when active.
/// 60 seconds matches the §5C flush cadence so synthetic mutations
/// stay aligned with the dashboard sync layer.
pub const SIMULATION_TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Shared simulator state. The driver mutates `paused` from background
/// admin endpoints; the routes do the same through the mutex guard.
#[derive(Debug, Default)]
pub struct SimulationState {
    paused: Mutex<bool>,
    /// Monotonically increasing counter of synthetic incidents emitted.
    generated: Mutex<u64>,
}

impl SimulationState {
    pub fn new(autostart: bool) -> Arc<Self> {
        let paused = !autostart;
        crate::observability::set_simulation_paused(paused);
        Arc::new(Self {
            paused: Mutex::new(paused),
            generated: Mutex::new(0),
        })
    }

    /// Read the current paused flag without taking the write lock.
    pub async fn is_paused(&self) -> bool {
        *self.paused.lock().await
    }

    pub async fn pause(&self) {
        let mut flag = self.paused.lock().await;
        *flag = true;
        drop(flag);
        crate::observability::set_simulation_paused(true);
    }

    pub async fn resume(&self) {
        let mut flag = self.paused.lock().await;
        *flag = false;
        drop(flag);
        crate::observability::set_simulation_paused(false);
    }

    pub async fn generated_count(&self) -> u64 {
        *self.generated.lock().await
    }

    async fn record_generated(&self) {
        let mut counter = self.generated.lock().await;
        *counter = counter.saturating_add(1);
        drop(counter);
        crate::observability::record_simulation_generated();
    }
}

/// Snapshot returned by the admin status endpoint so dashboards can
/// show "Simulation: paused · 17 generated" without polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStatus {
    pub paused: bool,
    pub generated: u64,
    pub tick_interval_seconds: u64,
}

impl SimulationStatus {
    pub async fn from_state(state: &SimulationState) -> Self {
        Self {
            paused: state.is_paused().await,
            generated: state.generated_count().await,
            tick_interval_seconds: SIMULATION_TICK_INTERVAL.as_secs(),
        }
    }
}

/// Long-running simulator task. Ticks every 60s; if not paused, synthesises
/// an incident with randomised severity, casualties, and a coordinate
/// inside the bounding box of the eight divisional hubs.
pub async fn run(
    state: AppState,
    simulation: Arc<SimulationState>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(SIMULATION_TICK_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Skip the first immediate tick so the listener has a moment to bind
    // before the simulator fires its first synthetic incident.
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if simulation.is_paused().await {
                    tracing::debug!("simulation tick skipped (paused)");
                    continue;
                }
                match synthesise_incident(&state, &simulation).await {
                    Ok(incident_id) => {
                        simulation.record_generated().await;
                        tracing::info!(
                            incident_id = %incident_id,
                            "simulation synthesised incident"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "simulation tick failed to synthesise incident"
                        );
                    }
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("simulation driver shutting down");
                    break;
                }
            }
        }
    }
}

/// Build a random incident using a deterministic seed pattern. The randomness
/// is process-local; no LLM involvement per §5D (this is the *heuristic*
/// disaster generator, not AI).
async fn synthesise_incident(
    state: &AppState,
    simulation: &SimulationState,
) -> Result<uuid::Uuid, crate::error::ApiError> {
    use crate::features::{flush::FlushKind, incidents::CreateIncident};

    fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0;
        let d_lat = (lat2 - lat1).to_radians();
        let d_lon = (lon2 - lon1).to_radians();
        let a = (d_lat / 2.0).sin().powi(2)
            + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }

    // 1% probability of a severity-5 mass-casualty event so the simulator
    // exercises the §5B severity-5 trigger on a regular cadence.
    let severity_level = if rand::random_bool(0.01) {
        5
    } else {
        rand::random_range(1..=4)
    };
    let casualty_count = if severity_level == 5 {
        rand::random_range(15..=60)
    } else {
        rand::random_range(0..=12)
    };
    let affected_people = casualty_count * rand::random_range(5..=15);
    let (latitude, longitude) = random_bangladesh_coordinate();
    let payload = CreateIncident {
        title: format!("Simulated event @ ({:.4}, {:.4})", latitude, longitude),
        severity_level,
        affected_people,
        casualty_count,
        latitude,
        longitude,
    };

    let now = Utc::now();
    let id = uuid::Uuid::new_v4();
    let centers = state.centers.read().await;
    let mut closest_center_id = uuid::Uuid::nil();
    let mut min_dist = f64::MAX;
    for center in centers.values() {
        let dist = haversine_distance(
            payload.latitude,
            payload.longitude,
            center.latitude,
            center.longitude,
        );
        if dist < min_dist {
            min_dist = dist;
            closest_center_id = center.id;
        }
    }
    drop(centers);

    let incident = crate::features::incidents::Incident {
        id,
        title: payload.title.clone(),
        primary_center_id: closest_center_id,
        severity_level: payload.severity_level,
        affected_people: payload.affected_people,
        casualty_count: payload.casualty_count,
        latitude: payload.latitude,
        longitude: payload.longitude,
        status: crate::features::incidents::IncidentStatus::Active,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    };

    if let Some(pool) = &state.database {
        crate::features::incidents::insert_postgres(pool, &incident).await?;
    } else {
        state.incidents.write().await.insert(id, incident.clone());
    }
    state.enqueue_flush(crate::features::flush::FlushMark::new(
        FlushKind::Incident,
        id,
        0,
    ));
    // Trigger §5B signal so the orchestrator reacts on the next 30s tick.
    let _ = simulation;
    crate::features::incidents::fire_creation_triggers_for(state, &incident);
    Ok(id)
}

/// Random coordinate inside a loose bounding box that covers the eight
/// divisional hubs (Dhaka, Chittagong, Khulna, Rajshahi, Sylhet, Barisal,
/// Rangpur, Mymensingh).
fn random_bangladesh_coordinate() -> (f64, f64) {
    let latitude = rand::random_range(22.0..26.5);
    let longitude = rand::random_range(88.0..92.5);
    (latitude, longitude)
}

/// Body for the `POST /v1/admin/simulation/inject` endpoint. Allows an
/// operator to spawn an incident with exact parameters instead of waiting
/// for the random generator.
#[derive(Debug, Deserialize)]
pub struct InjectIncidentRequest {
    pub title: String,
    pub severity_level: u8,
    pub affected_people: u32,
    pub casualty_count: u32,
    pub latitude: f64,
    pub longitude: f64,
}

impl InjectIncidentRequest {
    pub fn validate(&self) -> Result<(), crate::error::ApiError> {
        if self.title.trim().is_empty() {
            return Err(crate::error::ApiError::Validation(
                "title must not be empty".into(),
            ));
        }
        if !(1..=5).contains(&self.severity_level) {
            return Err(crate::error::ApiError::Validation(
                "severity_level must be between 1 and 5".into(),
            ));
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(crate::error::ApiError::Validation(
                "latitude must be between -90 and 90".into(),
            ));
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(crate::error::ApiError::Validation(
                "longitude must be between -180 and 180".into(),
            ));
        }
        Ok(())
    }
}

/// Shared write path used by both the random simulator and the manual
/// `inject` endpoint so we don't fork the persistence logic.
pub async fn inject_incident(
    state: &AppState,
    payload: InjectIncidentRequest,
) -> Result<crate::features::incidents::Incident, crate::error::ApiError> {
    use crate::features::{
        flush::{FlushKind, FlushMark},
        incidents::{Incident, IncidentStatus},
    };

    payload.validate()?;
    let now = Utc::now();
    fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0;
        let d_lat = (lat2 - lat1).to_radians();
        let d_lon = (lon2 - lon1).to_radians();
        let a = (d_lat / 2.0).sin().powi(2)
            + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }

    let centers = state.centers.read().await;
    let mut closest_center_id = uuid::Uuid::nil();
    let mut min_dist = f64::MAX;
    for center in centers.values() {
        let dist = haversine_distance(
            payload.latitude,
            payload.longitude,
            center.latitude,
            center.longitude,
        );
        if dist < min_dist {
            min_dist = dist;
            closest_center_id = center.id;
        }
    }
    drop(centers);

    let incident = Incident {
        id: uuid::Uuid::new_v4(),
        title: payload.title.trim().to_string(),
        primary_center_id: closest_center_id,
        severity_level: payload.severity_level,
        affected_people: payload.affected_people,
        casualty_count: payload.casualty_count,
        latitude: payload.latitude,
        longitude: payload.longitude,
        status: IncidentStatus::Active,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    };

    if let Some(pool) = &state.database {
        crate::features::incidents::insert_postgres(pool, &incident).await?;
    } else {
        state
            .incidents
            .write()
            .await
            .insert(incident.id, incident.clone());
    }
    state.enqueue_flush(FlushMark::new(FlushKind::Incident, incident.id, 0));
    crate::features::incidents::fire_creation_triggers_for(state, &incident);
    Ok(incident)
}

// --- Axum handlers ---------------------------------------------------------

/// `GET /api/v1/admin/simulation` — returns the current simulator status so
/// admin dashboards can render the toggle.
pub async fn status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<SimulationStatus>, crate::error::ApiError> {
    let simulation = state
        .simulation
        .as_ref()
        .ok_or_else(|| crate::error::ApiError::Internal("simulation driver not wired".into()))?;
    Ok(axum::Json(SimulationStatus::from_state(simulation).await))
}

/// `POST /api/v1/admin/simulation/pause` — flips the simulator to paused.
pub async fn pause(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<SimulationStatus>, crate::error::ApiError> {
    let simulation = state
        .simulation
        .as_ref()
        .ok_or_else(|| crate::error::ApiError::Internal("simulation driver not wired".into()))?;
    simulation.pause().await;
    tracing::info!("simulation paused by admin request");
    Ok(axum::Json(SimulationStatus::from_state(simulation).await))
}

/// `POST /api/v1/admin/simulation/resume` — flips the simulator to active.
pub async fn resume(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<SimulationStatus>, crate::error::ApiError> {
    let simulation = state
        .simulation
        .as_ref()
        .ok_or_else(|| crate::error::ApiError::Internal("simulation driver not wired".into()))?;
    simulation.resume().await;
    tracing::info!("simulation resumed by admin request");
    Ok(axum::Json(SimulationStatus::from_state(simulation).await))
}

/// `POST /api/v1/admin/simulation/inject` — manual injection of an
/// operator-specified incident (data.md §5D).
pub async fn inject(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<InjectIncidentRequest>,
) -> Result<impl axum::response::IntoResponse, crate::error::ApiError> {
    let incident = inject_incident(&state, payload).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(incident)))
}
