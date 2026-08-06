//! Multi-center dispatch engine (data.md §5A/§5B + §7).
//!
//! This module is the operational heart of CivicSync. It turns the live
//! incident/resource/center state into structured
//! `dispatch_multi_center_response` tool-call envelopes and can commit
//! those envelopes back to the store.
//!
//! ## Planning ([`heuristic_dispatch`])
//!
//! 1. **Dynamic prioritization** — ACTIVE incidents are ranked by
//!    [`crate::features::incidents::priority_score`] (severity 1–5 base
//!    weight, casualty load, affected population, and time-on-grid so
//!    stale crises escalate), highest first.
//! 2. **Multi-center optimization** — for each incident the engine takes
//!    up to [`MAX_ASSETS_PER_INCIDENT`] assets: first from the incident's
//!    primary (nearest) hub, then from the Dhaka Core center as the
//!    national fallback, then from any other hub by Haversine distance.
//! 3. **Conflict prevention** — a resource claimed by an earlier,
//!    higher-priority incident in the same planning cycle is never
//!    offered again ([`std::collections::HashSet`] in planning,
//!    `assigned_incident_id IS NULL` row guard on apply).
//! 4. **Explainability** — every envelope carries a human-readable
//!    justification built from the same numbers the engine used, so the
//!    decision is auditable even when no LLM is configured. When an LLM
//!    is wired up the orchestrator refines this text but can never change
//!    the allocation itself.
//!
//! ## Committing ([`apply_envelopes`])
//!
//! Apply links each allocated resource to the incident (status EN_ROUTE,
//! remaining distance stamped), then flips the incident to DISPATCHED.
//! Idempotent: incidents that are no longer ACTIVE and resources that are
//! already assigned are skipped and reported, never double-committed.

use std::collections::{HashMap, HashSet};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    features::{
        centers::Center,
        flush::{FlushKind, FlushMark},
        incidents::{Incident, IncidentStatus, priority_reasons, priority_score},
        resources::{Resource, ResourceStatus},
    },
    state::AppState,
};

pub const TOOL_NAME: &str = "dispatch_multi_center_response";

/// Maximum assets the engine commits to a single incident per planning
/// cycle. Keeps coverage broad across the grid instead of letting one
/// crisis drain a hub.
pub const MAX_ASSETS_PER_INCIDENT: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEnvelope {
    pub tool_name: String,
    pub arguments: DispatchArguments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchArguments {
    pub incident_id: Uuid,
    pub primary_center_id: Uuid,
    pub allocations: Vec<AllocationEntry>,
    pub resource_state_modifications: Vec<ResourceStateModification>,
    pub justification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationEntry {
    pub center_name: String,
    pub resource_id: Uuid,
    pub distance_km: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStateModification {
    pub resource_id: Uuid,
    pub new_status: String,
    pub reason: String,
}

/// One row of the explainable priority queue surfaced by
/// `POST /api/v1/dispatch/recommendations` so operators can see *why*
/// the engine plans in this order.
#[derive(Debug, Clone, Serialize)]
pub struct PriorityQueueEntry {
    pub incident_id: Uuid,
    pub title: String,
    pub severity_level: u8,
    pub casualty_count: u32,
    pub priority_score: f64,
    pub reasons: Vec<String>,
}

/// `POST /api/v1/dispatch/recommendations` — the AI orchestration
/// showcase. Runs the LLM-enriched orchestrator when configured,
/// otherwise the deterministic heuristic, and returns the structured
/// tool-call envelopes plus the explainable priority queue. Read-only:
/// nothing is committed until `POST /dispatch/apply` (or Autopilot).
pub async fn recommend(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (envelopes, mode, provider, model) = match state.orchestrator.as_ref() {
        Some(orchestrator) => {
            let envelopes = orchestrator.dispatch(&state).await?;
            (
                envelopes,
                "llm".to_string(),
                Some(orchestrator.provider.id().to_string()),
                Some(orchestrator.model.clone()),
            )
        }
        None => (
            heuristic_dispatch(&state).await?,
            "heuristic".to_string(),
            None,
            None,
        ),
    };
    let queue = priority_queue(&state).await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "generated_at": Utc::now(),
            "mode": mode,
            "provider": provider,
            "model": model,
            "queue": queue,
            "recommendations": envelopes,
        })),
    ))
}

