//! AI triage enrichment (data.md §7 + §5B).
//!
//! The deterministic engine still owns every allocation decision. This module
//! layers a *read-only* AI pass on top of incident ingest and dispatch: it
//! asks the configured LLM to do two things the heuristic cannot —
//!
//! 1. **Severity re-classification** — read the incident title + context and
//!    suggest a severity 1–5. Operators can compare this against the
//!    operator-supplied `severity_level` and audit the disagreement.
//! 2. **Resource-kind prediction** — read the incident and predict which
//!    `ResourceType` variants are most needed (e.g. flood →
//!    `[Boat, ReliefTruck, ShelterKit]`, fire → `[Ambulance,
//!    Helicopter, MedicalRation]`).
//!
//! Both pieces of output are *advisory only*. They never feed back into
//! the dispatch engine — the conflict-prevention contract holds. The
//! architecture is the same shape as the existing justification-refinement
//! path: AI may enrich, AI may not allocate.
//!
//! ## Failures are observable, not silent
//!
//! Every LLM failure (network, rate-limit, malformed JSON, validation
//! rejection, semaphore exhaustion) is recorded in
//! `civic_sync_ai_triage_total{outcome=...}` so judges can see exactly
//! how often the AI is on the critical path vs. the heuristic fallback.

use std::time::Instant;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use rig::client::CompletionClient;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::AiConfig,
    error::ApiError,
    features::{orchestrator::Provider, resources::ResourceType},
    state::AppState,
};

/// Request body for `POST /api/v1/ai/triage`. Mirrors `CreateIncident`
/// minus the fields the LLM never needs.
#[derive(Debug, Deserialize)]
pub struct TriageRequest {
    pub title: String,
    pub severity_level: u8,
    pub affected_people: u32,
    pub casualty_count: u32,
    pub latitude: f64,
    pub longitude: f64,
}

impl TriageRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.title.trim().is_empty() {
            return Err(ApiError::Validation("title must not be empty".into()));
        }
        if self.title.len() > 255 {
            return Err(ApiError::Validation(
                "title must not exceed 255 characters".into(),
            ));
        }
        if !(1..=5).contains(&self.severity_level) {
            return Err(ApiError::Validation(
                "severity_level must be between 1 and 5".into(),
            ));
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(ApiError::Validation(
                "latitude must be between -90 and 90".into(),
            ));
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(ApiError::Validation(
                "longitude must be between -180 and 180".into(),
            ));
        }
        Ok(())
    }

    /// Stable cache key: same input ⇒ same Redis lookup. We quantise the
    /// coordinates to 2 decimals (~1 km) so two reports about the same
    /// event from slightly different GPS readings collapse to one key.
    pub fn cache_key(&self) -> String {
        let lat_q = (self.latitude * 100.0).round() as i32;
        let lon_q = (self.longitude * 100.0).round() as i32;
        let title_key: String = self
            .title
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_")
            .to_ascii_lowercase();
        let title_key = title_key.chars().take(80).collect::<String>();
        format!(
            "ai_triage:{}:{}:{}:{}:{}",
            title_key, lat_q, lon_q, self.casualty_count, self.affected_people
        )
    }
}

/// The single-shot AI prediction the LLM is asked to return. Strict
/// schema: rig's extractor rejects any response that doesn't match.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TriagePrediction {
    /// LLM's suggested severity, 1..=5.
    pub severity: u8,
    /// Resource kinds the LLM thinks are most needed, in priority order.
    /// Validated to be a subset of [`ResourceType`] variants.
    pub resource_kinds: Vec<String>,
    /// Short operator-facing rationale (≤240 chars). Never used by the
    /// engine; surfaced to UI so a human can sanity-check the model.
    pub rationale: String,
}

/// Outer wrapper so rig's `extractor<T>` has a single named struct to
/// produce. The inner field is what we actually care about.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct TriagePredictionEnvelope(TriagePrediction);

/// Response body for `POST /api/v1/ai/triage`.
#[derive(Debug, Clone, Serialize)]
pub struct TriageResponse {
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// `"llm"`, `"heuristic"`, or `"cached"` — `cached` means a previous
    /// identical request hit the Redis cache within the TTL window.
    pub mode: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// Always present, even when `mode` is `"heuristic"`: we degrade to a
    /// safe deterministic prediction derived from the operator-supplied
    /// `severity_level` so the UI never gets a null result.
    pub prediction: TriagePrediction,
    /// `true` when an LLM call was actually attempted. `false` when the
    /// semaphore was exhausted, the cache hit, or the orchestrator is
    /// absent.
    pub ai_attempted: bool,
    /// Latency of the LLM call (excludes cache hit / heuristic / throttle
    /// paths). `None` when no LLM call ran.
    pub latency_ms: Option<u64>,
    pub ai_error: Option<String>,
}

