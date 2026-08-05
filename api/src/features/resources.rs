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

/// Vehicles & physical supplies tracked by the network.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceType {
    Ambulance,
    Boat,
    Helicopter,
    ReliefTruck,
    FoodPack,
    WaterSupply,
    ShelterKit,
    MedicalRation,
}

/// data.md §6.4 — only the four spec states are stored. FAILED is a
/// delta-trigger condition (§5B), not a stored status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceStatus {
    EnRoute,
    Stuck,
    Rejected,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: Uuid,
    pub center_id: Option<Uuid>,
    pub incident_id: Option<Uuid>,
    pub resource_type: ResourceType,
    pub unit_identifier: String,
    pub status: ResourceStatus,
    pub distance_passed_km: f64,
    pub distance_remaining_km: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateResource {
    pub center_id: Option<Uuid>,
    pub resource_type: ResourceType,
    pub unit_identifier: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl CreateResource {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.unit_identifier.trim().is_empty() {
            return Err(ApiError::Validation(
                "unit_identifier must not be empty".into(),
            ));
        }
        if self.unit_identifier.len() > 100 {
            return Err(ApiError::Validation(
                "unit_identifier must not exceed 100 characters".into(),
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

/// Query-parameter filters for the single `GET /resources` route.
#[derive(Debug, Default, Deserialize)]
pub struct ListResourcesQuery {
    pub status: Option<ResourceStatus>,
    pub resource_type: Option<ResourceType>,
    pub center_id: Option<Uuid>,
    pub incident_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatus {
    pub status: ResourceStatus,
    pub incident_id: Option<Uuid>,
    pub distance_passed_km: Option<f64>,
    pub distance_remaining_km: Option<f64>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateResource>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let now = Utc::now();
    let resource = Resource {
        id: Uuid::new_v4(),
        center_id: input.center_id,
        incident_id: None,
        resource_type: input.resource_type,
        unit_identifier: input.unit_identifier.trim().to_string(),
        status: ResourceStatus::EnRoute,
        distance_passed_km: 0.0,
        distance_remaining_km: 0.0,
        latitude: input.latitude,
        longitude: input.longitude,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
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

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListResourcesQuery>,
) -> Result<Json<Vec<Resource>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool, &query).await?,
        None => state
            .resources
            .read()
            .await
            .values()
            .filter(|&resource| query.matches(resource))
            .skip(query.offset.unwrap_or(0) as usize)
            .take(query.limit.unwrap_or(i64::MAX) as usize)
            .cloned()
            .collect::<Vec<_>>(),
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

impl ListResourcesQuery {
    fn matches(&self, resource: &Resource) -> bool {
        if let Some(status) = self.status
            && resource.status != status
        {
            return false;
        }
        if let Some(kind) = self.resource_type
            && resource.resource_type != kind
        {
            return false;
        }
        if let Some(center_id) = self.center_id
            && resource.center_id != Some(center_id)
        {
            return false;
        }
        if let Some(incident_id) = self.incident_id
            && resource.incident_id != Some(incident_id)
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
    update: UpdateStatus,
) -> Result<Json<Resource>, ApiError> {
    let mut resources = state.resources.write().await;
    let resource = resources.get_mut(&id).ok_or(ApiError::NotFound)?;
    resource.status = update.status;
    resource.incident_id = update.incident_id;
    if let Some(passed) = update.distance_passed_km {
        resource.distance_passed_km = passed;
    }
    if let Some(remaining) = update.distance_remaining_km {
        resource.distance_remaining_km = remaining;
    }
    resource.updated_at = Utc::now();
    Ok(Json(resource.clone()))
}

pub(crate) async fn insert_postgres(
    pool: &sqlx::PgPool,
    resource: &Resource,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"INSERT INTO resources (
            id, center_id, incident_id, resource_type, unit_identifier,
            status, distance_passed_km, distance_remaining_km,
            current_location, created_at, updated_at, server_synced_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                  ST_SetSRID(ST_MakePoint($9, $10), 4326)::geography,
                  $11, $12, $13)"#,
    )
    .bind(resource.id)
    .bind(resource.center_id)
    .bind(resource.incident_id)
    .bind(
        serde_json::to_value(resource.resource_type)
            .map_err(|err| ApiError::Internal(err.to_string()))?,
    )
    .bind(&resource.unit_identifier)
    .bind(serde_json::to_value(resource.status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(resource.distance_passed_km)
    .bind(resource.distance_remaining_km)
    .bind(resource.longitude)
    .bind(resource.latitude)
    .bind(resource.created_at)
    .bind(resource.updated_at)
    .bind(resource.server_synced_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &ListResourcesQuery,
) -> Result<Vec<Resource>, ApiError> {
    query.validate()?;
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"SELECT id, center_id, incident_id, resource_type, unit_identifier,
                  status, distance_passed_km, distance_remaining_km,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM resources"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(status) = query.status {
        builder.push(" AND status = ");
        builder.push_bind(
            serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?,
        );
    }
    if let Some(kind) = query.resource_type {
        builder.push(" AND resource_type = ");
        builder.push_bind(
            serde_json::to_value(kind).map_err(|err| ApiError::Internal(err.to_string()))?,
        );
    }
    if let Some(center_id) = query.center_id {
        builder.push(" AND center_id = ");
        builder.push_bind(center_id);
    }
    if let Some(incident_id) = query.incident_id {
        builder.push(" AND incident_id = ");
        builder.push_bind(incident_id);
    }
    builder.push(" ORDER BY updated_at ASC");
    if let Some(limit) = query.limit {
        builder.push(" LIMIT ");
        builder.push_bind(limit);
    }
    if let Some(offset) = query.offset {
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }
    let rows = builder
        .build_query_as::<ResourceRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Resource::from).collect())
}

pub(crate) async fn list_all_postgres(pool: &sqlx::PgPool) -> Result<Vec<Resource>, ApiError> {
    list_postgres(pool, &ListResourcesQuery::default()).await
}

async fn update_status_postgres(
    pool: &sqlx::PgPool,
    id: Uuid,
    update: UpdateStatus,
) -> Result<Json<Resource>, ApiError> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, ResourceRow>(
        r#"SELECT id, center_id, incident_id, resource_type, unit_identifier,
                  status, distance_passed_km, distance_remaining_km,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM resources WHERE id = $1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;

    let resource: Resource = Resource::from(existing);
    drop(resource); // transitioned freely per data.md §6.4 (no transition rules specified).

    let now = Utc::now();
    sqlx::query(
        r#"UPDATE resources
           SET status = $1,
               incident_id = COALESCE($2, incident_id),
               distance_passed_km = COALESCE($3, distance_passed_km),
               distance_remaining_km = COALESCE($4, distance_remaining_km),
               updated_at = $5
           WHERE id = $6"#,
    )
    .bind(serde_json::to_value(update.status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(update.incident_id)
    .bind(update.distance_passed_km)
    .bind(update.distance_remaining_km)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, ResourceRow>(
        r#"SELECT id, center_id, incident_id, resource_type, unit_identifier,
                  status, distance_passed_km, distance_remaining_km,
                  COALESCE(ST_Y(current_location::geometry), 0.0) AS latitude,
                  COALESCE(ST_X(current_location::geometry), 0.0) AS longitude,
                  created_at, updated_at, server_synced_at
           FROM resources WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Json(Resource::from(row)))
}

#[derive(sqlx::FromRow)]
pub struct ResourceRow {
    pub id: Uuid,
    pub center_id: Option<Uuid>,
    pub incident_id: Option<Uuid>,
    pub resource_type: serde_json::Value,
    pub unit_identifier: String,
    pub status: serde_json::Value,
    pub distance_passed_km: f64,
    pub distance_remaining_km: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<ResourceRow> for Resource {
    fn from(row: ResourceRow) -> Self {
        Resource {
            id: row.id,
            center_id: row.center_id,
            incident_id: row.incident_id,
            resource_type: serde_json::from_value(row.resource_type)
                .unwrap_or(ResourceType::Ambulance),
            unit_identifier: row.unit_identifier,
            status: serde_json::from_value(row.status).unwrap_or(ResourceStatus::EnRoute),
            distance_passed_km: row.distance_passed_km,
            distance_remaining_km: row.distance_remaining_km,
            latitude: row.latitude,
            longitude: row.longitude,
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