/// `POST /api/v1/dispatch/smoke` — diagnostic pipeline walker. Shows the
/// heuristic plan, whether an LLM was attempted, and the raw patches the
/// model returned, so a demo viewer can verify the AI path end-to-end.
pub async fn smoke(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = match state.orchestrator.as_ref() {
        Some(orchestrator) => orchestrator.smoke(&state).await?,
        None => crate::features::orchestrator::SmokeResult {
            heuristic: heuristic_dispatch(&state).await?,
            llm_attempted: false,
            llm_patches: Vec::new(),
            llm_error: None,
        },
    };
    let provider = state
        .orchestrator
        .as_ref()
        .as_ref()
        .map(|orchestrator| orchestrator.provider.id().to_string());
    let model = state
        .orchestrator
        .as_ref()
        .as_ref()
        .map(|orchestrator| orchestrator.model.clone());
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "generated_at": Utc::now(),
            "provider": provider,
            "model": model,
            "llm_attempted": result.llm_attempted,
            "llm_error": result.llm_error,
            "llm_patches": result.llm_patches,
            "recommendations": result.heuristic,
        })),
    ))
}

/// `POST /api/v1/dispatch/apply` — human-in-the-loop approval. Commits
/// one operator-reviewed envelope: resources get linked + flipped to
/// EN_ROUTE, the incident flips to DISPATCHED. Conflict-safe and
/// idempotent (see module docs).
pub async fn apply(
    State(state): State<AppState>,
    Json(envelope): Json<ToolCallEnvelope>,
) -> Result<impl IntoResponse, ApiError> {
    let summaries = apply_envelopes(&state, std::slice::from_ref(&envelope)).await?;
    match summaries.into_iter().next() {
        Some(summary) => Ok((StatusCode::OK, Json(summary)).into_response()),
        None => Err(ApiError::NotFound),
    }
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = ((d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2))
    .clamp(0.0, 1.0);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

/// Load the full working set (incidents, resources, centers) from either
/// Postgres or the in-memory maps behind one signature.
async fn load_world(
    state: &AppState,
) -> Result<(Vec<Incident>, Vec<Resource>, Vec<Center>), ApiError> {
    let incidents = match &state.database {
        Some(pool) => crate::features::incidents::list_all_postgres(pool).await?,
        None => state.incidents.read().await.values().cloned().collect(),
    };
    let resources = match &state.database {
        Some(pool) => crate::features::resources::list_all_postgres(pool).await?,
        None => state.resources.read().await.values().cloned().collect(),
    };
    let centers = match &state.database {
        Some(pool) => crate::features::centers::list_all_postgres(pool).await?,
        None => state.centers.read().await.values().cloned().collect(),
    };
    Ok((incidents, resources, centers))
}

/// The explainable priority queue: ACTIVE incidents ranked by
/// [`priority_score`], highest first, with the per-incident reasons a
/// reviewer can read verbatim.
pub(crate) async fn priority_queue(state: &AppState) -> Result<Vec<PriorityQueueEntry>, ApiError> {
    let (incidents, _resources, _centers) = load_world(state).await?;
    let mut active: Vec<&Incident> = incidents
        .iter()
        .filter(|incident| incident.status == IncidentStatus::Active)
        .collect();
    active.sort_by(|a, b| {
        priority_score(b)
            .partial_cmp(&priority_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.created_at.cmp(&b.created_at))
    });
    Ok(active
        .into_iter()
        .take(20)
        .map(|incident| PriorityQueueEntry {
            incident_id: incident.id,
            title: incident.title.clone(),
            severity_level: incident.severity_level,
            casualty_count: incident.casualty_count,
            priority_score: priority_score(incident),
            reasons: priority_reasons(incident),
        })
        .collect())
}

/// The deterministic multi-center dispatch planner. Also the source of
/// truth the orchestrator enriches (LLM may only rewrite justifications).
pub(crate) async fn heuristic_dispatch(
    state: &AppState,
) -> Result<Vec<ToolCallEnvelope>, ApiError> {
    let (incidents, resources, centers) = load_world(state).await?;
    let centers_by_id: HashMap<Uuid, &Center> =
        centers.iter().map(|center| (center.id, center)).collect();

    // Dynamic prioritization: highest score plans first, so scarce assets
    // always go to the most urgent crisis.
    let mut active: Vec<&Incident> = incidents
        .iter()
        .filter(|incident| incident.status == IncidentStatus::Active)
        .collect();
    active.sort_by(|a, b| {
        priority_score(b)
            .partial_cmp(&priority_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.created_at.cmp(&b.created_at))
    });

    // Deployable pool: not already committed and not terminal. STUCK
    // assets with no assignment stay eligible — that is exactly the
    // re-route case the §5B delta trigger exists for.
    let deployable: Vec<&Resource> = resources
        .iter()
        .filter(|resource| {
            resource.assigned_incident_id.is_none()
                && !matches!(
                    resource.status,
                    ResourceStatus::Completed | ResourceStatus::Rejected
                )
        })
        .collect();

    let mut claimed: HashSet<Uuid> = HashSet::new();
    let mut envelopes = Vec::new();

    for incident in active {
        if claimed.len() >= deployable.len() {
            break; // grid exhausted
        }
        let mut candidates: Vec<(&Resource, f64)> = deployable
            .iter()
            .filter(|resource| !claimed.contains(&resource.id))
            .map(|resource| {
                let distance = haversine_distance(
                    incident.latitude,
                    incident.longitude,
                    resource.latitude,
                    resource.longitude,
                );
                (*resource, distance)
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }

        // Multi-center layering: (1) primary hub assets, (2) Dhaka Core
        // national fallback, (3) any other hub by distance. Sorting each
        // phase by Haversine distance keeps routing optimal per layer.
        let mut primary_layer: Vec<_> = candidates
            .iter()
            .filter(|(resource, _)| resource.owner_center_id == incident.primary_center_id)
            .copied()
            .collect();
        let mut core_layer: Vec<_> = candidates
            .iter()
            .filter(|(resource, _)| {
                resource.owner_center_id != incident.primary_center_id
                    && centers_by_id
                        .get(&resource.owner_center_id)
                        .is_some_and(|center| center.is_core_center)
            })
            .copied()
            .collect();
        let mut regional_layer: Vec<_> = candidates
            .iter()
            .filter(|(resource, _)| {
                resource.owner_center_id != incident.primary_center_id
                    && !centers_by_id
                        .get(&resource.owner_center_id)
                        .is_some_and(|center| center.is_core_center)
            })
            .copied()
            .collect();
        for layer in [&mut primary_layer, &mut core_layer, &mut regional_layer] {
            layer.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        }
        candidates.clear();

        let mut picked: Vec<(&Resource, f64)> = Vec::with_capacity(MAX_ASSETS_PER_INCIDENT);
        for layer in [primary_layer, core_layer, regional_layer] {
            for candidate in layer {
                if picked.len() == MAX_ASSETS_PER_INCIDENT {
                    break;
                }
                picked.push(candidate);
            }
        }
        if picked.is_empty() {
            continue;
        }

        let primary_name = centers_by_id
            .get(&incident.primary_center_id)
            .map(|center| center.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let allocations: Vec<AllocationEntry> = picked
            .iter()
            .map(|(resource, distance)| AllocationEntry {
                center_name: centers_by_id
                    .get(&resource.owner_center_id)
                    .map(|center| center.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                resource_id: resource.id,
                distance_km: (*distance * 10.0).round() / 10.0,
            })
            .collect();

        let justification = build_justification(
            incident,
            &primary_name,
            &allocations,
            incident_score(incident),
        );
        for (resource, _) in &picked {
            claimed.insert(resource.id);
        }

        envelopes.push(ToolCallEnvelope {
            tool_name: TOOL_NAME.to_string(),
            arguments: DispatchArguments {
                incident_id: incident.id,
                primary_center_id: incident.primary_center_id,
                allocations,
                resource_state_modifications: Vec::new(),
                justification,
            },
        });
    }

    Ok(envelopes)
}

fn incident_score(incident: &Incident) -> f64 {
    (priority_score(incident) * 10.0).round() / 10.0
}

/// Explainable-AI justification assembled from the engine's own numbers.
/// Deterministic and auditable; the LLM path may refine the wording but
/// never the allocation.
fn build_justification(
    incident: &Incident,
    primary_name: &str,
    allocations: &[AllocationEntry],
    score: f64,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!(
        "Incident '{}' (severity {}/5, {} casualties, {} affected) ranks at priority {:.1}; primary hub is {}.",
        incident.title,
        incident.severity_level,
        incident.casualty_count,
        incident.affected_people,
        score,
        primary_name
    ));

    let mut local = 0usize;
    let mut fallbacks: Vec<String> = Vec::new();
    for allocation in allocations {
        if allocation.center_name == primary_name {
            local += 1;
        } else {
            fallbacks.push(format!(
                "{} from {} ({} km)",
                allocation.resource_id, allocation.center_name, allocation.distance_km
            ));
        }
    }
    parts.push(format!(
        "{} asset(s) dispatched from {}.",
        local, primary_name
    ));
    if !fallbacks.is_empty() {
        parts.push(format!(
            "Regional deficit covered by multi-center fallback: {}.",
            fallbacks.join("; ")
        ));
    }
    parts.push(
        "All distances are Haversine great-circle km; no asset is double-allocated in this cycle."
            .to_string(),
    );
    parts.join(" ")
}

/// Per-envelope commit report surfaced by `POST /dispatch/apply` and the
/// Autopilot driver.
#[derive(Debug, Clone, Serialize)]
pub struct ApplySummary {
    pub incident_id: Uuid,
    pub status: String,
    pub resources_attached: usize,
    pub conflicts_skipped: usize,
    pub applied_at: DateTime<Utc>,
}

/// Commit envelopes to the store. Conflict-safe (`assigned_incident_id
/// IS NULL` guard) and idempotent (non-ACTIVE incidents are skipped).
/// Shared by the human-in-the-loop `apply` handler and the 30-second
/// Autopilot driver in `triggers.rs`.
pub(crate) async fn apply_envelopes(
    state: &AppState,
    envelopes: &[ToolCallEnvelope],
) -> Result<Vec<ApplySummary>, ApiError> {
    let mut summaries = Vec::with_capacity(envelopes.len());
    for envelope in envelopes {
        let summary = match &state.database {
            Some(pool) => apply_postgres(pool, state, envelope).await?,
            None => apply_in_memory(state, envelope).await?,
        };
        summaries.push(summary);
    }
    Ok(summaries)
}

async fn apply_postgres(
    pool: &sqlx::PgPool,
    state: &AppState,
    envelope: &ToolCallEnvelope,
) -> Result<ApplySummary, ApiError> {
    let incident_id = envelope.arguments.incident_id;
    let now = Utc::now();

    let mut tx = pool.begin().await?;
    let status: Option<(String,)> =
        sqlx::query_as("SELECT status FROM incidents WHERE id = $1 FOR UPDATE")
            .bind(incident_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((status,)) = status else {
        return Ok(ApplySummary {
            incident_id,
            status: "incident_not_found".to_string(),
            resources_attached: 0,
            conflicts_skipped: 0,
            applied_at: now,
        });
    };
    if status != "ACTIVE" {
        // Already dispatched/resolved by an earlier cycle or an operator:
        // never double-commit.
        return Ok(ApplySummary {
            incident_id,
            status: "skipped_not_active".to_string(),
            resources_attached: 0,
            conflicts_skipped: 0,
            applied_at: now,
        });
    }

    let mut attached = 0usize;
    let mut conflicts = 0usize;
    let mut attached_ids: Vec<Uuid> = Vec::new();
    for allocation in &envelope.arguments.allocations {
        let rows = crate::features::resources::dispatch_link_postgres(
            &mut tx,
            allocation.resource_id,
            incident_id,
            ResourceStatus::EnRoute,
            allocation.distance_km,
            now,
        )
        .await?;
        if rows > 0 {
            attached += 1;
            attached_ids.push(allocation.resource_id);
        } else {
            conflicts += 1;
        }
    }

    if attached > 0 {
        sqlx::query("UPDATE incidents SET status = 'DISPATCHED', updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(incident_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    state.enqueue_flush(FlushMark::new(FlushKind::Incident, incident_id, 0));
    for resource_id in attached_ids {
        state.enqueue_flush(FlushMark::new(FlushKind::Resource, resource_id, 0));
    }

    Ok(ApplySummary {
        incident_id,
        status: if attached > 0 {
            "applied".to_string()
        } else {
            "no_assets_available".to_string()
        },
        resources_attached: attached,
        conflicts_skipped: conflicts,
        applied_at: now,
    })
}

async fn apply_in_memory(
    state: &AppState,
    envelope: &ToolCallEnvelope,
) -> Result<ApplySummary, ApiError> {
    let incident_id = envelope.arguments.incident_id;
    let now = Utc::now();

    let is_active = {
        let incidents = state.incidents.read().await;
        incidents
            .get(&incident_id)
            .map(|incident| incident.status == IncidentStatus::Active)
    };
    let Some(true) = is_active else {
        return Ok(ApplySummary {
            incident_id,
            status: match is_active {
                None => "incident_not_found".to_string(),
                _ => "skipped_not_active".to_string(),
            },
            resources_attached: 0,
            conflicts_skipped: 0,
            applied_at: now,
        });
    };

    let mut attached = 0usize;
    let mut conflicts = 0usize;
    {
        let mut resources = state.resources.write().await;
        for allocation in &envelope.arguments.allocations {
            match resources.get_mut(&allocation.resource_id) {
                Some(resource) if resource.assigned_incident_id.is_none() => {
                    resource.status = ResourceStatus::EnRoute;
                    resource.assigned_incident_id = Some(incident_id);
                    resource.distance_remaining_km = allocation.distance_km;
                    resource.updated_at = now;
                    attached += 1;
                }
                _ => conflicts += 1,
            }
        }
    }

    if attached > 0 {
        let mut incidents = state.incidents.write().await;
        if let Some(incident) = incidents.get_mut(&incident_id) {
            incident.status = IncidentStatus::Dispatched;
            incident.updated_at = now;
        }
    }

    state.enqueue_flush(FlushMark::new(FlushKind::Incident, incident_id, 0));

    Ok(ApplySummary {
        incident_id,
        status: if attached > 0 {
            "applied".to_string()
        } else {
            "no_assets_available".to_string()
        },
        resources_attached: attached,
        conflicts_skipped: conflicts,
        applied_at: now,
    })
}
