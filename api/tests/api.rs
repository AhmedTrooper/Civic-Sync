use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use civic_sync_api::features::{
    command_centers::CommandCenter,
    dispatch::Recommendation,
    incidents::{Environment, Incident, ResourceNeed, Severity, priority_reasons, priority_score},
    resources::{Resource, ResourceKind, ResourceStatus},
};
use civic_sync_api::{app, state::AppState};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

fn sample_incident() -> Incident {
    let now = Utc::now();
    Incident {
        id: Uuid::new_v4(),
        region: "Mirpur".into(),
        severity: Severity::High,
        casualties: 4,
        affected_population: 120,
        time_sensitivity: 8,
        resource_needs: vec![ResourceNeed::Ambulance, ResourceNeed::RescueTeam],
        environment: Environment::Flood,
        status: civic_sync_api::features::incidents::IncidentStatus::Pending,
        created_at: now,
        updated_at: now,
    }
}

fn sample_resource() -> Resource {
    Resource {
        id: Uuid::new_v4(),
        kind: ResourceKind::Ambulance,
        capacity: 6,
        latitude: 23.8103,
        longitude: 90.4125,
        status: ResourceStatus::Available,
        current_incident_id: None,
        updated_at: Utc::now(),
    }
}

#[test]
fn priority_score_combines_inputs() {
    let incident = sample_incident();
    let score = priority_score(&incident);
    assert!(score > 30.0, "expected high score, got {score}");
    let reasons = priority_reasons(&incident);
    assert!(reasons.iter().any(|reason| reason.contains("severity")));
    assert!(reasons.iter().any(|reason| reason.contains("casualty")));
}

#[test]
fn priority_score_escalates_environment() {
    let mut incident = sample_incident();
    let baseline = priority_score(&incident);
    incident.environment = Environment::StructuralCollapse;
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
        "region": "Uttara",
        "severity": "critical",
        "casualties": 7,
        "affected_population": 250,
        "time_sensitivity": 9,
        "resource_needs": ["ambulance", "hospital"],
        "environment": "fire"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/incidents")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let list = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/incidents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body: Vec<Value> = serde_json::from_slice(
        &axum::body::to_bytes(list.into_body(), 1_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(!body.is_empty());
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
            severity: Severity::Low,
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
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    let recommendations: Vec<Recommendation> =
        serde_json::from_value(body["recommendations"].clone()).unwrap();
    assert_eq!(recommendations.len(), 2);
    assert!(recommendations[0].priority_score >= recommendations[1].priority_score);
}
