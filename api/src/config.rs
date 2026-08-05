use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: Option<String>,
    pub database_max_connections: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_address = std::env::var("PORT")
            .ok()
            .and_then(|raw| raw.parse::<u16>().ok())
            .map(|port| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port))
            .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080));

        let database_url = std::env::var("DATABASE_URL")
            .ok()
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty());

        let database_max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(10);

        Ok::<Self, anyhow::Error>(Self {
            bind_address,
            database_url,
            database_max_connections,
        })
        .context("load configuration")
    }
}
