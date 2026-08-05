use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::{
    config::Config,
    features::{
        assistance_requests::AssistanceRequest, command_centers::CommandCenter,
        helper_allocations::HelperAllocation, helper_teams::HelperTeam, incidents::Incident,
        orchestrator::Orchestrator, resources::Resource, triggers::TriggerEvent,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Option<PgPool>,
    pub incidents: Arc<RwLock<HashMap<Uuid, Incident>>>,
    pub resources: Arc<RwLock<HashMap<Uuid, Resource>>>,
    pub command_centers: Arc<RwLock<HashMap<Uuid, CommandCenter>>>,
    pub helper_teams: Arc<RwLock<HashMap<Uuid, HelperTeam>>>,
    pub assistance_requests: Arc<RwLock<HashMap<Uuid, AssistanceRequest>>>,
    pub helper_allocations: Arc<RwLock<HashMap<Uuid, HelperAllocation>>>,
    /// Optional AI orchestrator. `None` when `AiConfig::is_configured()` is
    /// false, i.e. the operator did not wire up an LLM. When `None`, the
    /// dispatch handler runs the deterministic heuristic on every call and
    /// the triggers driver operates in heuristic-only mode.
    pub orchestrator: Arc<Option<Orchestrator>>,
    /// Sender for delta-trigger events (data.md §5B). Resource status
    /// updates push `TriggerEvent::ResourceStuck` etc. here, and the
    /// triggers driver drains the receiver on its 30-second ticker.
    pub triggers_tx: mpsc::Sender<TriggerEvent>,
}

impl AppState {
    /// Convenience constructor used by tests and the default `router` path.
    /// Uses an empty `Config::default_for_tests()` so handlers can still read
    /// config without panicking.
    pub fn new(database: Option<PgPool>) -> Self {
        Self::with_config(database, Config::default_for_tests())
    }

    /// Real entry point used by `main.rs` after `Config::from_env()`.
    pub fn with_config(database: Option<PgPool>, config: Arc<Config>) -> Self {
        let command_centers = crate::features::command_centers::seed_command_centers()
            .into_iter()
            .map(|center| (center.id, center))
            .collect::<HashMap<_, _>>();
        let orchestrator = Arc::new(Orchestrator::from_config(&config.ai).ok().flatten());
        let (triggers_tx, _triggers_rx) = crate::features::triggers::channel();
        Self {
            config,
            database,
            incidents: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
            command_centers: Arc::new(RwLock::new(command_centers)),
            helper_teams: Arc::new(RwLock::new(HashMap::new())),
            assistance_requests: Arc::new(RwLock::new(HashMap::new())),
            helper_allocations: Arc::new(RwLock::new(HashMap::new())),
            orchestrator,
            triggers_tx,
        }
    }
}
