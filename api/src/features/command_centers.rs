use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// The 8 divisional command hubs. Dhaka is the core center that supplies
/// fallback resources when a divisional hub faces a regional deficit.
const DHAKA_HUBS: &[(&str, bool, f64, f64)] = &[
    ("Dhaka", true, 23.8103, 90.4125),
    ("Chittagong", false, 22.3569, 91.7832),
    ("Rajshahi", false, 24.3745, 88.6042),
    ("Khulna", false, 22.8456, 89.5403),
    ("Barisal", false, 22.7010, 90.3535),
    ("Sylhet", false, 24.8949, 91.8687),
    ("Rangpur", false, 25.7466, 89.2517),
    ("Mymensingh", false, 24.7471, 90.4203),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCenter {
    pub id: Uuid,
    pub name: String,
    pub is_core_center: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

/// Query-parameter filters for the single `GET /command-centers` route.
/// Filtering on real data fields preserves data identity instead of
/// introducing path-based sub-routes.
#[derive(Debug, Default, Deserialize)]
pub struct ListCommandCentersQuery {
    pub name: Option<String>,
    pub is_core_center: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListCommandCentersQuery>,
) -> Result<Json<Vec<CommandCenter>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool, &query).await?,
        None => state
            .command_centers
            .read()
            .await
            .values()
            .filter(|&center| query.matches(center))
            .skip(query.offset.unwrap_or(0) as usize)
            .take(query.limit.unwrap_or(i64::MAX) as usize)
            .cloned()
            .collect::<Vec<_>>(),
    };
    Ok(Json(items))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CommandCenter>, ApiError> {
    let center = match &state.database {
        Some(pool) => fetch_postgres(pool, id).await?,
        None => state
            .command_centers
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(ApiError::NotFound)?,
    };
    Ok(Json(center))
}

impl ListCommandCentersQuery {
    fn matches(&self, center: &CommandCenter) -> bool {
        if let Some(name) = &self.name
            && !center.name.eq_ignore_ascii_case(name)
        {
            return false;
        }
        if let Some(core) = self.is_core_center
            && center.is_core_center != core
        {
            return false;
        }
        true
    }

    fn validate(&self) -> Result<(), ApiError> {
        if self.limit.is_some_and(|value| value <= 0) {
            return Err(ApiError::Validation("limit must be positive".into()));
        }
        if self.offset.is_some_and(|value| value < 0) {
            return Err(ApiError::Validation("offset must be non-negative".into()));
        }
        Ok(())
    }
}

pub(crate) async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &ListCommandCentersQuery,
) -> Result<Vec<CommandCenter>, ApiError> {
    query.validate()?;
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"SELECT id, name, is_core_center,
                  ST_Y(location::geometry) AS latitude,
                  ST_X(location::geometry) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM command_centers"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(name) = &query.name {
        builder.push(" AND name = ");
        builder.push_bind(name);
    }
    if let Some(core) = query.is_core_center {
        builder.push(" AND is_core_center = ");
        builder.push_bind(core);
    }
    builder.push(" ORDER BY is_core_center DESC, name ASC");
    if let Some(limit) = query.limit {
        builder.push(" LIMIT ");
        builder.push_bind(limit);
    }
    if let Some(offset) = query.offset {
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }
    let rows = builder
        .build_query_as::<CommandCenterRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(CommandCenter::from).collect())
}

#[allow(dead_code)] // consumed by the dispatch engine in src/features/dispatch.rs
pub(crate) async fn list_all_postgres(pool: &sqlx::PgPool) -> Result<Vec<CommandCenter>, ApiError> {
    let rows = sqlx::query_as::<_, CommandCenterRow>(
        r#"SELECT id, name, is_core_center,
                  ST_Y(location::geometry) AS latitude,
                  ST_X(location::geometry) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM command_centers
           ORDER BY is_core_center DESC, name ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(CommandCenter::from).collect())
}

pub(crate) async fn fetch_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
) -> Result<CommandCenter, ApiError> {
    let row = sqlx::query_as::<_, CommandCenterRow>(
        r#"SELECT id, name, is_core_center,
                  ST_Y(location::geometry) AS latitude,
                  ST_X(location::geometry) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM command_centers WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(CommandCenter::from(row))
}

/// Seed the in-memory replica with the 8 divisional hubs so the feature works
/// without a database (mirrors the DB seed in `0002_command_centers.sql`).
pub(crate) fn seed_command_centers() -> Vec<CommandCenter> {
    let now = Utc::now();
    DHAKA_HUBS
        .iter()
        .map(|(name, is_core, latitude, longitude)| CommandCenter {
            id: deterministic_id(name),
            name: (*name).to_string(),
            is_core_center: *is_core,
            latitude: *latitude,
            longitude: *longitude,
            created_at: now,
            updated_at: now,
            server_synced_at: None,
        })
        .collect()
}

fn deterministic_id(name: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes())
}

#[derive(sqlx::FromRow)]
pub struct CommandCenterRow {
    pub id: Uuid,
    pub name: String,
    pub is_core_center: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<CommandCenterRow> for CommandCenter {
    fn from(row: CommandCenterRow) -> Self {
        CommandCenter {
            id: row.id,
            name: row.name,
            is_core_center: row.is_core_center,
            latitude: row.latitude,
            longitude: row.longitude,
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
