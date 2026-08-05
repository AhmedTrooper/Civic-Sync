use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Ambulance,
    RescueTeam,
    Helicopter,
    Hospital,
    EmergencyCenter,
    FoodTruck,
    Boat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourceStatus {
    Available,
    Dispatched,
    Offline,
    Maintenance,
}

impl ResourceStatus {
    fn can_transition_to(self, target: ResourceStatus) -> bool {
        use ResourceStatus::*;
        matches!(
            (self, target),
            (Available, Dispatched)
                | (Available, Offline)
                | (Available, Maintenance)
                | (Dispatched, Available)
                | (Dispatched, Maintenance)
                | (Offline, Available)
                | (Offline, Maintenance)
                | (Maintenance, Available)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: Uuid,
    pub kind: ResourceKind,
    pub capacity: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub status: ResourceStatus,
    pub current_incident_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateResource {
    pub kind: ResourceKind,
    pub capacity: u32,
    pub latitude: f64,
    pub longitude: f64,
}

impl CreateResource {
    pub fn validate(&self) -> Result<(), ApiError> {
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
        if self.capacity == 0 {
            return Err(ApiError::Validation(
                "capacity must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatus {
    pub status: ResourceStatus,
    pub incident_id: Option<Uuid>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateResource>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let resource = Resource {
        id: Uuid::new_v4(),
        kind: input.kind,
        capacity: input.capacity,
        latitude: input.latitude,
        longitude: input.longitude,
        status: ResourceStatus::Available,
        current_incident_id: None,
        updated_at: Utc::now(),
    };

    if let Some(pool) = &state.database {
        insert_postgres(pool, &resource).await?;
    } else {
        state
            .resources
            .write()
            .await
            .insert(resource.id, resource.clone());
    }
    Ok((StatusCode::CREATED, Json(resource)))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Resource>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool).await?,
        None => state.resources.read().await.values().cloned().collect(),
    };
    Ok(Json(items))
}

pub async fn update_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(update): Json<UpdateStatus>,
) -> Result<Json<Resource>, ApiError> {
    match &state.database {
        Some(pool) => update_status_postgres(pool, id, update).await,
        None => update_status_in_memory(&state, id, update).await,
    }
}

async fn update_status_in_memory(
    state: &AppState,
    id: Uuid,
    update: UpdateStatus,
) -> Result<Json<Resource>, ApiError> {
    let mut resources = state.resources.write().await;
    let resource = resources.get_mut(&id).ok_or(ApiError::NotFound)?;
    if !resource.status.can_transition_to(update.status) {
        return Err(ApiError::Conflict(format!(
            "cannot transition from {:?} to {:?}",
            resource.status, update.status
        )));
    }
    resource.status = update.status;
    resource.current_incident_id = update.incident_id;
    resource.updated_at = Utc::now();
    Ok(Json(resource.clone()))
}

pub(crate) async fn insert_postgres(
    pool: &sqlx::PgPool,
    resource: &Resource,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"INSERT INTO resources (
            id, kind, capacity, latitude, longitude, status,
            current_incident_id, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(resource.id)
    .bind(serde_json::to_value(resource.kind).unwrap())
    .bind(resource.capacity as i32)
    .bind(resource.latitude)
    .bind(resource.longitude)
    .bind(serde_json::to_value(resource.status).unwrap())
    .bind(resource.current_incident_id)
    .bind(resource.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_postgres(pool: &sqlx::PgPool) -> Result<Vec<Resource>, ApiError> {
    let rows = sqlx::query_as::<_, ResourceRow>(
        r#"SELECT id, kind, capacity, latitude, longitude, status,
                  current_incident_id, updated_at
           FROM resources ORDER BY updated_at ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Resource::from).collect::<Vec<_>>())
}

async fn update_status_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
    update: UpdateStatus,
) -> Result<Json<Resource>, ApiError> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, ResourceRow>(
        r#"SELECT id, kind, capacity, latitude, longitude, status,
                  current_incident_id, updated_at
           FROM resources WHERE id = $1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;

    let resource: Resource = Resource::from(existing);
    if !resource.status.can_transition_to(update.status) {
        return Err(ApiError::Conflict(format!(
            "cannot transition from {:?} to {:?}",
            resource.status, update.status
        )));
    }

    let now = Utc::now();
    sqlx::query(
        r#"UPDATE resources
           SET status = $1, current_incident_id = $2, updated_at = $3
           WHERE id = $4"#,
    )
    .bind(serde_json::to_value(update.status).unwrap())
    .bind(update.incident_id)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, ResourceRow>(
        r#"SELECT id, kind, capacity, latitude, longitude, status,
                  current_incident_id, updated_at
           FROM resources WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Json(Resource::from(row)))
}

#[derive(sqlx::FromRow)]
struct ResourceRow {
    pub id: Uuid,
    pub kind: serde_json::Value,
    pub capacity: i32,
    pub latitude: f64,
    pub longitude: f64,
    pub status: serde_json::Value,
    pub current_incident_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

impl From<ResourceRow> for Resource {
    fn from(row: ResourceRow) -> Self {
        Resource {
            id: row.id,
            kind: serde_json::from_value(row.kind).unwrap_or(ResourceKind::Ambulance),
            capacity: row.capacity as u32,
            latitude: row.latitude,
            longitude: row.longitude,
            status: serde_json::from_value(row.status).unwrap_or(ResourceStatus::Available),
            current_incident_id: row.current_incident_id,
            updated_at: row.updated_at,
        }
    }
}
