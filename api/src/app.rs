use std::sync::Arc;

use axum::{
    Router,
    http::{HeaderValue, Method},
    routing::{get, patch, post},
};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    config::Config,
    features::{
        ai_triage, centers, dispatch, health,
        incidents::{self},
        resources::{self},
        simulation, sync,
    },
    observability,
    state::AppState,
};

pub fn router(database: Option<PgPool>) -> Router {
    router_with_config(database, Config::default_for_tests())
}

pub fn router_with_config(database: Option<PgPool>, config: Arc<Config>) -> Router {
    let state = AppState::with_config(database, config.clone());
    router_with_state_and_config(state, config.as_ref())
}

pub fn router_with_state(state: AppState) -> Router {
    let config = state.config.clone();
    router_with_state_and_config(state, config.as_ref())
}

pub fn router_with_state_and_config(state: AppState, config: &Config) -> Router {
    let api = Router::new()
        .route("/v1/centers", get(centers::list))
        .route(
            "/v1/centers/{id}",
            get(centers::get_one).delete(centers::delete),
        )
        // Compatibility alias: the admin dashboard addresses the hubs as
        // `command-centers` (their spec §1 name). Serving the same
        // handlers under both paths keeps the frontend contract intact
        // without forking any logic.
        .route("/v1/command-centers", get(centers::list))
        .route("/v1/command-centers/{id}", get(centers::get_one))
        .route(
            "/v1/incidents",
            post(incidents::create).get(incidents::list),
        )
        .route(
            "/v1/incidents/{id}",
            get(incidents::get_one)
                .patch(incidents::update)
                .delete(incidents::delete),
        )
        .route(
            "/v1/resources",
            post(resources::create).get(resources::list),
        )
        .route("/v1/resources/{id}/status", patch(resources::update_status))
        .route(
            "/v1/resources/{id}",
            get(resources::get_one).delete(resources::delete),
        )
        .route("/v1/dispatch/recommendations", post(dispatch::recommend))
        .route("/v1/dispatch/apply", post(dispatch::apply))
        .route("/v1/dispatch/smoke", post(dispatch::smoke))
        // AI severity re-classification + resource-kind predictor (data.md §7).
        // Read-only advisory: the deterministic engine still owns allocations.
        .route("/v1/ai/triage", post(ai_triage::triage))
        .route("/v1/admin/simulation", get(simulation::status))
        .route("/v1/admin/simulation/pause", post(simulation::pause))
        .route("/v1/admin/simulation/resume", post(simulation::resume))
        .route("/v1/admin/simulation/inject", post(simulation::inject))
        .route("/v1/sync/ws", get(sync::ws_handler))
        .layer(axum::middleware::from_fn(crate::auth::rbac_middleware));

    Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .route("/metrics", get(observability::handler))
        .nest("/api", api)
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(observability::http_metrics_layer))
                .layer(TraceLayer::new_for_http())
                .layer(build_cors_layer(config)),
        )
        .with_state(state)
}

fn build_cors_layer(config: &Config) -> CorsLayer {
    let methods = [
        Method::GET,
        Method::POST,
        Method::PATCH,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
    ];
    let allow_origin = if config.allowed_origins.is_empty() {
        AllowOrigin::any()
    } else {
        let values: Vec<HeaderValue> = config
            .allowed_origins
            .iter()
            .filter_map(|origin| HeaderValue::from_str(origin).ok())
            .collect();
        AllowOrigin::list(values)
    };
    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods(methods)
        .allow_headers(tower_http::cors::Any)
}
