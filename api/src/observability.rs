//! Prometheus metrics + counters (data.md §4 observability layer).
//!
//! data.md §4 shows "Prometheus + Grafana + OpenTelemetry (Queue
//! Tracking & Tracing)" in the architecture diagram. This module
//! provides the *Prometheus* half of that pair: a `metrics::Recorder`
//! that the rest of the code can call into via the global
//! `metrics::counter!`, `metrics::histogram!`, `metrics::gauge!`
//! macros, plus an Axum handler that returns the rendered text.
//!
//! The exporter is in-process — no separate Prometheus server needed.
//! Tests can pull the in-memory snapshot via [`render`] to assert
//! specific counters incremented.

use std::sync::OnceLock;
use std::time::Instant;

use axum::{
    extract::Request as AxumRequest,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use metrics::Unit;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the global Prometheus recorder. Idempotent: calling more than
/// once returns the same handle so tests can call this freely.
pub fn install() -> PrometheusHandle {
    if let Some(handle) = HANDLE.get() {
        return handle.clone();
    }
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("install Prometheus recorder");
    let _ = HANDLE.set(handle.clone());
    register_descriptions();
    handle
}

fn register_descriptions() {
    metrics::describe_counter!(
        "civic_sync_http_requests_total",
        "Total HTTP requests handled by the API"
    );
    metrics::describe_histogram!(
        "civic_sync_http_request_duration_seconds",
        Unit::Seconds,
        "Request latency in seconds"
    );
    metrics::describe_counter!(
        "civic_sync_dispatch_total",
        "Total dispatch calls broken out by fallback reason"
    );
    metrics::describe_counter!(
        "civic_sync_dispatch_envelopes",
        "Total envelopes produced by the dispatch pipeline"
    );
    metrics::describe_counter!(
        "civic_sync_semaphore_acquire_total",
        "Orchestrator §5B semaphore acquire attempts"
    );
    metrics::describe_counter!("civic_sync_flush_ticks_total", "60s flush driver ticks");
    metrics::describe_counter!(
        "civic_sync_flush_marks_total",
        "FlushMarks enqueued by mutating endpoints"
    );
    metrics::describe_counter!(
        "civic_sync_simulation_generated_total",
        "Synthetic incidents emitted by the §5D simulator"
    );
    metrics::describe_gauge!(
        "civic_sync_simulation_paused",
        "Whether the simulator is currently paused (1 = paused)"
    );
    // --- AI triage observability (added in the 1-hour hardening pass) ---
    metrics::describe_counter!(
        "civic_sync_ai_triage_total",
        "AI triage endpoint calls broken out by outcome (success|fallback|throttled|cache_hit|no_orchestrator)"
    );
    metrics::describe_histogram!(
        "civic_sync_ai_triage_latency_ms",
        Unit::Milliseconds,
        "Latency of successful AI triage calls (excludes cache hits and heuristic fallbacks)"
    );
    metrics::describe_counter!(
        "civic_sync_redis_cache_operations_total",
        "Redis cache operations broken out by op (get|set) and result (hit|miss|error)"
    );
}

/// Tower middleware: count every request and observe its duration.
pub async fn http_metrics_layer(req: AxumRequest, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_string();
    // Use the matched route template if the router populated it,
    // otherwise fall back to the raw path (high-cardinality risk —
    // we collapse `unknown` so it doesn't blow up Prometheus).
    let route = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let elapsed = started.elapsed().as_secs_f64();
    record_http(&method, &route, status, elapsed);
    response
}

/// Convenience: emit one HTTP request + observation.
pub fn record_http(method: &str, route: &str, status: u16, duration_secs: f64) {
    metrics::counter!(
        "civic_sync_http_requests_total",
        "method" => method.to_string(),
        "route" => route.to_string(),
        "status" => status.to_string(),
    )
    .increment(1);
    metrics::histogram!(
        "civic_sync_http_request_duration_seconds",
        "method" => method.to_string(),
        "route" => route.to_string(),
    )
    .record(duration_secs);
}

/// Convenience: emit one dispatch observation. `fallback` should be one
/// of `none`, `semaphore`, `llm_error`.
pub fn record_dispatch(fallback: &'static str, envelopes: usize) {
    metrics::counter!(
        "civic_sync_dispatch_total",
        "fallback" => fallback,
    )
    .increment(1);
    metrics::counter!("civic_sync_dispatch_envelopes").increment(envelopes as u64);
}

/// Convenience: emit one semaphore acquire observation.
pub fn record_semaphore(result: &'static str) {
    metrics::counter!(
        "civic_sync_semaphore_acquire_total",
        "result" => result,
    )
    .increment(1);
}

/// Convenience: emit one flush tick observation.
pub fn record_flush_tick(kind: &'static str) {
    metrics::counter!(
        "civic_sync_flush_ticks_total",
        "kind" => kind,
    )
    .increment(1);
}

/// Convenience: emit one flush-mark enqueue observation.
pub fn record_flush_mark(kind: &'static str) {
    metrics::counter!(
        "civic_sync_flush_marks_total",
        "kind" => kind,
    )
    .increment(1);
}

/// Convenience: emit one simulator observation.
pub fn record_simulation_generated() {
    metrics::counter!("civic_sync_simulation_generated_total").increment(1);
}

pub fn set_simulation_paused(paused: bool) {
    metrics::gauge!("civic_sync_simulation_paused").set(if paused { 1.0 } else { 0.0 });
}

/// Convenience: emit one AI triage observation. `outcome` is one of
/// `success | fallback | throttled | cache_hit | no_orchestrator`.
/// `latency_ms` is only recorded on the success path; it is ignored for
/// every other outcome.
pub fn record_ai_triage(outcome: &'static str, latency_ms: u64) {
    metrics::counter!(
        "civic_sync_ai_triage_total",
        "outcome" => outcome,
    )
    .increment(1);
    if outcome == "success" && latency_ms > 0 {
        metrics::histogram!("civic_sync_ai_triage_latency_ms").record(latency_ms as f64);
    }
}

/// Convenience: emit one Redis cache operation observation. `op` is
/// `get | set`, `result` is `hit | miss | error`.
pub fn record_cache_op(op: &'static str, result: &'static str) {
    metrics::counter!(
        "civic_sync_redis_cache_operations_total",
        "op" => op,
        "result" => result,
    )
    .increment(1);
}

/// Render the current metrics snapshot in Prometheus text format.
pub fn render() -> Option<String> {
    HANDLE.get().map(|handle| handle.render())
}

/// `GET /metrics` handler. Returns 503 if the recorder was never
/// installed (test default).
pub async fn handler() -> Response {
    match render() {
        Some(body) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4")],
            body,
        )
            .into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("content-type", "text/plain; charset=utf-8")],
            "metrics recorder not installed".to_string(),
        )
            .into_response(),
    }
}

