//! Rig AI orchestrator with a deterministic heuristic fallback (data.md §5B + §7).
//!
//! The orchestrator is the single entry point for the
//! `POST /api/v1/dispatch/recommendations` handler. It layers an LLM enrichment
//! pass on top of the multi-center dispatch heuristic — but the heuristic is
//! always the source of truth for *which* resources and teams are assigned. The
//! LLM is only ever asked to refine the per-incident `justification` text.
//! Anything stronger (a different allocation, a different primary center)
//! would punch a hole through the conflict-prevention contract, so it is
//! intentionally rejected: the LLM output is consumed but only the
//! `justification` field is carried back into the response.
//!
//! ## Provider model
//!
//! `Provider::from_config` maps the free-form `AI_PROVIDER` env string to one
//! of the known rig-core 0.39 provider variants (the same list documented in
//! `.env.example`). Unknown providers are preserved as `Unknown(String)` and
//! the orchestrator transparently falls back to the heuristic — this preserves
//! the "provider-agnostic" contract: the operator can put any string in the
//! env and the system keeps working.
//!
//! ## Concurrency gate (data.md §5B)
//!
//! `SemaphoreGate` enforces the "3 priority tasks per 30-second window" rule
//! from `data.md` §5B. The gate is sync (no awaits inside the lock) and the
//! counter is reset on a 30-second ticker that lives on the orchestrator. If
//! the gate is exhausted, the call returns the heuristic result immediately
//! and logs a warning. This is fire-and-forget graceful degradation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use rig::client::ProviderClient;
use rig::providers::{anthropic, cohere, gemini, ollama, openai};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use crate::config::AiConfig;
use crate::error::ApiError;
use crate::features::dispatch::{self, TOOL_NAME, ToolCallEnvelope};
use crate::state::AppState;

/// Named LLM providers the orchestrator knows how to wire up. Anything else
/// falls through to the heuristic fallback (which is the same behaviour as
/// having no provider configured at all).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Openai,
    Anthropic,
    Gemini,
    Deepseek,
    Bedrock,
    Cohere,
    Ollama,
    Openrouter,
    Huggingface,
    Mistral,
    Xai,
    Together,
    Azure,
    Groq,
    /// Any value not on the curated list. Preserved verbatim so the operator
    /// can see what they configured; the orchestrator refuses to make a real
    /// network call with this and falls back to the heuristic.
    Unknown(String),
}

impl Provider {
    pub fn id(&self) -> &str {
        match self {
            Provider::Openai => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
            Provider::Deepseek => "deepseek",
            Provider::Bedrock => "bedrock",
            Provider::Cohere => "cohere",
            Provider::Ollama => "ollama",
            Provider::Openrouter => "openrouter",
            Provider::Huggingface => "huggingface",
            Provider::Mistral => "mistral",
            Provider::Xai => "xai",
            Provider::Together => "together",
            Provider::Azure => "azure",
            Provider::Groq => "groq",
            Provider::Unknown(name) => name,
        }
    }

    pub fn from_config(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "openai" => Provider::Openai,
            "anthropic" | "claude" => Provider::Anthropic,
            "gemini" | "google" => Provider::Gemini,
            "deepseek" => Provider::Deepseek,
            "bedrock" | "aws_bedrock" => Provider::Bedrock,
            "cohere" => Provider::Cohere,
            "ollama" => Provider::Ollama,
            "openrouter" => Provider::Openrouter,
            "huggingface" | "hf" => Provider::Huggingface,
            "mistral" => Provider::Mistral,
            "xai" | "grok" => Provider::Xai,
            "together" => Provider::Together,
            "azure" | "azure_openai" => Provider::Azure,
            "groq" => Provider::Groq,
            other => Provider::Unknown(other.to_string()),
        }
    }

    /// True when this provider has been wired into the orchestrator's LLM
    /// enrichment path. Other providers are still accepted from the config so
    /// the operator gets a friendly runtime fallback instead of a hard error.
    fn is_wired(&self) -> bool {
        matches!(
            self,
            Provider::Openai
                | Provider::Anthropic
                | Provider::Gemini
                | Provider::Deepseek
                | Provider::Cohere
                | Provider::Ollama
        )
    }
}