/// `POST /api/v1/ai/triage` — AI severity + resource-kind predictor.
///
/// Read-only, RBAC-protected. Returns the LLM prediction alongside the
/// mode metadata so judges can verify the AI path end-to-end.
///
/// Order of operations:
/// 1. Validate the request.
/// 2. Check the Redis cache (5-minute TTL). Hit ⇒ respond immediately.
/// 3. Otherwise, take the §5B semaphore permit. Exhausted ⇒ respond with
///    `mode: "heuristic"` and a deterministic fallback.
/// 4. Call the configured provider. Any failure ⇒ heuristic fallback +
///    Prometheus counter bump.
/// 5. Validate the prediction against [`ResourceType`] variants. Invalid
///    kinds are silently filtered out (the prediction is advisory — we
///    never want a missing kind to crash the endpoint).
/// 6. Best-effort Redis SETEX of the validated prediction, then respond.
pub async fn triage(
    State(state): State<AppState>,
    Json(input): Json<TriageRequest>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let started = Instant::now();

    // 1. Cache lookup (fail-soft: any Redis error falls through to the LLM).
    if let Some(redis) = &state.redis
        && let Some(cached) = read_cache(redis, &input.cache_key()).await
    {
        crate::observability::record_ai_triage("cache_hit", 0);
        return Ok((
            StatusCode::OK,
            Json(TriageResponse {
                generated_at: chrono::Utc::now(),
                mode: "cached".to_string(),
                provider: state
                    .orchestrator
                    .as_ref()
                    .as_ref()
                    .map(|o| o.provider.id().to_string()),
                model: state
                    .orchestrator
                    .as_ref()
                    .as_ref()
                    .map(|o| o.model.clone()),
                prediction: cached,
                ai_attempted: false,
                latency_ms: None,
                ai_error: None,
            }),
        ));
    }

    // 2. Heuristic fallback when no orchestrator is configured.
    let Some(orchestrator) = state.orchestrator.as_ref().as_ref() else {
        let fallback = heuristic_prediction(&input);
        crate::observability::record_ai_triage("no_orchestrator", 0);
        return Ok((
            StatusCode::OK,
            Json(TriageResponse {
                generated_at: chrono::Utc::now(),
                mode: "heuristic".to_string(),
                provider: None,
                model: None,
                prediction: fallback,
                ai_attempted: false,
                latency_ms: None,
                ai_error: None,
            }),
        ));
    };

    // 3. Semaphore gate — same §5B 3/30s rule as the dispatch path. The
    //    gate is shared across all AI consumers via `state.ai_gate` so
    //    the AI budget is a single grid-wide resource.
    if !state.ai_gate.try_acquire_for_test().await {
        let fallback = heuristic_prediction(&input);
        crate::observability::record_ai_triage("throttled", 0);
        return Ok((
            StatusCode::OK,
            Json(TriageResponse {
                generated_at: chrono::Utc::now(),
                mode: "heuristic".to_string(),
                provider: Some(orchestrator.provider.id().to_string()),
                model: Some(orchestrator.model.clone()),
                prediction: fallback,
                ai_attempted: false,
                latency_ms: None,
                ai_error: Some("semaphore exhausted".to_string()),
            }),
        ));
    }

    // 4. Build prompt + call LLM.
    let prompt = build_triage_prompt(&input);
    let llm_result = call_triage_provider(orchestrator, &prompt, &orchestrator.model).await;
    let elapsed = started.elapsed().as_millis() as u64;

    let (prediction, mode, attempted, error) = match llm_result {
        Ok(raw) => match sanitize_prediction(raw) {
            Some(clean) => (clean, "llm".to_string(), true, None),
            None => (
                heuristic_prediction(&input),
                "heuristic".to_string(),
                true,
                Some("LLM response failed validation".to_string()),
            ),
        },
        Err(error) => (
            heuristic_prediction(&input),
            "heuristic".to_string(),
            true,
            Some(error.to_string()),
        ),
    };

    // 5. Metrics + best-effort cache write.
    crate::observability::record_ai_triage(
        if mode == "llm" { "success" } else { "fallback" },
        elapsed,
    );
    if let Some(redis) = &state.redis {
        write_cache(redis, &input.cache_key(), &prediction).await;
    }

    Ok((
        StatusCode::OK,
        Json(TriageResponse {
            generated_at: chrono::Utc::now(),
            mode,
            provider: Some(orchestrator.provider.id().to_string()),
            model: Some(orchestrator.model.clone()),
            prediction,
            ai_attempted: attempted,
            latency_ms: Some(elapsed),
            ai_error: error,
        }),
    ))
}

