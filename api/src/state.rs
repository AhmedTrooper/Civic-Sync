use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::features::{incidents::Incident, resources::Resource};

#[derive(Clone)]
pub struct AppState {
    pub database: Option<PgPool>,
    pub incidents: Arc<RwLock<HashMap<Uuid, Incident>>>,
    pub resources: Arc<RwLock<HashMap<Uuid, Resource>>>,
}

impl AppState {
    pub fn new(database: Option<PgPool>) -> Self {
        Self {
            database,
            incidents: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
