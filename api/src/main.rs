use std::sync::Arc;

use anyhow::Context;
use civic_sync_api::{app, config::Config};
use sqlx::postgres::PgPoolOptions;
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

    let router = app::router_with_config(database, config.clone());
    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    let address = listener.local_addr()?;
    tracing::info!(%address, "Civic-Sync API listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
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