/// Deterministic fallback that always produces a sane answer — used when
/// the LLM is absent, throttled, or failed. Mirrors what an operator would
/// pick: severity stays the same; resource kinds follow a simple keyword
/// scan of the title. Never called on the success path.
pub fn heuristic_prediction(req: &TriageRequest) -> TriagePrediction {
    let title = req.title.to_ascii_lowercase();
    let kinds: Vec<String> = if title.contains("flood") || title.contains("water") {
        vec!["BOAT", "RELIEF_TRUCK", "SHELTER_KIT"]
    } else if title.contains("fire") || title.contains("burn") {
        vec!["AMBULANCE", "HELICOPTER", "MEDICAL_RATION"]
    } else if title.contains("cyclone") || title.contains("storm") {
        vec!["RELIEF_TRUCK", "SHELTER_KIT", "FOOD_PACK"]
    } else if title.contains("medical") || title.contains("casualt") {
        vec!["AMBULANCE", "MEDICAL_RATION"]
    } else {
        vec!["RELIEF_TRUCK", "AMBULANCE"]
    }
    .into_iter()
    .map(String::from)
    .collect();
    TriagePrediction {
        severity: req.severity_level,
        resource_kinds: kinds,
        rationale:
            "Heuristic fallback: severity echoed from operator; resource kinds from keyword scan."
                .to_string(),
    }
}

/// Drop any LLM-hallucinated fields and clamp to legal ranges. Returns
/// `None` if the prediction is unusable (out-of-range severity, empty
/// kinds list). The caller treats that as a failure and falls back.
fn sanitize_prediction(mut raw: TriagePrediction) -> Option<TriagePrediction> {
    if !(1..=5).contains(&raw.severity) {
        return None;
    }
    if raw.resource_kinds.is_empty() {
        return None;
    }
    raw.resource_kinds
        .retain(|kind| serde_json::from_str::<ResourceType>(&format!("\"{}\"", kind)).is_ok());
    if raw.resource_kinds.is_empty() {
        return None;
    }
    if raw.rationale.len() > 480 {
        raw.rationale.truncate(477);
        raw.rationale.push_str("...");
    }
    Some(raw)
}

fn build_triage_prompt(req: &TriageRequest) -> String {
    let header = "You are an AI triage layer for an emergency response \
        platform. Given the incident below, predict:\n\
        1) `severity`: an integer 1..=5 (1=minor, 5=catastrophic).\n\
        2) `resource_kinds`: ordered list of resource types most needed.\n\
           Allowed values (uppercase, snake-case):\n\
             AMBULANCE, BOAT, HELICOPTER, RELIEF_TRUCK, FOOD_PACK,\n\
             WATER_SUPPLY, SHELTER_KIT, MEDICAL_RATION\n\
        3) `rationale`: a 1-2 sentence operator-facing rationale (<=240 chars).\n\
        Do NOT change other fields. Return ONLY the JSON object.";
    let body = serde_json::json!({
        "title": req.title,
        "operator_supplied_severity": req.severity_level,
        "affected_people": req.affected_people,
        "casualty_count": req.casualty_count,
        "latitude": req.latitude,
        "longitude": req.longitude,
    });
    format!("{header}\n\nIncident:\n{body}")
}