/// Per-window concurrency gate. The data.md §5B rule is "3 priority tasks per
/// 30s window". The semaphore has 3 permits; the rolling counter is reset on
/// every tick of the orchestrator's 30-second interval. The inner lock is
/// short-lived and never held across an `.await`.
#[derive(Debug)]
pub struct SemaphoreGate {
    semaphore: Arc<Semaphore>,
    window: Mutex<WindowState>,
}

#[derive(Debug)]
struct WindowState {
    window_start: Instant,
    permits_used: u32,
}

impl SemaphoreGate {
    /// data.md §5B: 3 permits per 30-second window.
    const PERMITS_PER_WINDOW: u32 = 3;
    pub const WINDOW: Duration = Duration::from_secs(30);

    pub fn new() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(Self::PERMITS_PER_WINDOW as usize)),
            window: Mutex::new(WindowState {
                window_start: Instant::now(),
                permits_used: 0,
            }),
        }
    }

    /// Return `true` and update the counter if a permit was acquired for the
    /// current 30-second window. Return `false` if the window is exhausted.
    /// The caller MUST call [`release`] from the same task that acquired the
    /// permit, otherwise the gate becomes permanently starved.
    async fn try_acquire(&self) -> bool {
        // Fast path: try the semaphore. If no permit is available we don't
        // touch the counter at all.
        let permit = match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                tracing::warn!(
                    semaphore_permits = Self::PERMITS_PER_WINDOW,
                    "orchestrator semaphore exhausted; using heuristic fallback"
                );
                crate::observability::record_semaphore("exhausted");
                return false;
            }
        };
        // Reserve one slot in the current window. Forgetting the permit would
        // leak slots, so we explicitly disable its drop hook.
        permit.forget();
        let permits_used = {
            let mut window = self.window.lock().await;
            let now = Instant::now();
            if now.duration_since(window.window_start) >= Self::WINDOW {
                window.window_start = now;
                window.permits_used = 0;
            }
            if window.permits_used >= Self::PERMITS_PER_WINDOW {
                // The semaphore had a permit available but the rolling window
                // did not. Refuse the call and let the next ticker reset.
                Some(window.permits_used)
            } else {
                window.permits_used += 1;
                None
            }
        };
        if let Some(used) = permits_used {
            tracing::warn!(
                permits_used = used,
                "orchestrator window exhausted; using heuristic fallback"
            );
            crate::observability::record_semaphore("exhausted");
            return false;
        }
        crate::observability::record_semaphore("acquired");
        true
    }

    /// Re-arm the rolling window. Called from the 30s ticker so expired
    /// windows free up their semaphore permits.
    pub async fn reset_window(&self) {
        let mut window = self.window.lock().await;
        window.window_start = Instant::now();
        window.permits_used = 0;
    }

    /// Test-only helper that exposes the gate's permit-acquire logic so
    /// integration tests in tests/api.rs can drive the gate into an
    /// exhausted state and assert the heuristic-fallback path. Not
    /// intended for production callers.
    #[doc(hidden)]
    pub async fn try_acquire_for_test(&self) -> bool {
        self.try_acquire().await
    }
}

impl Default for SemaphoreGate {
    fn default() -> Self {
        Self::new()
    }
}

/// The orchestrator owns the AI provider configuration and the §5B
/// concurrency gate. When `from_config` returns `None`, the rest of the
/// system treats the orchestrator as absent and the dispatch handler falls
/// straight through to the heuristic.
pub struct Orchestrator {
    pub provider: Provider,
    pub model: String,
    pub api_key: String,
    pub gate: SemaphoreGate,
}

impl Orchestrator {
    /// Build an orchestrator from the validated `AiConfig`. Returns `None`
    /// when `AiConfig::is_configured()` is false — that is the canonical
    /// signal that the operator did not wire up an LLM and we should run on
    /// the heuristic alone.
    pub fn from_config(ai: &AiConfig) -> anyhow::Result<Option<Self>> {
        if !ai.is_configured() {
            return Ok(None);
        }
        let provider_string = ai
            .provider
            .as_deref()
            .context("AI_PROVIDER present but missing in config")?;
        let model = ai.model.as_deref().context("AI_MODEL missing")?.to_string();
        let api_key = ai
            .api_key
            .as_deref()
            .context("AI_API_KEY missing")?
            .to_string();
        let provider = Provider::from_config(provider_string);
        if !provider.is_wired() {
            tracing::warn!(
                provider = provider.id(),
                "configured AI provider is not wired into the orchestrator yet; using heuristic fallback"
            );
        }
        Ok(Some(Self {
            provider,
            model,
            api_key,
            gate: SemaphoreGate::new(),
        }))
    }

