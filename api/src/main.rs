use std::sync::Arc;

use anyhow::Context;
use civic_sync_api::{app, config::Config, state::AppState};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::watch;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let config = Arc::new(Config::from_env()?);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    log_config_summary(&config);

    let database = if let Some(url) = &config.database_url {
        let pool = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .connect(url)
            .await
            .context("connect to PostgreSQL")?;
        sqlx::migrate!()
            .run(&pool)
            .await
            .context("run migrations")?;
        Some(pool)
    } else {
        tracing::warn!("DATABASE_URL is not configured; using in-memory repositories");
        None
    };

    let state = AppState::with_config(database, config.clone());
    let (triggers_tx, triggers_rx) = civic_sync_api::features::triggers::channel();
    let (flush_tx, flush_rx, flush_notice_tx) = civic_sync_api::features::flush::channels();
    // Replace the dummy senders AppState wired up with the actual ones so
    // that resource status hooks and incident-create hooks can notify the
    // real driver tasks.
    let mut state = state;
    state.triggers_tx = triggers_tx;
    state.flush_tx = flush_tx.clone();
    state.flush_notice_tx = flush_notice_tx.clone();
    // §5D background simulator. Disabled by default unless SIMULATION_AUTOSTART=true.
    let autostart = std::env::var("SIMULATION_AUTOSTART")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        });
    let simulation = civic_sync_api::features::simulation::SimulationState::new(autostart);
    let state = state.with_simulation(simulation.clone());
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let triggers_handle = {
        let state = state.clone();
        let sd = shutdown_rx.clone();
        tokio::spawn(async move {
            civic_sync_api::features::triggers::run(state, triggers_rx, sd).await;
        })
    };
    let flush_handle = {
        let state = state.clone();
        let sd = shutdown_rx.clone();
        tokio::spawn(async move {
            civic_sync_api::features::flush::run(state, flush_rx, flush_notice_tx, sd).await;
        })
    };
    let simulation_handle = {
        let state = state.clone();
        let sim = simulation.clone();
        let sd = shutdown_rx.clone();
        tokio::spawn(async move {
            civic_sync_api::features::simulation::run(state, sim, sd).await;
        })
    };

    let router = app::router_with_state(state);
    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    let address = listener.local_addr()?;
    tracing::info!(%address, "Civic-Sync API listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    let _ = shutdown_tx.send(true);
    let _ = triggers_handle.await;
    let _ = flush_handle.await;
    let _ = simulation_handle.await;
    Ok(())
}

fn log_config_summary(config: &Config) {
    tracing::info!(
        database = config.database_url.is_some(),
        redis = config.redis_url.is_some(),
        s3_bucket = config.s3.bucket.as_deref().unwrap_or("(unset)"),
        ai_provider = config.ai.provider.as_deref().unwrap_or("(unset)"),
        allowed_origins = config.allowed_origins.len(),
        "config loaded"
    );
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