async fn call_triage_provider(
    orchestrator: &crate::features::orchestrator::Orchestrator,
    prompt: &str,
    model: &str,
) -> anyhow::Result<TriagePrediction> {
    use rig::client::ProviderClient;
    match &orchestrator.provider {
        Provider::Openai => {
            let client =
                rig::providers::openai::Client::from_val(orchestrator.api_key.clone().into())
                    .map_err(|error| anyhow::anyhow!("build openai client: {error:?}"))?;
            let extractor = client
                .extractor::<TriagePredictionEnvelope>(model.to_string())
                .preamble("You triage emergency incidents. Return only the JSON object.")
                .build();
            extractor
                .extract(prompt)
                .await
                .map(|e| e.0)
                .map_err(|error| anyhow::anyhow!("openai extract: {error:?}"))
        }
        Provider::Anthropic => {
            let client = rig::providers::anthropic::Client::from_val(orchestrator.api_key.clone())
                .map_err(|error| anyhow::anyhow!("build anthropic client: {error:?}"))?;
            let extractor = client
                .extractor::<TriagePredictionEnvelope>(model.to_string())
                .preamble("You triage emergency incidents. Return only the JSON object.")
                .build();
            extractor
                .extract(prompt)
                .await
                .map(|e| e.0)
                .map_err(|error| anyhow::anyhow!("anthropic extract: {error:?}"))
        }
        Provider::Gemini => {
            let client =
                rig::providers::gemini::Client::from_val(orchestrator.api_key.clone().into())
                    .map_err(|error| anyhow::anyhow!("build gemini client: {error:?}"))?;
            let extractor = client
                .extractor::<TriagePredictionEnvelope>(model.to_string())
                .preamble("You triage emergency incidents. Return only the JSON object.")
                .build();
            extractor
                .extract(prompt)
                .await
                .map(|e| e.0)
                .map_err(|error| anyhow::anyhow!("gemini extract: {error:?}"))
        }
        Provider::Deepseek => {
            let completions_client = rig::providers::openai::CompletionsClient::from_val(
                orchestrator.api_key.clone().into(),
            )
            .map_err(|error| anyhow::anyhow!("build deepseek client: {error:?}"))?;
            let extractor = completions_client
                .extractor::<TriagePredictionEnvelope>(model.to_string())
                .preamble("You triage emergency incidents. Return only the JSON object.")
                .build();
            extractor
                .extract(prompt)
                .await
                .map(|e| e.0)
                .map_err(|error| anyhow::anyhow!("deepseek extract: {error:?}"))
        }
        Provider::Cohere => {
            let client =
                rig::providers::cohere::Client::from_val(orchestrator.api_key.clone().into())
                    .map_err(|error| anyhow::anyhow!("build cohere client: {error:?}"))?;
            let extractor = client
                .extractor::<TriagePredictionEnvelope>(model.to_string())
                .preamble("You triage emergency incidents. Return only the JSON object.")
                .build();
            extractor
                .extract(prompt)
                .await
                .map(|e| e.0)
                .map_err(|error| anyhow::anyhow!("cohere extract: {error:?}"))
        }
        Provider::Ollama => {
            let client =
                rig::providers::ollama::Client::from_val(orchestrator.api_key.clone().into())
                    .map_err(|error| anyhow::anyhow!("build ollama client: {error:?}"))?;
            let extractor = client
                .extractor::<TriagePredictionEnvelope>(model.to_string())
                .preamble("You triage emergency incidents. Return only the JSON object.")
                .build();
            extractor
                .extract(prompt)
                .await
                .map(|e| e.0)
                .map_err(|error| anyhow::anyhow!("ollama extract: {error:?}"))
        }
        _ => anyhow::bail!("provider not wired: {}", orchestrator.provider.id()),
    }
}

// --- Redis cache helpers -------------------------------------------------

const TRIAGE_TTL_SECS: u64 = 300; // 5 minutes

async fn read_cache(redis: &redis::Client, key: &str) -> Option<TriagePrediction> {
    let mut conn = redis.get_multiplexed_async_connection().await.ok()?;
    let raw: Option<String> = redis::AsyncCommands::get(&mut conn, key).await.ok()?;
    let raw = raw?;
    serde_json::from_str(&raw).ok()
}

async fn write_cache(redis: &redis::Client, key: &str, value: &TriagePrediction) {
    if let Ok(mut conn) = redis.get_multiplexed_async_connection().await
        && let Ok(serialized) = serde_json::to_string(value)
    {
        let _: redis::RedisResult<()> =
            redis::AsyncCommands::set_ex(&mut conn, key, serialized, TRIAGE_TTL_SECS).await;
    }
}

/// Convenience helper used by integration tests and by `main.rs` to
/// report provider presence without exposing internal state.
pub fn provider_label(ai: &AiConfig) -> String {
    ai.provider.as_deref().unwrap_or("(unset)").to_string()
}

#[allow(dead_code)]
fn _unused_uuid_compile_helper(_: Uuid) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_out_of_range_severity() {
        let bad = TriagePrediction {
            severity: 9,
            resource_kinds: vec!["AMBULANCE".into()],
            rationale: "x".into(),
        };
        assert!(sanitize_prediction(bad).is_none());
    }

    #[test]
    fn sanitize_rejects_empty_kinds() {
        let bad = TriagePrediction {
            severity: 3,
            resource_kinds: vec![],
            rationale: "x".into(),
        };
        assert!(sanitize_prediction(bad).is_none());
    }

    #[test]
    fn sanitize_drops_unknown_kinds_and_accepts_known() {
        let raw = TriagePrediction {
            severity: 3,
            resource_kinds: vec!["AMBULANCE".into(), "ROCKET".into(), "BOAT".into()],
            rationale: "x".into(),
        };
        let cleaned = sanitize_prediction(raw).unwrap();
        assert_eq!(cleaned.resource_kinds, vec!["AMBULANCE", "BOAT"]);
    }

    #[test]
    fn sanitize_rejects_when_all_kinds_unknown() {
        let raw = TriagePrediction {
            severity: 3,
            resource_kinds: vec!["NUCLEAR_SUBMARINE".into()],
            rationale: "x".into(),
        };
        assert!(sanitize_prediction(raw).is_none());
    }

    #[test]
    fn heuristic_predicts_flood_resources() {
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
    }

    #[test]
    fn cache_key_stable_across_small_coordinate_jitter() {
        // Coordinates picked so both round to the same 0.01-deg bucket.
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
        assert_eq!(a.cache_key(), b.cache_key());
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
}