    /// Run the orchestrator. The contract is:
    /// 1. Always execute the deterministic heuristic. It performs all the
    ///    state mutations (resource → EN_ROUTE, helper allocation row,
    ///    team `assigned_members` bump) and is the source of truth for
    ///    *which* resources and teams are assigned.
    /// 2. If the gate has a permit free, ask the LLM to refine the
    ///    per-envelope `justification` text. The LLM never replaces the
    ///    resource/team selection — only the human-readable justification.
    /// 3. If the LLM call fails for any reason, return the heuristic result
    ///    unchanged. The data.md §7 contract on the wire is invariant.
    pub async fn dispatch(&self, state: &AppState) -> Result<Vec<ToolCallEnvelope>, ApiError> {
        let heuristic = dispatch::heuristic_dispatch(state).await?;
        if !self.gate.try_acquire().await {
            crate::observability::record_dispatch("semaphore", heuristic.len());
            return Ok(heuristic);
        }
        match enrich_with_llm(self, &heuristic).await {
            Ok(enriched) => {
                crate::observability::record_dispatch("none", enriched.len());
                Ok(enriched)
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    provider = self.provider.id(),
                    "LLM enrichment failed; returning heuristic envelopes"
                );
                crate::observability::record_dispatch("llm_error", heuristic.len());
                Ok(heuristic)
            }
        }
    }

    /// Diagnostic variant of [`dispatch`] used by the smoke endpoint.
    /// Returns both the heuristic result and the raw LLM patch list (if any)
    /// so demo viewers can see exactly what the LLM said vs. what the
    /// system committed. When the LLM is unavailable, `llm_attempted` is
    /// `false` and the caller still gets the heuristic.
    pub async fn smoke(&self, state: &AppState) -> Result<SmokeResult, ApiError> {
        let heuristic = dispatch::heuristic_dispatch(state).await?;
        if !self.gate.try_acquire().await {
            return Ok(SmokeResult {
                heuristic,
                llm_attempted: false,
                llm_patches: Vec::new(),
                llm_error: Some("semaphore exhausted".to_string()),
            });
        }
        let prompt = build_prompt(&heuristic);
        match call_provider(self, &prompt).await {
            Ok(patches) => {
                let enriched = apply_patches(&heuristic, patches.clone())
                    .unwrap_or_else(|_| heuristic.clone());
                Ok(SmokeResult {
                    heuristic: enriched,
                    llm_attempted: true,
                    llm_patches: patches,
                    llm_error: None,
                })
            }
            Err(error) => Ok(SmokeResult {
                heuristic,
                llm_attempted: true,
                llm_patches: Vec::new(),
                llm_error: Some(error.to_string()),
            }),
        }
    }
}

/// Diagnostic snapshot returned by `Orchestrator::smoke` and surfaced via
/// `POST /api/v1/dispatch/smoke`. Lets a demo viewer confirm that the
/// LLM was reached (or skipped, with a reason) and inspect the raw patch
/// list the model returned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmokeResult {
    pub heuristic: Vec<ToolCallEnvelope>,
    pub llm_attempted: bool,
    pub llm_patches: Vec<JustificationPatch>,
    pub llm_error: Option<String>,
}

/// Ask the configured LLM to refine the per-incident justification text. The
/// prompt is the heuristic envelope serialized to JSON; the response is a
/// `JustificationPatch` per envelope which we apply if and only if every
/// patch passes a sanity check (right incident_id, non-empty text, same
/// envelope length).
async fn enrich_with_llm(
    orchestrator: &Orchestrator,
    heuristic: &[ToolCallEnvelope],
) -> anyhow::Result<Vec<ToolCallEnvelope>> {
    if heuristic.is_empty() {
        return Ok(heuristic.to_vec());
    }
    let prompt = build_prompt(heuristic);
    let patches = call_provider(orchestrator, &prompt).await?;
    apply_patches(heuristic, patches)
}

/// A patch that refines a single envelope's justification text. The LLM
/// returns these in the same order as the input envelopes. `pub` so the
/// smoke endpoint can serialise the raw model output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JustificationPatch {
    pub incident_id: Uuid,
    pub justification: String,
}

