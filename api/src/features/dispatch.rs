use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    features::{incidents, resources},
    state::AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Recommendation {
    pub incident_id: Uuid,
    pub priority_score: f64,
    pub reasons: Vec<String>,
    pub nearest_resource_id: Option<Uuid>,
    pub travel_distance_km: Option<f64>,
}

pub async fn recommend(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (incidents, resources) = match &state.database {
        Some(pool) => (
            incidents::list_all_postgres(pool).await?,
            resources::list_all_postgres(pool).await?,
        ),
        None => (
            state.incidents.read().await.values().cloned().collect(),
            state.resources.read().await.values().cloned().collect(),
        ),
    };

    let mut recommendations: Vec<Recommendation> = incidents
        .iter()
        .map(|incident| {
            let nearest = resources
                .iter()
                .filter(|resource| matches!(resource.status, resources::ResourceStatus::Available))
                .min_by(|a, b| {
                    let dist_a = haversine_km(
                        incident.latitude,
                        incident.longitude,
                        a.latitude,
                        a.longitude,
                    );
                    let dist_b = haversine_km(
                        incident.latitude,
                        incident.longitude,
                        b.latitude,
                        b.longitude,
                    );
                    dist_a
                        .partial_cmp(&dist_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            let (nearest_id, travel_km) = nearest
                .map(|resource| {
                    let distance = haversine_km(
                        incident.latitude,
                        incident.longitude,
                        resource.latitude,
                        resource.longitude,
                    );
                    (Some(resource.id), Some(distance))
                })
                .unwrap_or((None, None));

            Recommendation {
                incident_id: incident.id,
                priority_score: round2(incidents::priority_score(incident)),
                reasons: incidents::priority_reasons(incident),
                nearest_resource_id: nearest_id,
                travel_distance_km: travel_km,
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
