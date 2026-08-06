//! Integration tests for the AI triage endpoint (data.md §7).
//!
//! These tests pin the **request and response payloads** the dashboard and
//! the smoke script rely on. Every assertion references a concrete JSON
//! field so future refactors that change the wire shape fail loudly
//! instead of silently breaking the UI.
//!
//! Coverage:
//! * Happy path — admin role can POST a triage request and receives a
//!   fully-populated response with the required fields.
//! * RBAC — request without `x-role` is rejected with 403.
//! * Validation — out-of-range severity / empty title returns 4xx.
//! * Heuristic fallback — without an orchestrator the endpoint still
//!   responds (no AI key required, same shape).
//! * Cache key stability — two requests with coordinates differing by
//!   < 0.01 deg collapse to the same cache key.
//! * Payload shape — the response always carries `mode`, `prediction`,
//!   `ai_attempted`, and the provider/model fields when applicable.
//!
//! All tests run against the in-memory router (no Postgres / Redis
//! required) so the CI loop stays fast.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use civic_sync_api::app;
use civic_sync_api::features::ai_triage::{TriageRequest, heuristic_prediction};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Minimal payload that satisfies the request validator. Mirrors the
/// `CreateIncident` body the dashboard sends, minus `primary_center_id`
/// which the server computes from coordinates.
const MINIMAL_TRIAGE_PAYLOAD: &str = r#"{
    "title": "Flash flood in Sylhet",
    "severity_level": 4,
    "affected_people": 250,
    "casualty_count": 12,
    "latitude": 24.8949,
    "longitude": 91.8687
}"#;

fn build_request(body: &str, role: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/ai/triage")
        .header("content-type", "application/json");
    if let Some(role) = role {
        builder = builder.header("x-role", role);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn assert_required_response_fields(body: &Value) {
    // Top-level envelope shape — judges / frontend rely on every field
    // below being present, even when the LLM is unavailable.
    assert!(body.get("generated_at").is_some(), "missing generated_at");
    assert!(body.get("mode").is_some(), "missing mode");
    let mode = body["mode"].as_str().expect("mode must be a string");
    assert!(
        matches!(mode, "llm" | "heuristic" | "cached"),
        "mode must be one of llm|heuristic|cached, got {mode}"
    );
    assert!(body.get("prediction").is_some(), "missing prediction");
    let prediction = &body["prediction"];
    assert!(
        prediction.get("severity").is_some(),
        "missing prediction.severity"
    );
    assert!(
        prediction.get("resource_kinds").is_some(),
        "missing prediction.resource_kinds"
    );
    assert!(
        prediction.get("rationale").is_some(),
        "missing prediction.rationale"
    );
    assert!(body.get("ai_attempted").is_some(), "missing ai_attempted");
    assert!(body.get("provider").is_some(), "missing provider");
    assert!(body.get("model").is_some(), "missing model");
    assert!(body.get("ai_error").is_some(), "missing ai_error");
}

#[tokio::test]
async fn triage_happy_path_returns_full_payload() {
    let app = app::router(None);
    let response = app
        .oneshot(build_request(MINIMAL_TRIAGE_PAYLOAD, Some("admin")))
        .await
        .expect("run request");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "expected 200 OK from /api/v1/ai/triage"
    );
    let body_bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read body");
    let body: Value = serde_json::from_slice(&body_bytes).expect("valid JSON");
    assert_required_response_fields(&body);
    // Heuristic fallback is the expected path because the test env has no
    // AI_PROVIDER / AI_MODEL / AI_API_KEY configured.
    assert_eq!(
        body["mode"], "heuristic",
        "expected heuristic mode without AI env vars"
    );
    assert_eq!(
        body["ai_attempted"], false,
        "no LLM call should be attempted without orchestrator"
    );
    assert!(
        body["provider"].is_null(),
        "provider must be null in heuristic mode"
    );
    // Severity must be echoed from the operator's input.
    assert_eq!(body["prediction"]["severity"], json!(4));
    // Resource kinds must be non-empty and contain only legal variants.
    let kinds = body["prediction"]["resource_kinds"]
        .as_array()
        .expect("resource_kinds must be array");
    assert!(!kinds.is_empty(), "resource_kinds must not be empty");
    for kind in kinds {
        let kind_str = kind.as_str().expect("kind must be string");
        assert!(
            matches!(
                kind_str,
                "AMBULANCE"
                    | "BOAT"
                    | "HELICOPTER"
                    | "RELIEF_TRUCK"
                    | "FOOD_PACK"
                    | "WATER_SUPPLY"
                    | "SHELTER_KIT"
                    | "MEDICAL_RATION"
            ),
            "unknown resource kind from heuristic: {kind_str}"
        );
    }
}

#[tokio::test]
async fn triage_rejects_request_without_role() {
    let app = app::router(None);
    let response = app
        .oneshot(build_request(MINIMAL_TRIAGE_PAYLOAD, None))
        .await
        .expect("run request");
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "RBAC must reject anonymous triage calls"
    );
}

