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
    features::{
        flush::{FlushKind, FlushMark},
        helper_teams::{HelperTeam, HelperTeamRow},
    },
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

#[derive(Debug, Deserialize)]
pub struct UpdateHelperAllocation {
    pub status: Option<AllocationStatus>,
    pub members_deployed: Option<u32>,
}

impl UpdateHelperAllocation {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.status.is_none() && self.members_deployed.is_none() {
            return Err(ApiError::Validation(
                "at least one of status or members_deployed must be provided".into(),
            ));
        }
        if self.members_deployed == Some(0) {
            return Err(ApiError::Validation(
                "members_deployed must be greater than 0".into(),
            ));
        }
        Ok(())
    }
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
    let result = match &state.database {
        Some(pool) => create_postgres(pool, &input).await,
        None => create_in_memory(&state, &input).await,
    }?;
    state.enqueue_flush(FlushMark::new(FlushKind::HelperAllocation, result.1.id, 0));
    Ok(result)
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

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateHelperAllocation>,
) -> Result<Json<HelperAllocation>, ApiError> {
    input.validate()?;
    let updated = match &state.database {
        Some(pool) => update_postgres(pool, id, &input).await?,
        None => update_in_memory(&state, id, &input).await?,
    };
    state.enqueue_flush(FlushMark::new(FlushKind::HelperAllocation, updated.id, 0));
    Ok(Json(updated))
}

/// DELETE /api/v1/helper-allocations/{id}. Deleting an active allocation also
/// restores its deployed members to the helper team's available capacity.
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    match &state.database {
        Some(pool) => delete_postgres(pool, id).await?,
        None => delete_in_memory(&state, id).await?,
    }
    state.enqueue_flush(FlushMark::new(FlushKind::HelperAllocation, id, 0));
    Ok(StatusCode::NO_CONTENT)
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

fn is_terminal_status(status: AllocationStatus) -> bool {
    matches!(
        status,
        AllocationStatus::Completed | AllocationStatus::Cancelled
    )
}

async fn update_in_memory(
    state: &AppState,
    id: Uuid,
    input: &UpdateHelperAllocation,
) -> Result<HelperAllocation, ApiError> {
    let now = Utc::now();
    // Lock teams BEFORE allocations to match `create_in_memory` order; this
    // prevents AB/BA deadlocks between allocation create and update flows.
    let mut teams = state.helper_teams.write().await;
    let mut allocations = state.helper_allocations.write().await;
    let allocation = allocations.get(&id).cloned().ok_or(ApiError::NotFound)?;
    if is_terminal_status(allocation.status) {
        return Err(ApiError::Conflict(
            "allocation is in a terminal state and cannot be updated".into(),
        ));
    }
    let team = teams
        .get_mut(&allocation.helper_team_id)
        .expect("FK integrity: allocation points at existing team");
    // Release the old bump before applying the new one so the available-capacity
    // check operates against the correct counter.
    team.assigned_members = team
        .assigned_members
        .saturating_sub(allocation.members_deployed);
    let next_members = input
        .members_deployed
        .unwrap_or(allocation.members_deployed);
    if next_members > team.available_capacity() {
        return Err(capacity_conflict(team, next_members));
    }
    team.assigned_members += next_members;

    let next_status = input.status.unwrap_or(allocation.status);
    let now_terminal = is_terminal_status(next_status);
    if now_terminal {
        // Status became terminal — restore the just-applied bump.
        team.assigned_members = team.assigned_members.saturating_sub(next_members);
    }
    team.updated_at = now;

    let mut updated = allocation.clone();
    updated.members_deployed = next_members;
    updated.status = next_status;
    updated.updated_at = now;
    allocations.insert(id, updated.clone());
    Ok(updated)
}

async fn delete_in_memory(state: &AppState, id: Uuid) -> Result<(), ApiError> {
    // Lock teams BEFORE allocations to match create/update order.
    let mut teams = state.helper_teams.write().await;
    let mut allocations = state.helper_allocations.write().await;
    let allocation = allocations.remove(&id).ok_or(ApiError::NotFound)?;
    if let Some(team) = teams.get_mut(&allocation.helper_team_id) {
        // Restore the just-released bump; saturating_sub guards against drift.
        team.assigned_members = team
            .assigned_members
            .saturating_sub(allocation.members_deployed);
        team.updated_at = Utc::now();
    }
    Ok(())
}

async fn delete_postgres(pool: &sqlx::PgPool, id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM helper_allocations WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
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

async fn update_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
    input: &UpdateHelperAllocation,
) -> Result<HelperAllocation, ApiError> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, HelperAllocationRow>(
        r#"SELECT id, helper_team_id, incident_id, assistance_request_id,
                  members_deployed, status, created_at, updated_at, server_synced_at
           FROM helper_allocations WHERE id = $1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;
    let prev_status: AllocationStatus =
        serde_json::from_str(&existing.status)
            .or_else(|_| serde_json::from_str(&format!("\"{}\"", existing.status)))
            .unwrap_or(AllocationStatus::EnRoute);
    if is_terminal_status(prev_status) {
        return Err(ApiError::Conflict(
            "allocation is in a terminal state and cannot be updated".into(),
        ));
    }

    let next_members = input
        .members_deployed
        .unwrap_or(existing.members_deployed as u32);
    let next_status = input.status.unwrap_or(prev_status);

    // Lock the team row so the assigned_members counter is updated atomically.
    let team_row = sqlx::query_as::<_, HelperTeamRow>(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams WHERE id = $1 FOR UPDATE"#,
    )
    .bind(existing.helper_team_id)
    .fetch_one(&mut *tx)
    .await?;
    let team = HelperTeam::from(team_row);
    // Release the existing bump, then re-apply the new value.
    let after_release = team
        .assigned_members
        .saturating_sub(existing.members_deployed as u32);
    let available = team.total_members.saturating_sub(after_release);
    if next_members > available {
        return Err(ApiError::Conflict(format!(
            "team '{}' has capacity {available} but {next_members} members were requested",
            team.team_name
        )));
    }
    let new_assigned = after_release + next_members;
    let now = Utc::now();
    sqlx::query(
        r#"UPDATE helper_teams
           SET assigned_members = $1, updated_at = $2
           WHERE id = $3"#,
    )
    .bind(new_assigned as i32)
    .bind(now)
    .bind(team.id)
    .execute(&mut *tx)
    .await?;

    let final_assigned = if is_terminal_status(next_status) {
        new_assigned.saturating_sub(next_members)
    } else {
        new_assigned
    };
    if final_assigned != new_assigned {
        sqlx::query(
            r#"UPDATE helper_teams
               SET assigned_members = $1, updated_at = $2
               WHERE id = $3"#,
        )
        .bind(final_assigned as i32)
        .bind(now)
        .bind(team.id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"UPDATE helper_allocations
           SET members_deployed = $1,
               status = $2,
               updated_at = $3
           WHERE id = $4"#,
    )
    .bind(next_members as i32)
    .bind(serde_json::to_value(next_status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    fetch_postgres(pool, id).await
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
    pub status: String,
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
            status: serde_json::from_str(&row.status)
                .or_else(|_| serde_json::from_str(&format!("\"{}\"", row.status)))
                .unwrap_or(AllocationStatus::EnRoute),
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
