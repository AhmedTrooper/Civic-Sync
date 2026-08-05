use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: Option<String>,
    pub database_max_connections: u32,
    pub redis_url: Option<String>,
    pub s3: S3Config,
    pub ai: AiConfig,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct S3Config {
    pub endpoint: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AiConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_address = std::env::var("PORT")
            .ok()
            .and_then(|raw| raw.parse::<u16>().ok())
            .map(|port| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port))
            .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080));

        let database_url = env_opt("DATABASE_URL");
        let database_max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(10);

        let redis_url = env_opt("REDIS_URL");
        let s3 = S3Config {
            endpoint: env_opt("S3_ENDPOINT"),
            access_key: env_opt("S3_ACCESS_KEY"),
            secret_key: env_opt("S3_SECRET_KEY"),
            bucket: env_opt("S3_BUCKET"),
            region: env_opt("S3_REGION"),
        };
        let ai = AiConfig {
            provider: env_opt("AI_PROVIDER"),
            model: env_opt("AI_MODEL"),
            api_key: env_opt("AI_API_KEY"),
        };
        let allowed_origins = std::env::var("ALLOWED_ORIGINS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|segment| !segment.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok::<Self, anyhow::Error>(Self {
            bind_address,
            database_url,
            database_max_connections,
            redis_url,
            s3,
            ai,
            allowed_origins,
        })
        .context("load configuration")
    }

    /// Empty config used by tests and in-memory boot paths. Never reached for
    /// production: `main.rs` always calls `from_env` after `dotenvy::dotenv()`.
    pub fn default_for_tests() -> Arc<Self> {
        Arc::new(Self {
            bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080),
            database_url: None,
            database_max_connections: 10,
            redis_url: None,
            s3: S3Config::default(),
            ai: AiConfig::default(),
            allowed_origins: Vec::new(),
        })
    }
}

fn env_opt(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|value| !value.is_empty())
}