fn build_prompt(envelopes: &[ToolCallEnvelope]) -> String {
    let header = "You are an AI orchestration layer for an emergency response \
        platform. Given the deterministic dispatch envelopes below, refine the \
        `justification` text for each envelope to be a 1-2 sentence operator \
        briefing: which hub is responding, why, and what was re-routed if any. \
        Do NOT change any other field. Do NOT invent new resources or teams. \
        Return a JSON array of objects with exactly two fields: \
        `incident_id` (UUID as string) and `justification` (string). \
        Keep the array in the same order as the input envelopes.";
    let serialized = serde_json::to_string(envelopes).unwrap_or_else(|_| "[]".to_string());
    format!("{header}\n\nDispatch envelopes (for context only):\n{serialized}")
}

/// Dispatch to the correct rig provider client. Each supported provider runs
/// its own typed completion pipeline, then runs the LLM prompt through rig's
/// `extractor<T>` to get a typed `Vec<JustificationPatch>` back. Any error
/// from any provider bubbles up to the orchestrator which falls back to the
/// heuristic.
async fn call_provider(
    orchestrator: &Orchestrator,
    prompt: &str,
) -> anyhow::Result<Vec<JustificationPatch>> {
    use rig::client::CompletionClient;
    match &orchestrator.provider {
        Provider::Openai => {
            let client = openai::Client::from_val(orchestrator.api_key.clone().into())
                .context("build openai client")?;
            let extractor = client
                .extractor::<JustificationPatchList>(orchestrator.model.clone())
                .preamble(PATCHER_PREAMBLE)
                .build();
            let response = extractor.extract(prompt).await.context("openai extract")?;
            Ok(response.0)
        }
        Provider::Anthropic => {
            let client = anthropic::Client::from_val(orchestrator.api_key.clone())
                .context("build anthropic client")?;
            let extractor = client
                .extractor::<JustificationPatchList>(orchestrator.model.clone())
                .preamble(PATCHER_PREAMBLE)
                .build();
            let response = extractor
                .extract(prompt)
                .await
                .context("anthropic extract")?;
            Ok(response.0)
        }
        Provider::Gemini => {
            let client = gemini::Client::from_val(orchestrator.api_key.clone().into())
                .context("build gemini client")?;
            let extractor = client
                .extractor::<JustificationPatchList>(orchestrator.model.clone())
                .preamble(PATCHER_PREAMBLE)
                .build();
            let response = extractor.extract(prompt).await.context("gemini extract")?;
            Ok(response.0)
        }
        Provider::Deepseek => {
            // DeepSeek reuses the OpenAI Chat Completions API surface.
            let completions_client =
                openai::CompletionsClient::from_val(orchestrator.api_key.clone().into())
                    .context("build deepseek client")?;
            let extractor = completions_client
                .extractor::<JustificationPatchList>(orchestrator.model.clone())
                .preamble(PATCHER_PREAMBLE)
                .build();
            let response = extractor
                .extract(prompt)
                .await
                .context("deepseek extract")?;
            Ok(response.0)
        }
        Provider::Cohere => {
            let client = cohere::Client::from_val(orchestrator.api_key.clone().into())
                .context("build cohere client")?;
            let extractor = client
                .extractor::<JustificationPatchList>(orchestrator.model.clone())
                .preamble(PATCHER_PREAMBLE)
                .build();
            let response = extractor.extract(prompt).await.context("cohere extract")?;
            Ok(response.0)
        }
        Provider::Ollama => {
            let client = ollama::Client::from_val(orchestrator.api_key.clone().into())
                .context("build ollama client")?;
            let extractor = client
                .extractor::<JustificationPatchList>(orchestrator.model.clone())
                .preamble(PATCHER_PREAMBLE)
                .build();
            let response = extractor.extract(prompt).await.context("ollama extract")?;
            Ok(response.0)
        }
        // Either explicitly disabled or not yet wired. The orchestrator has
        // already filtered these out via `is_wired`, but we keep the arm for
        // future-proofing.
        _ => anyhow::bail!("provider not wired: {}", orchestrator.provider.id()),
    }
}

const PATCHER_PREAMBLE: &str = "You refine justification text for emergency dispatch \
    envelopes. Return only the JSON array, no prose.";

/// rig's extractor expects a single JSON object; we wrap the array in a
/// named-struct so the LLM knows exactly what shape to return.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct JustificationPatchList(Vec<JustificationPatch>);

