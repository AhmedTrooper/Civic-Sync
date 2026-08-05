use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode};
use chrono::Utc;
use civic_sync_api::config::Config;
use civic_sync_api::features::{
    assistance_requests::{AssistanceRequest, AssistanceStatus},
    command_centers::CommandCenter,
    dispatch::ToolCallEnvelope,
    helper_allocations::HelperAllocation,
    helper_teams::HelperTeam,
    incidents::{Incident, IncidentStatus, priority_reasons, priority_score},
    resources::{Resource, ResourceStatus, ResourceType},
};
use civic_sync_api::{app, state::AppState};
use serde_json::{Value, json};
use std::sync::Arc;
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
    let low_id;
    let high_id;
    {
        let mut incidents = state.incidents.write().await;
        let low = Incident {
            id: Uuid::new_v4(),
            severity_level: 1,
            ..sample_incident()
        };
        let high = sample_incident();
        low_id = low.id;
        high_id = high.id;
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
    let envelopes: Vec<ToolCallEnvelope> =
        serde_json::from_value(body["recommendations"].clone()).expect("parse envelopes");
    assert_eq!(envelopes.len(), 2);
    assert!(
        envelopes
            .iter()
            .all(|e| e.tool_name == "dispatch_multi_center_response"),
        "every envelope must use the spec tool_name"
    );
    assert!(
        envelopes
            .iter()
            .all(|e| !e.arguments.primary_center_id.is_nil()),
        "every envelope must select a primary center"
    );
    // The higher-severity incident must rank first; the engine sorts by priority_score DESC.
    let expected_order = vec![high_id, low_id];
    let actual_order: Vec<Uuid> = envelopes.iter().map(|e| e.arguments.incident_id).collect();
    assert_eq!(actual_order, expected_order);
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

#[tokio::test]
async fn dispatch_creates_helper_allocation_and_marks_resource_en_route() {
    let state = AppState::new(None);
    let core = state
        .command_centers
        .read()
        .await
        .values()
        .find(|c| c.is_core_center)
        .cloned()
        .expect("Dhaka core center is seeded");

    let (_, incident_body) = post_json(
        &app::router_with_state(state.clone()),
        "/api/v1/incidents",
        &json!({
            "title": "High-rise fire in Gulshan",
            "severity_level": 5,
            "affected_people": 200,
            "casualty_count": 3,
            "latitude": 23.79,
            "longitude": 90.41,
            "required_resource_types": ["AMBULANCE"]
        }),
    )
    .await;
    let incident_id = incident_body["id"]
        .as_str()
        .expect("incident id")
        .to_string();

    let (_, resource_body) = post_json(
        &app::router_with_state(state.clone()),
        "/api/v1/resources",
        &json!({
            "center_id": core.id,
            "resource_type": "AMBULANCE",
            "unit_identifier": "AMB-007",
            "latitude": 23.81,
            "longitude": 90.41
        }),
    )
    .await;
    let resource_id = resource_body["id"]
        .as_str()
        .expect("resource id")
        .to_string();

    let (_, team_body) = post_json(
        &app::router_with_state(state.clone()),
        "/api/v1/helper-teams",
        &json!({
            "center_id": core.id,
            "team_name": "Dhaka Rapid Response",
            "total_members": 5,
            "latitude": 23.81,
            "longitude": 90.41
        }),
    )
    .await;
    let team_id = team_body["id"].as_str().expect("team id").to_string();

    // Trigger dispatch.
    let app = app::router_with_state(state.clone());
    let response = app
        .clone()
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

    // Resource mutated to EN_ROUTE and linked to the incident.
    let (status, body) = get_json(
        &app::router_with_state(state.clone()),
        &format!("/api/v1/resources/{resource_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resource: Resource = serde_json::from_value(body).expect("parse resource");
    assert_eq!(resource.status, ResourceStatus::EnRoute);
    assert_eq!(
        resource.incident_id,
        Some(Uuid::parse_str(&incident_id).unwrap())
    );

    // Helper team assigned_members bumped.
    let (status, body) = get_json(
        &app::router_with_state(state.clone()),
        &format!("/api/v1/helper-teams/{team_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let team: HelperTeam = serde_json::from_value(body).expect("parse team");
    assert_eq!(team.assigned_members, 5);

    // helper_allocations row created.
    let (status, body) = get_json(
        &app::router_with_state(state.clone()),
        "/api/v1/helper-allocations",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let allocations: Vec<HelperAllocation> =
        serde_json::from_value(body).expect("parse allocations");
    assert_eq!(allocations.len(), 1);
    assert_eq!(
        allocations[0].incident_id,
        Some(Uuid::parse_str(&incident_id).unwrap())
    );
}

fn config_with_origins(origins: Vec<String>) -> Arc<Config> {
    let mut config = (*Config::default_for_tests()).clone();
    config.allowed_origins = origins;
    Arc::new(config)
}

#[tokio::test]
async fn cors_reflects_configured_allowed_origins() {
    let config = config_with_origins(vec![
        "http://allowed.test".into(),
        "https://another.allowed.test".into(),
    ]);
    let app = app::router_with_config(None, config);

    // Preflight from an allowed origin.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/incidents")
                .header("origin", "http://allowed.test")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .expect("build preflight"),
        )
        .await
        .expect("run preflight");
    assert_eq!(response.status(), StatusCode::OK);
    let allow_origin = response
        .headers()
        .get("access-control-allow-origin")
        .expect("allow-origin header is present");
    assert_eq!(
        allow_origin,
        HeaderValue::from_static("http://allowed.test")
    );

    // Preflight from a non-allowed origin — the header must be absent.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/incidents")
                .header("origin", "http://blocked.test")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .expect("build preflight"),
        )
        .await
        .expect("run preflight");
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "blocked origin must not receive allow-origin"
    );
}

#[tokio::test]
async fn cors_is_wildcard_when_allowed_origins_empty() {
    let app = app::router(None);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/incidents")
                .header("origin", "http://anything.test")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .expect("build preflight"),
        )
        .await
        .expect("run preflight");
    assert_eq!(response.status(), StatusCode::OK);
    let allow_origin = response
        .headers()
        .get("access-control-allow-origin")
        .expect("allow-origin present when empty allow list");
    assert_eq!(allow_origin, HeaderValue::from_static("*"));
}

/// Build a Config with the AI block fully populated. Used by the
/// "orchestrator configured" path tests below. The API key is a dummy
/// since the tests verify the *heuristic fallback* path — the LLM is
/// never actually called.
fn config_with_ai_provider(provider: &str, model: &str) -> Arc<Config> {
    let mut config = (*Config::default_for_tests()).clone();
    config.ai.provider = Some(provider.into());
    config.ai.model = Some(model.into());
    config.ai.api_key = Some("sk-test".into());
    Arc::new(config)
}

#[tokio::test]
async fn dispatch_uses_heuristic_when_orchestrator_configured_but_key_is_dummy() {
    // Synthesize a state with the AI block fully populated. The tests are
    // not allowed to make a real network call, so the orchestrator must
    // transparently fall back to the heuristic when the LLM is unreachable.
    let config = config_with_ai_provider("openai", "gpt-4o-mini");
    let state = AppState::with_config(None, config);
    {
        let mut resources = state.resources.write().await;
        resources.insert(sample_resource().id, sample_resource());
    }
    let mut incidents = state.incidents.write().await;
    let incident_id = uuid::Uuid::new_v4();
    incidents.insert(
        incident_id,
        Incident {
            id: incident_id,
            ..sample_incident()
        },
    );
    drop(incidents);

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
    let envelopes: Vec<ToolCallEnvelope> =
        serde_json::from_value(body["recommendations"].clone()).expect("parse envelopes");
    assert_eq!(envelopes.len(), 1);
    let envelope = &envelopes[0];
    assert_eq!(envelope.tool_name, "dispatch_multi_center_response");
    assert_eq!(envelope.arguments.incident_id, incident_id);
    assert!(envelope.arguments.primary_center_id != uuid::Uuid::nil());
    // The heuristic justification is deterministic; the LLM (which would
    // refine the text) is never actually called in this test.
    assert!(
        envelope.arguments.justification.contains("Hub"),
        "heuristic-fallback justification should mention the hub: got {:?}",
        envelope.arguments.justification
    );
}

#[tokio::test]
async fn dispatch_uses_heuristic_when_no_ai_configured() {
    // Default Config has no AI block — the orchestrator is None and the
    // dispatch handler must run the heuristic directly. This is the
    // "production" path when the operator hasn't wired up an LLM.
    let state = AppState::new(None);
    {
        let mut resources = state.resources.write().await;
        resources.insert(sample_resource().id, sample_resource());
    }
    let mut incidents = state.incidents.write().await;
    let incident_id = uuid::Uuid::new_v4();
    incidents.insert(
        incident_id,
        Incident {
            id: incident_id,
            ..sample_incident()
        },
    );
    drop(incidents);

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
    let envelopes: Vec<ToolCallEnvelope> =
        serde_json::from_value(body["recommendations"].clone()).expect("parse envelopes");
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0].arguments.incident_id, incident_id);
}

