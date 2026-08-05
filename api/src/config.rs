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
    /// Provider name (openai, bedrock, cohere, gemini, anthropic, ollama, ...).
    /// Any rig-core 0.39 supported value is acceptable — the orchestrator
    /// treats the string opaquely so swapping providers is config-only.
    pub provider: Option<String>,
    /// Model identifier as the upstream provider expects it.
    pub model: Option<String>,
    /// API key for the configured provider.
    pub api_key: Option<String>,
}

impl AiConfig {
    /// True when provider + model + api_key are all populated. The orchestrator
    /// uses this to decide between a real LLM call and the heuristic fallback
    /// (data.md §5B / §7).
    pub fn is_configured(&self) -> bool {
        self.provider.is_some() && self.model.is_some() && self.api_key.is_some()
    }
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
        validate_ai_config(&ai)?;
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

/// All-or-nothing invariant for the AI block. If any of provider/model/api_key
/// is set, all three must be set — otherwise startup fails fast. This prevents
/// the orchestrator from silently falling back to the heuristic when the user
/// thought they had wired up an LLM.
fn validate_ai_config(ai: &AiConfig) -> anyhow::Result<()> {
    let set = [
        ("AI_PROVIDER", ai.provider.as_deref()),
        ("AI_MODEL", ai.model.as_deref()),
        ("AI_API_KEY", ai.api_key.as_deref()),
    ];
    let present: Vec<&str> = set
        .iter()
        .filter_map(|(name, value)| value.is_some().then_some(*name))
        .collect();
    if present.is_empty() {
        return Ok(());
    }
    if present.len() != set.len() {
        let missing: Vec<&str> = set
            .iter()
            .filter_map(|(name, value)| value.is_none().then_some(*name))
            .collect();
        anyhow::bail!(
            "AI configuration is partial: {} set, {} missing. Either set all three of AI_PROVIDER, AI_MODEL, AI_API_KEY or leave all three unset to run on the heuristic fallback.",
            present.join(", "),
            missing.join(", ")
        );
    }
    Ok(())
}

fn env_opt(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_config_is_configured_only_when_all_three_present() {
        let mut ai = AiConfig::default();
        assert!(!ai.is_configured());
        ai.provider = Some("openai".into());
        assert!(!ai.is_configured());
        ai.model = Some("gpt-4o-mini".into());
        assert!(!ai.is_configured());
        ai.api_key = Some("sk-test".into());
        assert!(ai.is_configured());
    }

    #[test]
    fn ai_validator_rejects_partial_configuration() {
        let ai = AiConfig {
            provider: Some("openai".into()),
            model: Some("gpt-4o-mini".into()),
            api_key: None,
        };
        let err = validate_ai_config(&ai).unwrap_err().to_string();
        assert!(err.contains("AI_API_KEY"));
        assert!(err.contains("AI_PROVIDER"));
    }

    #[test]
    fn ai_validator_accepts_all_set_and_all_unset() {
        let all_set = AiConfig {
            provider: Some("ollama".into()),
            model: Some("llama3".into()),
            api_key: Some("ignored".into()),
        };
        assert!(validate_ai_config(&all_set).is_ok());
        assert!(validate_ai_config(&AiConfig::default()).is_ok());
    }
}