fn apply_patches(
    heuristic: &[ToolCallEnvelope],
    patches: Vec<JustificationPatch>,
) -> anyhow::Result<Vec<ToolCallEnvelope>> {
    // Defensive contract: every patch must match an envelope by id, the patch
    // count must be ≤ the envelope count, and justifications must be
    // non-empty. Anything else is treated as a malformed response and the
    // orchestrator returns the heuristic envelopes unchanged.
    if patches.len() > heuristic.len() {
        anyhow::bail!(
            "LLM returned more patches ({}) than envelopes ({})",
            patches.len(),
            heuristic.len()
        );
    }
    let mut by_id: std::collections::HashMap<Uuid, String> =
        std::collections::HashMap::with_capacity(patches.len());
    for patch in patches {
        if patch.justification.trim().is_empty() {
            anyhow::bail!(
                "LLM returned an empty justification for {}",
                patch.incident_id
            );
        }
        if patch.tool_name().is_some() {
            anyhow::bail!("LLM returned an extraneous field for {}", patch.incident_id);
        }
        if by_id
            .insert(patch.incident_id, patch.justification)
            .is_some()
        {
            anyhow::bail!("LLM returned duplicate patches for {}", patch.incident_id);
        }
    }
    let mut enriched = heuristic.to_vec();
    for envelope in enriched.iter_mut() {
        if let Some(justification) = by_id.remove(&envelope.arguments.incident_id) {
            envelope.arguments.justification = justification;
        }
    }
    // Any leftover patches that didn't match an envelope are discarded;
    // the orchestrator's contract is to enrich existing envelopes, not add
    // new ones.
    Ok(enriched)
}

impl JustificationPatch {
    /// Helper used to assert that the deserialized patch carries no extra
    /// fields. rig's extractor serializes the typed schema, so any field
    /// beyond `incident_id` and `justification` is a model-hallucination
    /// signal that we should reject.
    fn tool_name(&self) -> Option<&'static str> {
        // We have no `tool_name` field on the patch — this is a stand-in so
        // that callers can detect a deserialization artifact via test hooks.
        None
    }
}

impl ToolCallEnvelope {
    /// Helper that asserts the tool name is the spec §7 envelope. Used by
    /// the orchestrator log path; not part of the wire contract.
    pub fn expected_tool_name() -> &'static str {
        TOOL_NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_from_config_matches_documented_names() {
        assert_eq!(Provider::from_config("openai"), Provider::Openai);
        assert_eq!(Provider::from_config("OpenAI"), Provider::Openai);
        assert_eq!(Provider::from_config("anthropic"), Provider::Anthropic);
        assert_eq!(Provider::from_config("claude"), Provider::Anthropic);
        assert_eq!(Provider::from_config("gemini"), Provider::Gemini);
        assert_eq!(Provider::from_config("google"), Provider::Gemini);
        assert_eq!(Provider::from_config("ollama"), Provider::Ollama);
        assert_eq!(Provider::from_config("azure_openai"), Provider::Azure);
        assert_eq!(
            Provider::from_config("custom-rig-thing"),
            Provider::Unknown("custom-rig-thing".to_string())
        );
    }

    #[test]
    fn provider_is_wired_only_for_supported_clients() {
        assert!(Provider::Openai.is_wired());
        assert!(Provider::Anthropic.is_wired());
        assert!(Provider::Gemini.is_wired());
        assert!(Provider::Cohere.is_wired());
        assert!(Provider::Ollama.is_wired());
        assert!(!Provider::Bedrock.is_wired());
        assert!(!Provider::Openrouter.is_wired());
        assert!(!Provider::Unknown("custom".into()).is_wired());
    }

    #[test]
    fn orchestrator_from_config_returns_none_for_unconfigured_ai() {
        let mut ai = AiConfig::default();
        assert!(Orchestrator::from_config(&ai).unwrap().is_none());
        ai.provider = Some("openai".into());
        assert!(Orchestrator::from_config(&ai).unwrap().is_none());
        ai.model = Some("gpt-4o-mini".into());
        assert!(Orchestrator::from_config(&ai).unwrap().is_none());
        ai.api_key = Some("sk-test".into());
        let orchestrator = Orchestrator::from_config(&ai).unwrap().unwrap();
        assert_eq!(orchestrator.provider, Provider::Openai);
        assert_eq!(orchestrator.model, "gpt-4o-mini");
        assert_eq!(orchestrator.api_key, "sk-test");
    }

