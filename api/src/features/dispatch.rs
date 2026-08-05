use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    features::{
        incidents::{self, ResourceNeed, priority_reasons, priority_score},
        resources::{self, ResourceStatus},
    },
    state::AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Recommendation {
    pub incident_id: Uuid,
    pub priority_score: f64,
    pub reasons: Vec<String>,
    pub nearest_resource_id: Option<Uuid>,
    pub resource_gap: Vec<ResourceNeed>,
}

pub async fn recommend(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (incidents, resources) = match &state.database {
        Some(pool) => (
            incidents::list_postgres(pool).await?,
            resources::list_postgres(pool).await?,
        ),
        None => (
            state.incidents.read().await.values().cloned().collect(),
            state.resources.read().await.values().cloned().collect(),
        ),
    };

    let mut recommendations: Vec<Recommendation> = incidents
        .iter()
        .map(|incident| {
            let available = resources
                .iter()
                .filter(|resource| matches!(resource.status, ResourceStatus::Available))
                .min_by(|a, b| {
                    let dist_a = approx_distance(a.latitude, a.longitude, 23.8103, 90.4125);
                    let dist_b = approx_distance(b.latitude, b.longitude, 23.8103, 90.4125);
                    dist_a
                        .partial_cmp(&dist_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            let nearest = available.map(|resource| resource.id); // resource is &Resource
            let mut gap: Vec<ResourceNeed> = incident.resource_needs.clone();
            gap.sort_by_key(|need| serde_json::to_string(need).unwrap_or_default());
            Recommendation {
                incident_id: incident.id,
                priority_score: round2(priority_score(incident)),
                reasons: priority_reasons(incident),
                nearest_resource_id: nearest,
                resource_gap: gap,
            }
        })
        .collect();

    recommendations.sort_by(|a, b| {
        b.priority_score
            .partial_cmp(&a.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "generated_at": chrono::Utc::now(),
            "recommendations": recommendations,
        })),
    ))
}

fn approx_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dx = lat1 - lat2;
    let dy = lon1 - lon2;
    (dx * dx + dy * dy).sqrt()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
