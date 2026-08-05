use axum::{
    Router,
    http::Method,
    routing::{get, patch, post},
};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    features::{
        dispatch, health,
        incidents::{self},
        resources::{self},
    },
    state::AppState,
};

pub fn router(database: Option<PgPool>) -> Router {
    let state = AppState::new(database);
    router_with_state(state)
}

pub fn router_with_state(state: AppState) -> Router {
    let api = Router::new()
        .route(
            "/v1/incidents",
            post(incidents::create).get(incidents::list),
        )
        .route("/v1/incidents/{id}", get(incidents::get_one))
        .route(
            "/v1/resources",
            post(resources::create).get(resources::list),
        )
        .route("/v1/resources/{id}/status", patch(resources::update_status))
        .route("/v1/dispatch/recommendations", post(dispatch::recommend));

    Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .nest("/api", api)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CorsLayer::new()
                        .allow_origin(tower_http::cors::Any)
                        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::OPTIONS])
                        .allow_headers(tower_http::cors::Any),
                ),
        )
        .with_state(state)
}
