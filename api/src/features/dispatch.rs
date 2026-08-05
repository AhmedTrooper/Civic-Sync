use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    features::{incidents::IncidentStatus, resources::Resource},
    state::AppState,
};

pub const TOOL_NAME: &str = "dispatch_multi_center_response";

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

pub async fn recommend(State(_state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "generated_at": Utc::now(),
            "recommendations": Vec::<ToolCallEnvelope>::new(),
        })),
    ))
}

pub async fn smoke(State(_state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "generated_at": Utc::now(),
            "provider": null,
            "model": null,
            "llm_attempted": false,
            "llm_error": null,
            "llm_patches": [],
            "recommendations": Vec::<ToolCallEnvelope>::new(),
        })),
    ))
}

pub async fn apply(
    State(_state): State<AppState>,
    Json(envelope): Json<ToolCallEnvelope>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "envelope_incident_id": envelope.arguments.incident_id,
            "applied_at": Utc::now(),
            "resources_attached": envelope.arguments.allocations.len(),
            "teams_deployed": 0,
            "status": "applied",
        })),
    ))
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

pub(crate) async fn heuristic_dispatch(
    state: &AppState,
) -> Result<Vec<ToolCallEnvelope>, ApiError> {
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

    let mut envelopes = Vec::new();

    for incident in incidents
        .iter()
        .filter(|i| i.status == IncidentStatus::Active)
    {
        let mut available_resources: Vec<&Resource> = resources
            .iter()
            .filter(|r| r.assigned_incident_id.is_none())
            .collect();

        if available_resources.is_empty() {
            continue;
        }

        available_resources.sort_by(|a, b| {
            let dist_a = haversine_distance(
                incident.latitude,
                incident.longitude,
                a.latitude,
                a.longitude,
            );
            let dist_b = haversine_distance(
                incident.latitude,
                incident.longitude,
                b.latitude,
                b.longitude,
            );
            dist_a
                .partial_cmp(&dist_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take up to 3 closest resources for heuristic allocation
        let to_assign: Vec<_> = available_resources.into_iter().take(3).collect();

        let allocations: Vec<AllocationEntry> = to_assign
            .iter()
            .map(|r| {
                let dist = haversine_distance(
                    incident.latitude,
                    incident.longitude,
                    r.latitude,
                    r.longitude,
                );
                let center_name = centers
                    .iter()
                    .find(|c| c.id == r.owner_center_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                AllocationEntry {
                    center_name,
                    resource_id: r.id,
                    distance_km: dist,
                }
            })
            .collect();

        if !allocations.is_empty() {
            envelopes.push(ToolCallEnvelope {
                tool_name: TOOL_NAME.to_string(),
                arguments: DispatchArguments {
                    incident_id: incident.id,
                    primary_center_id: incident.primary_center_id,
                    allocations,
                    resource_state_modifications: Vec::new(),
                    justification: "Heuristic proximity allocation".to_string(),
                },
            });
        }
    }

    Ok(envelopes)
}
