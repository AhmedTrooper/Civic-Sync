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
        assistance_requests, command_centers, dispatch, health, helper_allocations, helper_teams,
        incidents::{self},
        resources::{self},
    },
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
        .route("/v1/command-centers", get(command_centers::list))
        .route("/v1/command-centers/{id}", get(command_centers::get_one))
        .route(
            "/v1/incidents",
            post(incidents::create).get(incidents::list),
        )
        .route(
            "/v1/incidents/{id}",
            get(incidents::get_one).patch(incidents::update),
        )
        .route(
            "/v1/resources",
            post(resources::create).get(resources::list),
        )
        .route("/v1/resources/{id}/status", patch(resources::update_status))
        .route(
            "/v1/helper-teams",
            post(helper_teams::create).get(helper_teams::list),
        )
        .route(
            "/v1/helper-teams/{id}",
            get(helper_teams::get_one).patch(helper_teams::update),
        )
        .route(
            "/v1/assistance-requests",
            post(assistance_requests::create).get(assistance_requests::list),
        )
        .route(
            "/v1/assistance-requests/{id}",
            get(assistance_requests::get_one).patch(assistance_requests::update_status),
        )
        .route(
            "/v1/helper-allocations",
            post(helper_allocations::create).get(helper_allocations::list),
        )
        .route(
            "/v1/helper-allocations/{id}",
            get(helper_allocations::get_one),
        )
        .route("/v1/dispatch/recommendations", post(dispatch::recommend));

    Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .nest("/api", api)
        .layer(
            ServiceBuilder::new()
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
