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
pub enum IncidentStatus {
    Active,
    Dispatched,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: Uuid,
    pub title: String,
    pub severity_level: u8,
    pub affected_people: u32,
    pub casualty_count: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub status: IncidentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIncident {
    pub title: String,
    pub severity_level: u8,
    pub affected_people: u32,
    pub casualty_count: u32,
    pub latitude: f64,
    pub longitude: f64,
}

impl CreateIncident {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.title.trim().is_empty() {
            return Err(ApiError::Validation("title must not be empty".into()));
        }
        if self.title.len() > 255 {
            return Err(ApiError::Validation(
                "title must not exceed 255 characters".into(),
            ));
        }
        if !(1..=5).contains(&self.severity_level) {
            return Err(ApiError::Validation(
                "severity_level must be between 1 and 5".into(),
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

/// Query-parameter filters for the single `GET /incidents` route.
#[derive(Debug, Default, Deserialize)]
pub struct ListIncidentsQuery {
    pub status: Option<IncidentStatus>,
    pub severity_level: Option<u8>,
    pub min_casualties: Option<u32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateIncident>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let now = Utc::now();
    let incident = Incident {
        id: Uuid::new_v4(),
        title: input.title.trim().to_string(),
        severity_level: input.severity_level,
        affected_people: input.affected_people,
        casualty_count: input.casualty_count,
        latitude: input.latitude,
        longitude: input.longitude,
        status: IncidentStatus::Active,
        created_at: now,
        updated_at: now,
        server_synced_at: None,
    };

    if let Some(pool) = &state.database {
        insert_postgres(pool, &incident).await?;
    } else {
        state
            .incidents
            .write()
            .await
            .insert(incident.id, incident.clone());
    }
    Ok((StatusCode::CREATED, Json(incident)))
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListIncidentsQuery>,
) -> Result<Json<Vec<Incident>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool, &query).await?,
        None => state
            .incidents
            .read()
            .await
            .values()
            .filter(|&incident| query.matches(incident))
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
) -> Result<Json<Incident>, ApiError> {
    let incident = match &state.database {
        Some(pool) => fetch_postgres(pool, id).await?,
        None => state
            .incidents
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(ApiError::NotFound)?,
    };
    Ok(Json(incident))
}

impl ListIncidentsQuery {
    fn matches(&self, incident: &Incident) -> bool {
        if let Some(status) = self.status
            && incident.status != status
        {
            return false;
        }
        if let Some(severity) = self.severity_level
            && incident.severity_level != severity
        {
            return false;
        }
        if let Some(min_casualties) = self.min_casualties
            && incident.casualty_count < min_casualties
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
        if let Some(severity) = self.severity_level
            && !(1..=5).contains(&severity)
        {
            return Err(ApiError::Validation(
                "severity_level must be between 1 and 5".into(),
            ));
        }
        Ok(())
    }
}

pub fn priority_score(incident: &Incident) -> f64 {
    let severity = f64::from(incident.severity_level);
    let casualties = f64::from(incident.casualty_count);
    let affected = f64::from(incident.affected_people);
    (severity * 10.0) + (casualties * 0.5) + (affected * 0.02)
}

pub fn priority_reasons(incident: &Incident) -> Vec<String> {
    let mut reasons = Vec::new();
    reasons.push(format!(
        "severity level {} of 5 contributes a base weight of {:.2}",
        incident.severity_level,
        f64::from(incident.severity_level) * 10.0
    ));
    if incident.casualty_count > 0 {
        reasons.push(format!(
            "{} casualties increase urgency",
            incident.casualty_count
        ));
    }
    if incident.affected_people > 0 {
        reasons.push(format!(
            "{} people affected, expand coordination scope",
            incident.affected_people
        ));
    }
    reasons.push(format!(
        "located at ({:.4}, {:.4})",
        incident.latitude, incident.longitude
    ));
    reasons
}

pub(crate) async fn insert_postgres(
    pool: &sqlx::PgPool,
    incident: &Incident,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"INSERT INTO incidents (
            id, title, severity_level, affected_people, casualty_count,
            location, status, created_at, updated_at, server_synced_at
        ) VALUES ($1, $2, $3, $4, $5,
                  ST_SetSRID(ST_MakePoint($6, $7), 4326)::geography,
                  $8, $9, $10, $11)"#,
    )
    .bind(incident.id)
    .bind(&incident.title)
    .bind(i32::from(incident.severity_level))
    .bind(incident.affected_people as i32)
    .bind(incident.casualty_count as i32)
    .bind(incident.longitude)
    .bind(incident.latitude)
    .bind(serde_json::to_value(incident.status).map_err(|err| ApiError::Internal(err.to_string()))?)
    .bind(incident.created_at)
    .bind(incident.updated_at)
    .bind(incident.server_synced_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &ListIncidentsQuery,
) -> Result<Vec<Incident>, ApiError> {
    query.validate()?;
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"SELECT id, title, severity_level, affected_people, casualty_count,
                  ST_Y(location::geometry) AS latitude,
                  ST_X(location::geometry) AS longitude,
                  status, created_at, updated_at, server_synced_at
           FROM incidents"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(status) = query.status {
        builder.push(" AND status = ");
        builder.push_bind(
            serde_json::to_value(status).map_err(|err| ApiError::Internal(err.to_string()))?,
        );
    }
    if let Some(severity) = query.severity_level {
        builder.push(" AND severity_level = ");
        builder.push_bind(i32::from(severity));
    }
    if let Some(min_casualties) = query.min_casualties {
        builder.push(" AND casualty_count >= ");
        builder.push_bind(min_casualties as i32);
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
        .build_query_as::<IncidentRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Incident::from).collect())
}

pub(crate) async fn list_all_postgres(pool: &sqlx::PgPool) -> Result<Vec<Incident>, ApiError> {
    list_postgres(pool, &ListIncidentsQuery::default()).await
}

pub(crate) async fn fetch_postgres(pool: &sqlx::PgPool, id: Uuid) -> Result<Incident, ApiError> {
    let row = sqlx::query_as::<_, IncidentRow>(
        r#"SELECT id, title, severity_level, affected_people, casualty_count,
                  ST_Y(location::geometry) AS latitude,
                  ST_X(location::geometry) AS longitude,
                  status, created_at, updated_at, server_synced_at
           FROM incidents WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Incident::from(row))
}

#[derive(sqlx::FromRow)]
pub struct IncidentRow {
    pub id: Uuid,
    pub title: String,
    pub severity_level: i32,
    pub affected_people: i32,
    pub casualty_count: i32,
    pub latitude: f64,
    pub longitude: f64,
    pub status: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<IncidentRow> for Incident {
    fn from(row: IncidentRow) -> Self {
        let status = serde_json::from_value(row.status).unwrap_or(IncidentStatus::Active);
        Incident {
            id: row.id,
            title: row.title,
            severity_level: row.severity_level as u8,
            affected_people: row.affected_people as u32,
            casualty_count: row.casualty_count as u32,
            latitude: row.latitude,
            longitude: row.longitude,
            status,
            created_at: row.created_at,
            updated_at: row.updated_at,
            server_synced_at: row.server_synced_at,
        }
    }
}
