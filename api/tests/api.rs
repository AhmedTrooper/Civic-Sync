use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use civic_sync_api::features::{
    command_centers::CommandCenter,
    dispatch::Recommendation,
    incidents::{Incident, IncidentStatus, priority_reasons, priority_score},
    resources::{Resource, ResourceStatus, ResourceType},
};
use civic_sync_api::{app, state::AppState};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

fn sample_incident() -> Incident {
    let now = Utc::now();
    Incident {
        id: Uuid::new_v4(),
        title: "Flooding in Mirpur".into(),
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

fn sample_resource() -> Resource {
    Resource {
        id: Uuid::new_v4(),
        center_id: None,
        incident_id: None,
        resource_type: ResourceType::Ambulance,
        unit_identifier: "AMB-001".into(),
        status: ResourceStatus::Available,
        distance_passed_km: 0.0,
        distance_remaining_km: 0.0,
        latitude: 23.8103,
        longitude: 90.4125,
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

#[tokio::test]
async fn command_centers_seed_all_8_hubs() {
    let app = app::router(None);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/command-centers")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
    let centers: Vec<CommandCenter> = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("read body"),
    )
    .expect("parse command centers");
    assert_eq!(centers.len(), 8, "expected the 8 divisional hubs");
    let dhaka = centers
        .iter()
        .find(|center| center.is_core_center && center.name == "Dhaka")
        .expect("Dhaka core center present");
    assert_eq!(dhaka.latitude, 23.8103);
    assert_eq!(dhaka.longitude, 90.4125);
}

#[tokio::test]
async fn command_centers_filter_by_core_flag() {
    let app = app::router(None);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/command-centers?is_core_center=true")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
    let centers: Vec<CommandCenter> = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("read body"),
    )
    .expect("parse command centers");
    assert_eq!(centers.len(), 1, "only Dhaka is the core center");
    assert_eq!(centers[0].name, "Dhaka");
}

#[tokio::test]
async fn incident_lifecycle_works() {
    let app = app::router(None);
    let payload = json!({
        "title": "Structural collapse in Uttara",
        "severity_level": 5,
        "affected_people": 250,
        "casualty_count": 7,
        "latitude": 23.87,
        "longitude": 90.40
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/incidents")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let list = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/incidents?severity_level=5")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(list.status(), StatusCode::OK);
    let body: Vec<Value> = serde_json::from_slice(
        &axum::body::to_bytes(list.into_body(), 1_000_000)
            .await
            .expect("read body"),
    )
    .expect("parse incidents");
    assert!(
        !body.is_empty(),
        "filtered list should contain the incident"
    );
}

#[tokio::test]
async fn dispatch_recommendations_rank_incidents() {
    let state = AppState::new(None);
    {
        let mut resources = state.resources.write().await;
        resources.insert(sample_resource().id, sample_resource());
    }
    {
        let mut incidents = state.incidents.write().await;
        let low = Incident {
            id: Uuid::new_v4(),
            severity_level: 1,
            ..sample_incident()
        };
        let high = sample_incident();
        incidents.insert(low.id, low);
        incidents.insert(high.id, high);
    }
    let app = app::router_with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/dispatch/recommendations")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("read body"),
    )
    .expect("parse recommendations");
    let recommendations: Vec<Recommendation> =
        serde_json::from_value(body["recommendations"].clone()).unwrap();
    assert_eq!(recommendations.len(), 2);
    assert!(
        recommendations[0].priority_score >= recommendations[1].priority_score,
        "higher-severity incident should rank first"
    );
    assert!(
        recommendations
            .iter()
            .all(|r| r.travel_distance_km.is_some()),
        "each recommendation should include a travel distance"
    );
    assert!(
        recommendations
            .iter()
            .all(|r| r.nearest_resource_id.is_some()),
        "each recommendation should reference the single available resource"
    );
}

#[tokio::test]
async fn incident_validation_rejects_bad_severity() {
    let app = app::router(None);
    let payload = json!({
        "title": "Out-of-range severity",
        "severity_level": 9,
        "affected_people": 10,
        "casualty_count": 0,
        "latitude": 23.8,
        "longitude": 90.4
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/incidents")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "severity_level outside 1..=5 must be rejected"
    );
}
