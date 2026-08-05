use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use civic_sync_api::app;
use civic_sync_api::features::{
    incidents::{Incident, IncidentStatus, priority_reasons, priority_score},
    resources::{Resource, ResourceStatus, ResourceType},
};
use tower::ServiceExt;
use uuid::Uuid;

fn sample_incident() -> Incident {
    let now = Utc::now();
    Incident {
        id: Uuid::new_v4(),
        title: "Flooding in Mirpur".into(),
        primary_center_id: Uuid::new_v4(),
        severity_level: 4,
        affected_people: 120,
        casualty_count: 4,
        latitude: 23.78,
        longitude: 90.37,
        status: IncidentStatus::Active,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    }
}

#[allow(dead_code)]
fn sample_resource() -> Resource {
    Resource {
        id: Uuid::new_v4(),
        owner_center_id: Uuid::new_v4(),
        assigned_incident_id: None,
        resource_type: ResourceType::Ambulance,
        unit_identifier: "AMB-001".into(),
        status: ResourceStatus::EnRoute,
        distance_passed_km: 0.0,
        distance_remaining_km: 0.0,
        latitude: 23.8103,
        longitude: 90.4125,
        total_capacity: 10,
        current_capacity: 10,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        server_synced_at: None,
    }
}

#[test]
fn priority_score_combines_inputs() {
    let incident = sample_incident();
    let score = priority_score(&incident);
    assert!(score > 40.0, "expected a high score, got {score}");
    let reasons = priority_reasons(&incident);
    assert!(reasons.iter().any(|reason| reason.contains("severity")));
    assert!(reasons.iter().any(|reason| reason.contains("casualties")));
}

#[test]
fn priority_score_increases_with_severity() {
    let mut incident = sample_incident();
    let baseline = priority_score(&incident);
    incident.severity_level = 5;
    let escalated = priority_score(&incident);
    assert!(escalated > baseline);
}

#[test]
fn priority_score_ranks_higher_with_more_casualties() {
    let mut incident = sample_incident();
    let baseline = priority_score(&incident);
    incident.casualty_count = 50;
    let escalated = priority_score(&incident);
    assert!(escalated > baseline);
}

#[tokio::test]
async fn health_endpoints_respond() {
    let app = app::router(None);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
}
