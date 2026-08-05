use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssistanceStatus {
    Pending,
    Approved,
    Rejected,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceRequest {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub issue_description: String,
    pub status: AssistanceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssistanceRequest {
    pub resource_id: Uuid,
    pub issue_description: String,
}

impl CreateAssistanceRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.issue_description.trim().is_empty() {
            return Err(ApiError::Validation(
                "issue_description must not be empty".into(),
            ));
        }
        Ok(())
    }
}

/// Query-parameter filters for the single `GET /assistance-requests` route.
#[derive(Debug, Default, Deserialize)]
pub struct ListAssistanceRequestsQuery {
    pub status: Option<AssistanceStatus>,
    pub resource_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAssistanceStatus {
    pub status: AssistanceStatus,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateAssistanceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let now = Utc::now();
    let request = AssistanceRequest {
        id: Uuid::new_v4(),
        resource_id: input.resource_id,
        issue_description: input.issue_description.trim().to_string(),
        status: AssistanceStatus::Pending,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    };

    if let Some(pool) = &state.database {
        insert_postgres(pool, &request).await?;
    } else {
        state
            .assistance_requests
            .write()
            .await
            .insert(request.id, request.clone());
    }
    Ok((StatusCode::CREATED, Json(request)))
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListAssistanceRequestsQuery>,
) -> Result<Json<Vec<AssistanceRequest>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool, &query).await?,
        None => state
            .assistance_requests
            .read()
            .await
            .values()
            .filter(|&request| query.matches(request))
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
) -> Result<Json<AssistanceRequest>, ApiError> {
    let request = match &state.database {
        Some(pool) => fetch_postgres(pool, id).await?,
        None => state
            .assistance_requests
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(ApiError::NotFound)?,
    };
    Ok(Json(request))
}

pub async fn update_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(update): Json<UpdateAssistanceStatus>,
) -> Result<Json<AssistanceRequest>, ApiError> {
    match &state.database {
        Some(pool) => update_status_postgres(pool, id, update.status).await,
        None => update_status_in_memory(&state, id, update.status).await,
    }
}

impl ListAssistanceRequestsQuery {
    fn matches(&self, request: &AssistanceRequest) -> bool {
        if let Some(status) = self.status
            && request.status != status
        {
            return false;
        }
        if let Some(resource_id) = self.resource_id
            && request.resource_id != resource_id
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

async fn update_status_in_memory(
    state: &AppState,
    id: Uuid,
    status: AssistanceStatus,
) -> Result<Json<AssistanceRequest>, ApiError> {
    let mut requests = state.assistance_requests.write().await;
    let request = requests.get_mut(&id).ok_or(ApiError::NotFound)?;
    request.status = status;
    request.updated_at = Utc::now();
    Ok(Json(request.clone()))
}

pub(crate) async fn insert_postgres(
    pool: &sqlx::PgPool,
    request: &AssistanceRequest,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"INSERT INTO assistance_requests (
            id, resource_id, issue_description, status,
            created_at, updated_at, server_synced_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(request.id)
    .bind(request.resource_id)
    .bind(&request.issue_description)
    .bind(serde_json::to_value(request.status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(request.created_at)
    .bind(request.updated_at)
    .bind(request.server_synced_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &ListAssistanceRequestsQuery,
) -> Result<Vec<AssistanceRequest>, ApiError> {
    query.validate()?;
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"SELECT id, resource_id, issue_description, status,
                  created_at, updated_at, server_synced_at
           FROM assistance_requests"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(status) = query.status {
        builder.push(" AND status = ");
        builder.push_bind(
            serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?,
        );
    }
    if let Some(resource_id) = query.resource_id {
        builder.push(" AND resource_id = ");
        builder.push_bind(resource_id);
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
        .build_query_as::<AssistanceRequestRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(AssistanceRequest::from).collect())
}

pub(crate) async fn fetch_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
) -> Result<AssistanceRequest, ApiError> {
    let row = sqlx::query_as::<_, AssistanceRequestRow>(
        r#"SELECT id, resource_id, issue_description, status,
                  created_at, updated_at, server_synced_at
           FROM assistance_requests WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(AssistanceRequest::from(row))
}

async fn update_status_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
    status: AssistanceStatus,
) -> Result<Json<AssistanceRequest>, ApiError> {
    let now = Utc::now();
    let result = sqlx::query(
        r#"UPDATE assistance_requests
           SET status = $1, updated_at = $2
           WHERE id = $3"#,
    )
    .bind(serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    let row = sqlx::query_as::<_, AssistanceRequestRow>(
        r#"SELECT id, resource_id, issue_description, status,
                  created_at, updated_at, server_synced_at
           FROM assistance_requests WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Json(AssistanceRequest::from(row)))
}

#[derive(sqlx::FromRow)]
pub struct AssistanceRequestRow {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub issue_description: String,
    pub status: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<AssistanceRequestRow> for AssistanceRequest {
    fn from(row: AssistanceRequestRow) -> Self {
        AssistanceRequest {
            id: row.id,
            resource_id: row.resource_id,
            issue_description: row.issue_description,
            status: serde_json::from_value(row.status).unwrap_or(AssistanceStatus::Pending),
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