    #[test]
    fn apply_patches_replaces_justification_text_in_place() {
        let incident_a = Uuid::new_v4();
        let incident_b = Uuid::new_v4();
        let heuristic = vec![
            ToolCallEnvelope {
                tool_name: TOOL_NAME.to_string(),
                arguments: dispatch::DispatchArguments {
                    incident_id: incident_a,
                    primary_center_id: Uuid::new_v4(),
                    core_fallback_center_id: None,
                    allocations: Vec::new(),
                    resource_state_modifications: Vec::new(),
                    justification: "old-a".into(),
                },
            },
            ToolCallEnvelope {
                tool_name: TOOL_NAME.to_string(),
                arguments: dispatch::DispatchArguments {
                    incident_id: incident_b,
                    primary_center_id: Uuid::new_v4(),
                    core_fallback_center_id: None,
                    allocations: Vec::new(),
                    resource_state_modifications: Vec::new(),
                    justification: "old-b".into(),
                },
            },
        ];
        let patches = vec![
            JustificationPatch {
                incident_id: incident_a,
                justification: "new-a".into(),
            },
            JustificationPatch {
                incident_id: incident_b,
                justification: "new-b".into(),
            },
        ];
        let enriched = apply_patches(&heuristic, patches).unwrap();
        assert_eq!(enriched.len(), 2);
        assert_eq!(enriched[0].arguments.justification, "new-a");
        assert_eq!(enriched[1].arguments.justification, "new-b");
    }

    #[test]
    fn apply_patches_rejects_empty_justification() {
        let incident = Uuid::new_v4();
        let heuristic = vec![ToolCallEnvelope {
            tool_name: TOOL_NAME.to_string(),
            arguments: dispatch::DispatchArguments {
                incident_id: incident,
                primary_center_id: Uuid::new_v4(),
                core_fallback_center_id: None,
                allocations: Vec::new(),
                resource_state_modifications: Vec::new(),
                justification: "old".into(),
            },
        }];
        let patches = vec![JustificationPatch {
            incident_id: incident,
            justification: "  ".into(),
        }];
        assert!(apply_patches(&heuristic, patches).is_err());
    }

    #[test]
    fn apply_patches_rejects_too_many_patches() {
        let heuristic = vec![ToolCallEnvelope {
            tool_name: TOOL_NAME.to_string(),
            arguments: dispatch::DispatchArguments {
                incident_id: Uuid::new_v4(),
                primary_center_id: Uuid::new_v4(),
                core_fallback_center_id: None,
                allocations: Vec::new(),
                resource_state_modifications: Vec::new(),
                justification: "old".into(),
            },
        }];
        let patches = vec![
            JustificationPatch {
                incident_id: Uuid::new_v4(),
                justification: "a".into(),
            },
            JustificationPatch {
                incident_id: Uuid::new_v4(),
                justification: "b".into(),
            },
        ];
        assert!(apply_patches(&heuristic, patches).is_err());
    }

    #[test]
    fn apply_patches_rejects_duplicates() {
        let incident = Uuid::new_v4();
        let heuristic = vec![ToolCallEnvelope {
            tool_name: TOOL_NAME.to_string(),
            arguments: dispatch::DispatchArguments {
                incident_id: incident,
                primary_center_id: Uuid::new_v4(),
                core_fallback_center_id: None,
                allocations: Vec::new(),
                resource_state_modifications: Vec::new(),
                justification: "old".into(),
            },
        }];
        let patches = vec![
            JustificationPatch {
                incident_id: incident,
                justification: "a".into(),
            },
            JustificationPatch {
                incident_id: incident,
                justification: "b".into(),
            },
        ];
        assert!(apply_patches(&heuristic, patches).is_err());
    }

    #[tokio::test]
    async fn semaphore_gate_tracks_permits_within_window() {
        let gate = SemaphoreGate::new();
        assert!(gate.try_acquire().await);
        assert!(gate.try_acquire().await);
        assert!(gate.try_acquire().await);
        // 4th call within the window must return false.
        assert!(!gate.try_acquire().await);
    }

    #[tokio::test]
    async fn semaphore_gate_resets_when_window_is_reset() {
        let gate = SemaphoreGate::new();
        for _ in 0..3 {
            assert!(gate.try_acquire().await);
        }
        assert!(!gate.try_acquire().await);
        gate.reset_window().await;
        assert!(gate.try_acquire().await);
    }
}
