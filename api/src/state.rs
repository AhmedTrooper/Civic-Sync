use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::features::{
    assistance_requests::AssistanceRequest, command_centers::CommandCenter,
    helper_allocations::HelperAllocation, helper_teams::HelperTeam, incidents::Incident,
    resources::Resource,
};

#[derive(Clone)]
pub struct AppState {
    pub database: Option<PgPool>,
    pub incidents: Arc<RwLock<HashMap<Uuid, Incident>>>,
    pub resources: Arc<RwLock<HashMap<Uuid, Resource>>>,
    pub command_centers: Arc<RwLock<HashMap<Uuid, CommandCenter>>>,
    pub helper_teams: Arc<RwLock<HashMap<Uuid, HelperTeam>>>,
    pub assistance_requests: Arc<RwLock<HashMap<Uuid, AssistanceRequest>>>,
    pub helper_allocations: Arc<RwLock<HashMap<Uuid, HelperAllocation>>>,
}

impl AppState {
    pub fn new(database: Option<PgPool>) -> Self {
        let command_centers = crate::features::command_centers::seed_command_centers()
            .into_iter()
            .map(|center| (center.id, center))
            .collect::<HashMap<_, _>>();
        Self {
            database,
            incidents: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
            command_centers: Arc::new(RwLock::new(command_centers)),
            helper_teams: Arc::new(RwLock::new(HashMap::new())),
            assistance_requests: Arc::new(RwLock::new(HashMap::new())),
            helper_allocations: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
