use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    features::helper_teams::{HelperTeam, HelperTeamRow},
    state::AppState,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AllocationStatus {
    EnRoute,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperAllocation {
    pub id: Uuid,
    pub helper_team_id: Uuid,
    pub incident_id: Option<Uuid>,
    pub assistance_request_id: Option<Uuid>,
    pub members_deployed: u32,
    pub status: AllocationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateHelperAllocation {
    pub helper_team_id: Uuid,
    pub incident_id: Option<Uuid>,
    pub assistance_request_id: Option<Uuid>,
    pub members_deployed: u32,
}

impl CreateHelperAllocation {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.incident_id.is_none() && self.assistance_request_id.is_none() {
            return Err(ApiError::Validation(
                "at least one of incident_id or assistance_request_id is required".into(),
            ));
        }
        if self.members_deployed == 0 {
            return Err(ApiError::Validation(
                "members_deployed must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

/// Query-parameter filters for the single `GET /helper-allocations` route.
#[derive(Debug, Default, Deserialize)]
pub struct ListHelperAllocationsQuery {
    pub status: Option<AllocationStatus>,
    pub helper_team_id: Option<Uuid>,
    pub incident_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateHelperAllocation>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    match &state.database {
        Some(pool) => create_postgres(pool, &input).await,
        None => create_in_memory(&state, &input).await,
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListHelperAllocationsQuery>,
) -> Result<Json<Vec<HelperAllocation>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool, &query).await?,
        None => state
            .helper_allocations
            .read()
            .await
            .values()
            .filter(|&allocation| query.matches(allocation))
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
) -> Result<Json<HelperAllocation>, ApiError> {
    let allocation = match &state.database {
        Some(pool) => fetch_postgres(pool, id).await?,
        None => state
            .helper_allocations
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(ApiError::NotFound)?,
    };
    Ok(Json(allocation))
}

impl ListHelperAllocationsQuery {
    fn matches(&self, allocation: &HelperAllocation) -> bool {
        if let Some(status) = self.status
            && allocation.status != status
        {
            return false;
        }
        if let Some(helper_team_id) = self.helper_team_id
            && allocation.helper_team_id != helper_team_id
        {
            return false;
        }
        if let Some(incident_id) = self.incident_id
            && allocation.incident_id != Some(incident_id)
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

fn capacity_conflict(team: &HelperTeam, requested: u32) -> ApiError {
    ApiError::Conflict(format!(
        "team '{}' has capacity {} but {} members were requested",
        team.team_name,
        team.available_capacity(),
        requested
    ))
}

async fn create_in_memory(
    state: &AppState,
    input: &CreateHelperAllocation,
) -> Result<(StatusCode, Json<HelperAllocation>), ApiError> {
    let now = Utc::now();
    let allocation = {
        let mut teams = state.helper_teams.write().await;
        let team = teams
            .get_mut(&input.helper_team_id)
            .ok_or(ApiError::NotFound)?;
        if input.members_deployed > team.available_capacity() {
            return Err(capacity_conflict(team, input.members_deployed));
        }
        team.assigned_members += input.members_deployed;
        team.updated_at = now;
        HelperAllocation {
            id: Uuid::new_v4(),
            helper_team_id: input.helper_team_id,
            incident_id: input.incident_id,
            assistance_request_id: input.assistance_request_id,
            members_deployed: input.members_deployed,
            status: AllocationStatus::EnRoute,
            created_at: now,
            updated_at: now,
            server_synced_at: None,
        }
    };
    state
        .helper_allocations
        .write()
        .await
        .insert(allocation.id, allocation.clone());
    Ok((StatusCode::CREATED, Json(allocation)))
}

/// Insert a helper_allocations row inside an existing transaction. Used by
/// both the POST endpoint and the dispatch engine so the entire batch shares
/// one tx (data.md §2 — conflict prevention).
#[allow(dead_code)] // consumed by src/features/dispatch.rs
pub(crate) async fn insert_postgres_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &CreateHelperAllocation,
    status: AllocationStatus,
    now: DateTime<Utc>,
) -> Result<HelperAllocation, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO helper_allocations (
            id, helper_team_id, incident_id, assistance_request_id,
            members_deployed, status, created_at, updated_at, server_synced_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(id)
    .bind(input.helper_team_id)
    .bind(input.incident_id)
    .bind(input.assistance_request_id)
    .bind(input.members_deployed as i32)
    .bind(serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(now)
    .bind(now)
    .bind(Option::<DateTime<Utc>>::None)
    .execute(&mut **tx)
    .await?;
    Ok(HelperAllocation {
        id,
        helper_team_id: input.helper_team_id,
        incident_id: input.incident_id,
        assistance_request_id: input.assistance_request_id,
        members_deployed: input.members_deployed,
        status,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    })
}

async fn create_postgres(
    pool: &sqlx::PgPool,
    input: &CreateHelperAllocation,
) -> Result<(StatusCode, Json<HelperAllocation>), ApiError> {
    let mut tx = pool.begin().await?;
    let team_row = sqlx::query_as::<_, HelperTeamRow>(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams WHERE id = $1 FOR UPDATE"#,
    )
    .bind(input.helper_team_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;
    let team = HelperTeam::from(team_row);
    if input.members_deployed > team.available_capacity() {
        return Err(capacity_conflict(&team, input.members_deployed));
    }

    let now = Utc::now();
    let allocation = insert_postgres_tx(&mut tx, input, AllocationStatus::EnRoute, now).await?;
    crate::features::helper_teams::bump_assigned_postgres_tx(
        &mut tx,
        input.helper_team_id,
        input.members_deployed,
        now,
    )
    .await?;
    tx.commit().await?;

    let stored = fetch_postgres(pool, allocation.id).await?;
    Ok((StatusCode::CREATED, Json(stored)))
}

pub(crate) async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &ListHelperAllocationsQuery,
) -> Result<Vec<HelperAllocation>, ApiError> {
    query.validate()?;
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"SELECT id, helper_team_id, incident_id, assistance_request_id,
                  members_deployed, status, created_at, updated_at, server_synced_at
           FROM helper_allocations"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(status) = query.status {
        builder.push(" AND status = ");
        builder.push_bind(
            serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?,
        );
    }
    if let Some(helper_team_id) = query.helper_team_id {
        builder.push(" AND helper_team_id = ");
        builder.push_bind(helper_team_id);
    }
    if let Some(incident_id) = query.incident_id {
        builder.push(" AND incident_id = ");
        builder.push_bind(incident_id);
    }
    builder.push(" ORDER BY created_at ASC");
    if let Some(limit) = query.limit {
        builder.push(" LIMIT ");
        builder.push_bind(limit);
    }
    if let Some(offset) = query.offset {
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }
    let rows = builder
        .build_query_as::<HelperAllocationRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(HelperAllocation::from).collect())
}

pub(crate) async fn fetch_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
) -> Result<HelperAllocation, ApiError> {
    let row = sqlx::query_as::<_, HelperAllocationRow>(
        r#"SELECT id, helper_team_id, incident_id, assistance_request_id,
                  members_deployed, status, created_at, updated_at, server_synced_at
           FROM helper_allocations WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(HelperAllocation::from(row))
}

#[derive(sqlx::FromRow)]
pub struct HelperAllocationRow {
    pub id: Uuid,
    pub helper_team_id: Uuid,
    pub incident_id: Option<Uuid>,
    pub assistance_request_id: Option<Uuid>,
    pub members_deployed: i32,
    pub status: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<HelperAllocationRow> for HelperAllocation {
    fn from(row: HelperAllocationRow) -> Self {
        HelperAllocation {
            id: row.id,
            helper_team_id: row.helper_team_id,
            incident_id: row.incident_id,
            assistance_request_id: row.assistance_request_id,
            members_deployed: row.members_deployed as u32,
            status: serde_json::from_value(row.status).unwrap_or(AllocationStatus::EnRoute),
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
