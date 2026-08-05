use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use civic_sync_api::features::{
    assistance_requests::{AssistanceRequest, AssistanceStatus},
    command_centers::CommandCenter,
    dispatch::Recommendation,
    helper_allocations::HelperAllocation,
    helper_teams::HelperTeam,
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
        required_resource_types: vec![ResourceType::Ambulance],
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
        status: ResourceStatus::EnRoute,
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

async fn post_json(service: &axum::Router, uri: &str, payload: &Value) -> (StatusCode, Value) {
    let response = service
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("build request"),
        )
        .await
        .expect("run request");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("read body");
    let value = serde_json::from_slice(&body).unwrap_or(json!({}));
    (status, value)
}

async fn get_json(service: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = service
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("run request");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("read body");
    let value = serde_json::from_slice(&body).unwrap_or(json!({}));
    (status, value)
}

#[tokio::test]
async fn helper_team_lifecycle_tracks_capacity() {
    let app = app::router(None);
    let (status, body) = post_json(
        &app,
        "/api/v1/helper-teams",
        &json!({
            "center_id": null,
            "team_name": "Sylhet Rescue Alpha",
            "total_members": 10,
            "latitude": 24.89,
            "longitude": 91.86
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let team: HelperTeam = serde_json::from_value(body).expect("parse helper team");
    assert_eq!(team.team_name, "Sylhet Rescue Alpha");
    assert_eq!(team.available_capacity(), 10);

    let (status, body) = get_json(&app, "/api/v1/helper-teams?has_capacity=true").await;
    assert_eq!(status, StatusCode::OK);
    let teams: Vec<HelperTeam> = serde_json::from_value(body).expect("parse helper teams");
    assert_eq!(teams.len(), 1, "the available team should be listed");
    assert_eq!(teams[0].id, team.id);
}

#[tokio::test]
async fn helper_allocation_checks_capacity_and_conflicts() {
    let app = app::router(None);

    let (_, team_body) = post_json(
        &app,
        "/api/v1/helper-teams",
        &json!({
            "center_id": null,
            "team_name": "Dhaka Rapid Response",
            "total_members": 5,
            "latitude": 23.81,
            "longitude": 90.41
        }),
    )
    .await;
    let team: HelperTeam = serde_json::from_value(team_body).expect("parse team");

    let (_, incident_body) = post_json(
        &app,
        "/api/v1/incidents",
        &json!({
            "title": "High-rise fire in Gulshan",
            "severity_level": 5,
            "affected_people": 200,
            "casualty_count": 3,
            "latitude": 23.79,
            "longitude": 90.41
        }),
    )
    .await;
    let incident_id = incident_body["id"]
        .as_str()
        .expect("incident id")
        .to_string();

    let (status, body) = post_json(
        &app,
        "/api/v1/helper-allocations",
        &json!({
            "helper_team_id": team.id.to_string(),
            "incident_id": incident_id,
            "members_deployed": 5
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let allocation: HelperAllocation = serde_json::from_value(body).expect("parse allocation");
    assert_eq!(allocation.members_deployed, 5);

    let (conflict_status, _) = post_json(
        &app,
        "/api/v1/helper-allocations",
        &json!({
            "helper_team_id": team.id.to_string(),
            "incident_id": team.id.to_string(),
            "members_deployed": 1
        }),
    )
    .await;
    assert_eq!(
        conflict_status,
        StatusCode::CONFLICT,
        "over-allocation beyond team capacity must be rejected"
    );
}

#[tokio::test]
async fn assistance_request_lifecycle_works() {
    let app = app::router(None);

    let (_, resource_body) = post_json(
        &app,
        "/api/v1/resources",
        &json!({
            "center_id": null,
            "resource_type": "BOAT",
            "unit_identifier": "BOAT-07",
            "latitude": 22.35,
            "longitude": 91.78
        }),
    )
    .await;
    let resource_id = resource_body["id"]
        .as_str()
        .expect("resource id")
        .to_string();

    let (status, body) = post_json(
        &app,
        "/api/v1/assistance-requests",
        &json!({
            "resource_id": resource_id,
            "issue_description": "Outboard motor failed during flood rescue"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let request: AssistanceRequest =
        serde_json::from_value(body).expect("parse assistance request");
    assert_eq!(request.status, AssistanceStatus::Pending);

    let (status, body) = get_json(&app, "/api/v1/assistance-requests?status=PENDING").await;
    assert_eq!(status, StatusCode::OK);
    let requests: Vec<AssistanceRequest> = serde_json::from_value(body).expect("parse requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].id, request.id);
}
