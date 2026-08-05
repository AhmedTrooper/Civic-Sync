use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    error::ApiError,
    features::{
        command_centers::{self, CommandCenter},
        helper_allocations::{self, AllocationStatus, CreateHelperAllocation},
        helper_teams::{self, HelperTeam},
        incidents::{self, Incident},
        resources::{self, Resource, ResourceStatus, ResourceType},
    },
    state::AppState,
};

/// data.md §7 — the named tool-call the Rig AI engine will emit.
pub const TOOL_NAME: &str = "dispatch_multi_center_response";

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallEnvelope {
    pub tool_name: String,
    pub arguments: DispatchArguments,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchArguments {
    pub incident_id: Uuid,
    pub primary_center_id: Uuid,
    pub core_fallback_center_id: Option<Uuid>,
    pub allocations: Vec<AllocationEntry>,
    pub resource_state_modifications: Vec<ResourceStateModification>,
    pub justification: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AllocationEntry {
    pub center_name: String,
    pub team_id: Uuid,
    pub members_deployed: u32,
    pub distance_km: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceStateModification {
    pub resource_id: Uuid,
    pub new_status: String,
    pub reason: String,
}

pub async fn recommend(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (incidents, centers, resources, teams) = match &state.database {
        Some(pool) => (
            incidents::list_all_postgres(pool).await?,
            command_centers::list_all_postgres(pool).await?,
            resources::list_all_postgres(pool).await?,
            helper_teams::list_all_postgres(pool).await?,
        ),
        None => (
            state.incidents.read().await.values().cloned().collect(),
            state
                .command_centers
                .read()
                .await
                .values()
                .cloned()
                .collect(),
            state.resources.read().await.values().cloned().collect(),
            state.helper_teams.read().await.values().cloned().collect(),
        ),
    };

    let core = centers
        .iter()
        .find(|c| c.is_core_center)
        .ok_or_else(|| ApiError::Internal("Dhaka core center is missing".into()))?;

    // Sort incidents by priority DESC per data.md §3 — most urgent first.
    let mut ranked: Vec<&Incident> = incidents.iter().collect();
    ranked.sort_by(|a, b| {
        incidents::priority_score(b)
            .partial_cmp(&incidents::priority_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Dispatch-eligible means "EN_ROUTE and not currently assigned" per data.md §6.4 + the
    // Conflict-Prevention contract. Local working copies prevent double-booking across
    // incidents within a single batch.
    let mut taken_resources: HashSet<Uuid> = HashSet::new();
    let mut taken_teams: HashSet<Uuid> = HashSet::new();
    let mut resources_view: HashMap<Uuid, Resource> =
        resources.into_iter().map(|r| (r.id, r)).collect();
    let mut teams_view: HashMap<Uuid, HelperTeam> = teams.into_iter().map(|t| (t.id, t)).collect();

    // Pre-build a list of centers sorted by distance to each incident on demand.
    let mut envelopes: Vec<ToolCallEnvelope> = Vec::with_capacity(ranked.len());
    let mut pending_modifications: Vec<(Uuid, Uuid, ResourceStatus, f64)> = Vec::new();
    let mut pending_allocations: Vec<CreateHelperAllocation> = Vec::new();
    let mut pending_team_bumps: Vec<(Uuid, u32)> = Vec::new();
    let now = Utc::now();

    for incident in ranked {
        let primary = nearest_center(&centers, incident.latitude, incident.longitude);

        // 1) Resource selection per kind (data.md §5A primary → Dhaka fallback).
        let mut allocations: Vec<AllocationEntry> = Vec::new();
        let mut resource_modifications: Vec<ResourceStateModification> = Vec::new();
        let mut fallback_used = false;

        for kind in &incident.required_resource_types {
            if let Some(picked) = pick_resource(
                &resources_view,
                *kind,
                primary.id,
                incident,
                &taken_resources,
            ) {
                let distance = haversine_km(
                    incident.latitude,
                    incident.longitude,
                    picked.latitude,
                    picked.longitude,
                );
                taken_resources.insert(picked.id);
                pending_modifications.push((
                    picked.id,
                    incident.id,
                    ResourceStatus::EnRoute,
                    distance,
                ));
            } else if let Some(picked) = pick_resource_any_hub(
                &resources_view,
                *kind,
                primary.id,
                incident,
                &taken_resources,
            ) {
                let distance = haversine_km(
                    incident.latitude,
                    incident.longitude,
                    picked.latitude,
                    picked.longitude,
                );
                fallback_used = true;
                taken_resources.insert(picked.id);
                pending_modifications.push((
                    picked.id,
                    incident.id,
                    ResourceStatus::EnRoute,
                    distance,
                ));
                resource_modifications.push(ResourceStateModification {
                    resource_id: picked.id,
                    new_status: "REJECTED".to_string(),
                    reason: format!(
                        "{} Hub has no {:?} available; re-routed via secondary hub",
                        primary.name, kind
                    ),
                });
            } else {
                resource_modifications.push(ResourceStateModification {
                    resource_id: Uuid::nil(),
                    new_status: "REJECTED".to_string(),
                    reason: format!(
                        "no {:?} resource available across the network for incident {}",
                        kind, incident.title
                    ),
                });
            }
        }

        // 2) Helper team selection (primary first, then core fallback).
        let required_members = u32::from(incident.severity_level).saturating_mul(2).max(1);
        let mut picked_primary: Option<(HelperTeam, f64)> = None;
        let mut picked_core: Option<(HelperTeam, f64)> = None;

        if let Some((team, distance)) = pick_team(
            &teams_view,
            primary.id,
            &taken_teams,
            required_members,
            incident,
        ) {
            let deploy = required_members.min(team.available_capacity());
            pending_allocations.push(CreateHelperAllocation {
                helper_team_id: team.id,
                incident_id: Some(incident.id),
                assistance_request_id: None,
                members_deployed: deploy,
            });
            pending_team_bumps.push((team.id, deploy));
            taken_teams.insert(team.id);
            if let Some(t) = teams_view.get_mut(&team.id) {
                t.assigned_members = t.assigned_members.saturating_add(deploy);
            }
            picked_primary = Some((team, distance));
        } else if let Some((team, distance)) = pick_team(
            &teams_view,
            core.id,
            &taken_teams,
            required_members,
            incident,
        ) {
            let deploy = required_members.min(team.available_capacity());
            fallback_used = true;
            pending_allocations.push(CreateHelperAllocation {
                helper_team_id: team.id,
                incident_id: Some(incident.id),
                assistance_request_id: None,
                members_deployed: deploy,
            });
            pending_team_bumps.push((team.id, deploy));
            taken_teams.insert(team.id);
            if let Some(t) = teams_view.get_mut(&team.id) {
                t.assigned_members = t.assigned_members.saturating_add(deploy);
            }
            picked_core = Some((team, distance));
        }

        if let Some((team, distance)) = picked_primary {
            allocations.push(AllocationEntry {
                center_name: primary.name.clone(),
                team_id: team.id,
                members_deployed: required_members.min(team.available_capacity()),
                distance_km: round2(distance),
            });
        }
        if let Some((team, distance)) = picked_core {
            allocations.push(AllocationEntry {
                center_name: core.name.clone(),
                team_id: team.id,
                members_deployed: required_members.min(team.available_capacity()),
                distance_km: round2(distance),
            });
        }

        let justification = if fallback_used {
            format!(
                "{} Hub lacks required assets; Dhaka Core Center assigned to cover remaining need for incident {}",
                primary.name, incident.title
            )
        } else {
            format!(
                "{} Hub has nearest resources for incident {} (severity {}, casualties {})",
                primary.name, incident.title, incident.severity_level, incident.casualty_count
            )
        };

        envelopes.push(ToolCallEnvelope {
            tool_name: TOOL_NAME.to_string(),
            arguments: DispatchArguments {
                incident_id: incident.id,
                primary_center_id: primary.id,
                core_fallback_center_id: if fallback_used { Some(core.id) } else { None },
                allocations,
                resource_state_modifications: resource_modifications,
                justification,
            },
        });
    }

    // 3) Apply mutations: in-memory mirror OR a single postgres batch transaction.
    if let Some(pool) = &state.database {
        let mut tx = pool.begin().await?;
        for (resource_id, incident_id, status, distance_remaining_km) in &pending_modifications {
            resources::dispatch_link_postgres(
                &mut tx,
                *resource_id,
                *incident_id,
                *status,
                *distance_remaining_km,
                now,
            )
            .await?;
        }
        for alloc in &pending_allocations {
            helper_allocations::insert_postgres_tx(&mut tx, alloc, AllocationStatus::EnRoute, now)
                .await?;
        }
        for (team_id, delta) in &pending_team_bumps {
            helper_teams::bump_assigned_postgres_tx(&mut tx, *team_id, *delta, now).await?;
        }
        tx.commit().await?;
    } else {
        for (resource_id, incident_id, status, distance_remaining_km) in pending_modifications {
            if let Some(resource) = resources_view.get_mut(&resource_id) {
                resource.status = status;
                resource.incident_id = Some(incident_id);
                resource.distance_remaining_km = distance_remaining_km;
                resource.updated_at = now;
            }
        }
        for alloc in pending_allocations {
            if let Some(team) = teams_view.get_mut(&alloc.helper_team_id) {
                team.assigned_members =
                    team.assigned_members.saturating_add(alloc.members_deployed);
                team.updated_at = now;
            }
        }
        // Push the mutated view back into AppState.
        let mut state_resources = state.resources.write().await;
        *state_resources = resources_view;
        drop(state_resources);
        let mut state_teams = state.helper_teams.write().await;
        *state_teams = teams_view;
        drop(state_teams);
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "generated_at": Utc::now(),
            "recommendations": envelopes,
        })),
    ))
}

// --- helpers ----------------------------------------------------------------

fn nearest_center(centers: &[CommandCenter], lat: f64, lon: f64) -> &CommandCenter {
    centers
        .iter()
        .min_by(|a, b| {
            let da = haversine_km(lat, lon, a.latitude, a.longitude);
            let db = haversine_km(lat, lon, b.latitude, b.longitude);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("centers list is non-empty (validated at function entry)")
}

fn is_dispatchable(resource: &Resource, taken: &HashSet<Uuid>) -> bool {
    resource.incident_id.is_none()
        && matches!(resource.status, ResourceStatus::EnRoute)
        && !taken.contains(&resource.id)
}

fn pick_resource(
    resources: &HashMap<Uuid, Resource>,
    kind: ResourceType,
    primary_id: Uuid,
    incident: &Incident,
    taken: &HashSet<Uuid>,
) -> Option<Resource> {
    resources
        .values()
        .filter(|r| r.resource_type == kind && r.center_id == Some(primary_id))
        .filter(|r| is_dispatchable(r, taken))
        .min_by(|a, b| {
            let da = haversine_km(
                incident.latitude,
                incident.longitude,
                a.latitude,
                a.longitude,
            );
            let db = haversine_km(
                incident.latitude,
                incident.longitude,
                b.latitude,
                b.longitude,
            );
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn pick_resource_any_hub(
    resources: &HashMap<Uuid, Resource>,
    kind: ResourceType,
    exclude_id: Uuid,
    incident: &Incident,
    taken: &HashSet<Uuid>,
) -> Option<Resource> {
    resources
        .values()
        .filter(|r| {
            r.resource_type == kind && r.center_id.is_some() && r.center_id != Some(exclude_id)
        })
        .filter(|r| is_dispatchable(r, taken))
        .min_by(|a, b| {
            let da = haversine_km(
                incident.latitude,
                incident.longitude,
                a.latitude,
                a.longitude,
            );
            let db = haversine_km(
                incident.latitude,
                incident.longitude,
                b.latitude,
                b.longitude,
            );
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn pick_team(
    teams: &HashMap<Uuid, HelperTeam>,
    center_id: Uuid,
    taken: &HashSet<Uuid>,
    required: u32,
    incident: &Incident,
) -> Option<(HelperTeam, f64)> {
    teams
        .values()
        .filter(|t| t.center_id == Some(center_id))
        .filter(|t| !taken.contains(&t.id) && t.available_capacity() >= required)
        .min_by(|a, b| {
            let da = haversine_km(
                incident.latitude,
                incident.longitude,
                a.latitude,
                a.longitude,
            );
            let db = haversine_km(
                incident.latitude,
                incident.longitude,
                b.latitude,
                b.longitude,
            );
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|team| {
            let distance = haversine_km(
                incident.latitude,
                incident.longitude,
                team.latitude,
                team.longitude,
            );
            (team.clone(), distance)
        })
}

/// Great-circle distance in kilometres between two WGS84 coordinates.
fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let earth_radius_km = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    earth_radius_km * c
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