#[tokio::test]
async fn dispatch_semaphore_exhaustion_does_not_block_subsequent_calls() {
    // Even when the orchestrator's gate is fully consumed, the dispatch
    // endpoint must always return a successful response. The orchestrator
    // returns the heuristic envelopes when the gate is exhausted; the HTTP
    // response is still 200 OK with the spec-shaped envelope.
    let config = config_with_ai_provider("openai", "gpt-4o-mini");
    let state = AppState::with_config(None, config);
    let mut incidents = state.incidents.write().await;
    let incident_id = uuid::Uuid::new_v4();
    incidents.insert(
        incident_id,
        Incident {
            id: incident_id,
            ..sample_incident()
        },
    );
    drop(incidents);

    // Pre-consume all 3 permits on the orchestrator's gate so the next
    // dispatch call is forced through the heuristic fallback.
    if let Some(orchestrator) = state.orchestrator.as_ref() {
        for _ in 0..3 {
            assert!(orchestrator.gate.try_acquire_for_test().await);
        }
        assert!(!orchestrator.gate.try_acquire_for_test().await);
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
    let envelopes: Vec<ToolCallEnvelope> =
        serde_json::from_value(body["recommendations"].clone()).expect("parse envelopes");
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0].arguments.incident_id, incident_id);
    // The justification text is the deterministic heuristic text, not an
    // LLM-refined variant — the gate forced the fallback path.
    assert!(
        envelopes[0].arguments.justification.contains("Hub"),
        "exhausted-gate dispatch should fall back to heuristic justification: got {:?}",
        envelopes[0].arguments.justification
    );
}