#[tokio::test]
async fn triage_rejects_empty_title() {
    let payload = r#"{
        "title": "   ",
        "severity_level": 3,
        "affected_people": 10,
        "casualty_count": 0,
        "latitude": 23.81,
        "longitude": 90.41
    }"#;
    let app = app::router(None);
    let response = app
        .oneshot(build_request(payload, Some("admin")))
        .await
        .expect("run request");
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "empty title must be rejected as a validation error"
    );
}

#[tokio::test]
async fn triage_rejects_out_of_range_severity() {
    let payload = r#"{
        "title": "Fire at factory",
        "severity_level": 9,
        "affected_people": 5,
        "casualty_count": 0,
        "latitude": 23.81,
        "longitude": 90.41
    }"#;
    let app = app::router(None);
    let response = app
        .oneshot(build_request(payload, Some("admin")))
        .await
        .expect("run request");
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "severity 9 must be rejected as a validation error"
    );
}

#[tokio::test]
async fn triage_rejects_invalid_coordinates() {
    let payload = r#"{
        "title": "Out of range coords",
        "severity_level": 2,
        "affected_people": 1,
        "casualty_count": 0,
        "latitude": 999.0,
        "longitude": 0.0
    }"#;
    let app = app::router(None);
    let response = app
        .oneshot(build_request(payload, Some("admin")))
        .await
        .expect("run request");
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "out-of-range latitude must be rejected"
    );
}

#[tokio::test]
async fn triage_supports_dispatcher_role() {
    // RBAC allows both admin and dispatcher for mutations; triage is a
    // read-only call but the middleware path is shared.
    let app = app::router(None);
    let response = app
        .oneshot(build_request(MINIMAL_TRIAGE_PAYLOAD, Some("dispatcher")))
        .await
        .expect("run request");
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn heuristic_prediction_returns_flood_resources_for_flood_titles() {
    let req = TriageRequest {
        title: "Flash flood in Sylhet".into(),
        severity_level: 5,
        affected_people: 1200,
        casualty_count: 35,
        latitude: 24.8949,
        longitude: 91.8687,
    };
    let pred = heuristic_prediction(&req);
    assert_eq!(pred.severity, 5);
    assert!(pred.resource_kinds.contains(&"BOAT".to_string()));
    assert!(pred.resource_kinds.contains(&"RELIEF_TRUCK".to_string()));
}

#[test]
fn heuristic_prediction_returns_medical_resources_for_medical_titles() {
    let req = TriageRequest {
        title: "Mass casualty at hospital".into(),
        severity_level: 4,
        affected_people: 50,
        casualty_count: 15,
        latitude: 23.81,
        longitude: 90.41,
    };
    let pred = heuristic_prediction(&req);
    assert_eq!(pred.severity, 4);
    assert!(pred.resource_kinds.contains(&"AMBULANCE".to_string()));
}

#[test]
fn cache_key_is_stable_across_coordinate_jitter() {
    // Choose coordinates that round to the same 0.01-deg bucket.
    let a = TriageRequest {
        title: "Flood".into(),
        severity_level: 3,
        affected_people: 100,
        casualty_count: 4,
        latitude: 23.811,
        longitude: 90.411,
    };
    let b = TriageRequest {
        title: "Flood".into(),
        severity_level: 3,
        affected_people: 100,
        casualty_count: 4,
        latitude: 23.814,
        longitude: 90.414,
    };
    assert_eq!(
        a.cache_key(),
        b.cache_key(),
        "coordinates within the same 0.01-deg bucket must collapse to one cache key"
    );
}

#[test]
fn cache_key_differs_on_casualty_delta() {
    let a = TriageRequest {
        title: "Flood".into(),
        severity_level: 3,
        affected_people: 100,
        casualty_count: 4,
        latitude: 23.81,
        longitude: 90.41,
    };
    let b = TriageRequest {
        title: "Flood".into(),
        severity_level: 3,
        affected_people: 100,
        casualty_count: 8,
        latitude: 23.81,
        longitude: 90.41,
    };
    assert_ne!(a.cache_key(), b.cache_key());
}

#[test]
fn cache_key_ignores_punctuation_and_case() {
    let a = TriageRequest {
        title: "Flash Flood!!! in SYLHET".into(),
        severity_level: 3,
        affected_people: 100,
        casualty_count: 4,
        latitude: 23.81,
        longitude: 90.41,
    };
    let b = TriageRequest {
        title: "flash flood in sylhet".into(),
        severity_level: 3,
        affected_people: 100,
        casualty_count: 4,
        latitude: 23.81,
        longitude: 90.41,
    };
    assert_eq!(a.cache_key(), b.cache_key());
}

#[tokio::test]
async fn response_includes_latency_ms_field() {
    let app = app::router(None);
    let response = app
        .oneshot(build_request(MINIMAL_TRIAGE_PAYLOAD, Some("admin")))
        .await
        .expect("run request");
    let body_bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read body");
    let body: Value = serde_json::from_slice(&body_bytes).expect("valid JSON");
    // latency_ms is always present: Some(ms) when LLM ran, None otherwise.
    assert!(
        body.get("latency_ms").is_some(),
        "response must include latency_ms (Some on success, None on heuristic/cached)"
    );
}