/// Silence unused-type warnings on the metrics facade. The Prometheus
/// exporter registers itself via [`install`]; the global macros
/// (`metrics::counter!`, `metrics::histogram!`, etc.) handle
/// registration implicitly.
#[allow(dead_code)]
const _ENSURE_METRICS_MACROS_HAVE_A_RECORDER: fn() = || {
    metrics::counter!("noop").increment(0);
};

// --- OpenTelemetry tracing (data.md §4) ----------------------------------
//
// Layered on top of `tracing`: a `tracing-opentelemetry` layer pairs every
// `tracing::span` with an OpenTelemetry span, and `opentelemetry-stdout`
// renders the finished spans to stdout. No external collector (Tempo/Jaeger)
// required — every span shows up next to the existing tracing logs.
//
// We never block install: if the SDK fails to initialise (e.g. resource
// limits) we just log a warning and continue with the plain `tracing_subscriber`
// registry so the API still boots.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Install tracing + stdout OpenTelemetry. Idempotent: a second call is a
/// no-op so tests can call it freely without crashing on the global
/// subscriber registration.
pub fn install_tracing() {
    static INSTALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if INSTALLED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let resource = Resource::builder()
        .with_attributes(vec![
            opentelemetry::KeyValue::new("service.name", "civic-sync-api"),
            opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build();
    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .build();
    let tracer = provider.tracer("civic-sync-api");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
        .init();
}