#[tokio::test]
async fn patch_incident_partial_update_persists_changes() {
    let state = AppState::new(None);
    let incident_id = {
        let mut incidents = state.incidents.write().await;
        let inc = sample_incident();
        let id = inc.id;
        incidents.insert(id, inc);
        id
    };

    let app = app::router_with_state(state.clone());
    let body = json!({ "casualty_count": 18, "severity_level": 5 });
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/incidents/{incident_id}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
    let updated: Incident = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("read body"),
    )
    .expect("parse incident");
    assert_eq!(updated.casualty_count, 18);
    assert_eq!(updated.severity_level, 5);
    // Title untouched (PATCH is partial).
    assert_eq!(updated.title, "Flooding in Mirpur");
}

#[tokio::test]
async fn patch_incident_rejects_invalid_severity() {
    let state = AppState::new(None);
    let incident_id = {
        let mut incidents = state.incidents.write().await;
        let inc = sample_incident();
        let id = inc.id;
        incidents.insert(id, inc);
        id
    };

    let app = app::router_with_state(state);
    let body = json!({ "severity_level": 7 });
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/incidents/{incident_id}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_incident_no_op_returns_current_row() {
    let state = AppState::new(None);
    let incident_id = {
        let mut incidents = state.incidents.write().await;
        let inc = sample_incident();
        let id = inc.id;
        incidents.insert(id, inc);
        id
    };

    let app = app::router_with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/incidents/{incident_id}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
    let row: Incident = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("read body"),
    )
    .expect("parse incident");
    assert_eq!(row.id, incident_id);
}

#[tokio::test]
async fn patch_incident_missing_returns_404() {
    let state = AppState::new(None);
    let app = app::router_with_state(state);
    let missing = Uuid::new_v4();
    let body = json!({ "casualty_count": 12 });
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/incidents/{missing}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("build request"),
        )
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
