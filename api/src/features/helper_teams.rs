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
    features::flush::{FlushKind, FlushMark},
    state::AppState,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HelperTeamStatus {
    Available,
    Deployed,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperTeam {
    pub id: Uuid,
    pub center_id: Option<Uuid>,
    pub team_name: String,
    pub total_members: u32,
    pub assigned_members: u32,
    pub status: HelperTeamStatus,
    pub latitude: f64,
    pub longitude: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl HelperTeam {
    /// Available deployment capacity: `total_members - assigned_members`.
    pub fn available_capacity(&self) -> u32 {
        self.total_members.saturating_sub(self.assigned_members)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateHelperTeam {
    pub center_id: Option<Uuid>,
    pub team_name: String,
    pub total_members: u32,
    pub latitude: f64,
    pub longitude: f64,
}

impl CreateHelperTeam {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.team_name.trim().is_empty() {
            return Err(ApiError::Validation("team_name must not be empty".into()));
        }
        if self.team_name.len() > 100 {
            return Err(ApiError::Validation(
                "team_name must not exceed 100 characters".into(),
            ));
        }
        if self.total_members == 0 {
            return Err(ApiError::Validation(
                "total_members must be greater than 0".into(),
            ));
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(ApiError::Validation(
                "latitude must be between -90 and 90".into(),
            ));
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(ApiError::Validation(
                "longitude must be between -180 and 180".into(),
            ));
        }
        Ok(())
    }
}

/// Query-parameter filters for the single `GET /helper-teams` route.
#[derive(Debug, Default, Deserialize)]
pub struct ListHelperTeamsQuery {
    pub status: Option<HelperTeamStatus>,
    pub center_id: Option<Uuid>,
    pub has_capacity: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateHelperTeam {
    pub status: Option<HelperTeamStatus>,
    pub assigned_members: Option<u32>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateHelperTeam>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let now = Utc::now();
    let team = HelperTeam {
        id: Uuid::new_v4(),
        center_id: input.center_id,
        team_name: input.team_name.trim().to_string(),
        total_members: input.total_members,
        assigned_members: 0,
        status: HelperTeamStatus::Available,
        latitude: input.latitude,
        longitude: input.longitude,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    };

    if let Some(pool) = &state.database {
        insert_postgres(pool, &team).await?;
    } else {
        state
            .helper_teams
            .write()
            .await
            .insert(team.id, team.clone());
    }
    state.enqueue_flush(FlushMark::new(FlushKind::HelperTeam, team.id, 0));
    Ok((StatusCode::CREATED, Json(team)))
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListHelperTeamsQuery>,
) -> Result<Json<Vec<HelperTeam>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool, &query).await?,
        None => state
            .helper_teams
            .read()
            .await
            .values()
            .filter(|&team| query.matches(team))
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
) -> Result<Json<HelperTeam>, ApiError> {
    let team = match &state.database {
        Some(pool) => fetch_postgres(pool, id).await?,
        None => state
            .helper_teams
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(ApiError::NotFound)?,
    };
    Ok(Json(team))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(update): Json<UpdateHelperTeam>,
) -> Result<Json<HelperTeam>, ApiError> {
    let result = match &state.database {
        Some(pool) => update_postgres(pool, id, update).await,
        None => update_in_memory(&state, id, update).await,
    }?;
    state.enqueue_flush(FlushMark::new(FlushKind::HelperTeam, id, 0));
    Ok(result)
}

/// DELETE /api/v1/helper-teams/{id}. Dependent helper allocations are removed
/// by the Postgres FK cascade; in-memory allocations are reconciled later.
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    match &state.database {
        Some(pool) => delete_postgres(pool, id).await?,
        None => {
            state
                .helper_teams
                .write()
                .await
                .remove(&id)
                .ok_or(ApiError::NotFound)?;
        }
    }
    state.enqueue_flush(FlushMark::new(FlushKind::HelperTeam, id, 0));
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_postgres(pool: &sqlx::PgPool, id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM helper_teams WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

impl ListHelperTeamsQuery {
    fn matches(&self, team: &HelperTeam) -> bool {
        if let Some(status) = self.status
            && team.status != status
        {
            return false;
        }
        if let Some(center_id) = self.center_id
            && team.center_id != Some(center_id)
        {
            return false;
        }
        if let Some(has_capacity) = self.has_capacity
            && (team.available_capacity() > 0) != has_capacity
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

async fn update_in_memory(
    state: &AppState,
    id: Uuid,
    update: UpdateHelperTeam,
) -> Result<Json<HelperTeam>, ApiError> {
    let mut teams = state.helper_teams.write().await;
    let team = teams.get_mut(&id).ok_or(ApiError::NotFound)?;
    if let Some(assigned) = update.assigned_members {
        if assigned > team.total_members {
            return Err(ApiError::Validation(format!(
                "assigned_members ({assigned}) exceeds total_members ({})",
                team.total_members
            )));
        }
        team.assigned_members = assigned;
    }
    if let Some(status) = update.status {
        team.status = status;
    }
    team.updated_at = Utc::now();
    let snapshot = team.clone();
    drop(teams);
    state.enqueue_flush(FlushMark::new(FlushKind::HelperTeam, id, 0));
    Ok(Json(snapshot))
}

pub(crate) async fn insert_postgres(
    pool: &sqlx::PgPool,
    team: &HelperTeam,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"INSERT INTO helper_teams (
            id, center_id, team_name, total_members, assigned_members,
            status, current_location, created_at, updated_at, server_synced_at
        ) VALUES ($1, $2, $3, $4, $5, $6,
                  ST_SetSRID(ST_MakePoint($7, $8), 4326)::geography,
                  $9, $10, $11)"#,
    )
    .bind(team.id)
    .bind(team.center_id)
    .bind(&team.team_name)
    .bind(team.total_members as i32)
    .bind(team.assigned_members as i32)
    .bind(serde_json::to_value(team.status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(team.longitude)
    .bind(team.latitude)
    .bind(team.created_at)
    .bind(team.updated_at)
    .bind(team.server_synced_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &ListHelperTeamsQuery,
) -> Result<Vec<HelperTeam>, ApiError> {
    query.validate()?;
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(status) = query.status {
        builder.push(" AND status = ");
        builder.push_bind(
            serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?,
        );
    }
    if let Some(center_id) = query.center_id {
        builder.push(" AND center_id = ");
        builder.push_bind(center_id);
    }
    if let Some(has_capacity) = query.has_capacity {
        builder.push(if has_capacity {
            " AND assigned_members < total_members"
        } else {
            " AND assigned_members >= total_members"
        });
    }
    builder.push(" ORDER BY team_name ASC");
    if let Some(limit) = query.limit {
        builder.push(" LIMIT ");
        builder.push_bind(limit);
    }
    if let Some(offset) = query.offset {
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }
    let rows = builder
        .build_query_as::<HelperTeamRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(HelperTeam::from).collect())
}

#[allow(dead_code)] // consumed by the dispatch engine in src/features/dispatch.rs
pub(crate) async fn list_all_postgres(pool: &sqlx::PgPool) -> Result<Vec<HelperTeam>, ApiError> {
    let rows = sqlx::query_as::<_, HelperTeamRow>(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams
           ORDER BY team_name ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(HelperTeam::from).collect())
}

/// Bump the assigned_members counter inside an existing transaction. The
/// dispatcher pre-screens against `HelperTeam::available_capacity()` so
/// the table CHECK constraint (assigned_members <= total_members) holds.
#[allow(dead_code)] // consumed by src/features/dispatch.rs
pub(crate) async fn bump_assigned_postgres_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team_id: Uuid,
    delta: u32,
    now: DateTime<Utc>,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"UPDATE helper_teams
           SET assigned_members = assigned_members + $1, updated_at = $2
           WHERE id = $3"#,
    )
    .bind(delta as i32)
    .bind(now)
    .bind(team_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn fetch_postgres(pool: &sqlx::PgPool, id: Uuid) -> Result<HelperTeam, ApiError> {
    let row = sqlx::query_as::<_, HelperTeamRow>(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(HelperTeam::from(row))
}

async fn update_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
    update: UpdateHelperTeam,
) -> Result<Json<HelperTeam>, ApiError> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, HelperTeamRow>(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams WHERE id = $1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;

    let team: HelperTeam = HelperTeam::from(existing);
    if let Some(assigned) = update.assigned_members
        && assigned > team.total_members
    {
        return Err(ApiError::Validation(format!(
            "assigned_members ({assigned}) exceeds total_members ({})",
            team.total_members
        )));
    }

    let now = Utc::now();
    sqlx::query(
        r#"UPDATE helper_teams
           SET assigned_members = COALESCE($1, assigned_members),
               status = COALESCE($2, status),
               updated_at = $3
           WHERE id = $4"#,
    )
    .bind(update.assigned_members.map(|value| value as i32))
    .bind(
        update
            .status
            .map(|status| serde_json::to_value(status).unwrap_or(serde_json::Value::Null)),
    )
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, HelperTeamRow>(
        r#"SELECT id, center_id, team_name, total_members, assigned_members, status,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM helper_teams WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Json(HelperTeam::from(row)))
}

#[derive(sqlx::FromRow)]
pub struct HelperTeamRow {
    pub id: Uuid,
    pub center_id: Option<Uuid>,
    pub team_name: String,
    pub total_members: i32,
    pub assigned_members: i32,
    pub status: String,
    pub latitude: f64,
    pub longitude: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<HelperTeamRow> for HelperTeam {
    fn from(row: HelperTeamRow) -> Self {
        HelperTeam {
            id: row.id,
            center_id: row.center_id,
            team_name: row.team_name,
            total_members: row.total_members as u32,
            assigned_members: row.assigned_members as u32,
            status: serde_json::from_str(&row.status)
                .or_else(|_| serde_json::from_str(&format!("\"{}\"", row.status)))
                .unwrap_or(HelperTeamStatus::Available),
            latitude: row.latitude,
            longitude: row.longitude,
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
