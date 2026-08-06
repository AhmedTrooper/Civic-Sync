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
pub enum IncidentStatus {
    Active,
    Dispatched,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: Uuid,
    pub title: String,
    pub primary_center_id: Uuid,
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

/// PATCH body for `PATCH /api/v1/incidents/{id}`. Every field is optional;
/// clients send only the delta they want applied. data.md §5B requires that
/// intra-incident deltas (Δcasualty ≥ 10, severity 3 → 5) wake the trigger
/// driver, so this endpoint exists specifically to unlock those signals
/// after creation.
#[derive(Debug, Deserialize)]
pub struct UpdateIncident {
    pub title: Option<String>,
    pub severity_level: Option<u8>,
    pub affected_people: Option<u32>,
    pub casualty_count: Option<u32>,
    pub status: Option<IncidentStatus>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl UpdateIncident {
    pub fn validate(&self) -> Result<(), ApiError> {
        if let Some(title) = &self.title {
            if title.trim().is_empty() {
                return Err(ApiError::Validation("title must not be empty".into()));
            }
            if title.len() > 255 {
                return Err(ApiError::Validation(
                    "title must not exceed 255 characters".into(),
                ));
            }
        }
        if let Some(severity) = self.severity_level
            && !(1..=5).contains(&severity)
        {
            return Err(ApiError::Validation(
                "severity_level must be between 1 and 5".into(),
            ));
        }
        if let Some(latitude) = self.latitude
            && !(-90.0..=90.0).contains(&latitude)
        {
            return Err(ApiError::Validation(
                "latitude must be between -90 and 90".into(),
            ));
        }
        if let Some(longitude) = self.longitude
            && !(-180.0..=180.0).contains(&longitude)
        {
            return Err(ApiError::Validation(
                "longitude must be between -180 and 180".into(),
            ));
        }
        Ok(())
    }

    /// Whether this PATCH carries any state-mutating payload. Used to
    /// short-circuit no-op PATCHes (e.g. `{}`) so we don't fire triggers
    /// or enqueue flush marks for nothing.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.severity_level.is_none()
            && self.affected_people.is_none()
            && self.casualty_count.is_none()
            && self.status.is_none()
            && self.latitude.is_none()
            && self.longitude.is_none()
    }
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = ((d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2))
    .clamp(0.0, 1.0);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateIncident>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let now = Utc::now();
    let centers = match &state.database {
        Some(pool) => crate::features::centers::list_all_postgres(pool).await?,
        None => state
            .centers
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>(),
    };

    let mut closest_center_id = Uuid::nil();
    let mut min_dist = f64::MAX;
    for center in &centers {
        let dist = haversine_distance(
            input.latitude,
            input.longitude,
            center.latitude,
            center.longitude,
        );
        if dist < min_dist {
            min_dist = dist;
            closest_center_id = center.id;
        }
    }

    let incident = Incident {
        id: Uuid::new_v4(),
        title: input.title.trim().to_string(),
        primary_center_id: closest_center_id,
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
    state.enqueue_flush(FlushMark::new(FlushKind::Incident, incident.id, 0));
    fire_incident_triggers(&state, &incident);
    Ok((StatusCode::CREATED, Json(incident)))
}

/// Emit `TriggerEvent::NewSeverityFiveIncident` and/or
/// `TriggerEvent::NewMassCasualtyIncident` per data.md §5B trigger
/// conditions. Each signal is fire-and-forget; the 30s ticker is the
/// authoritative re-evaluation point.
fn fire_incident_triggers(state: &AppState, incident: &Incident) {
    fire_creation_triggers_for(state, incident);
}

/// Public alias for [`fire_incident_triggers`] so the simulator and
/// the manual injection endpoint can reuse the exact same trigger
/// fan-out without re-implementing it.
pub fn fire_creation_triggers_for(state: &AppState, incident: &Incident) {
    let sender = state.triggers_tx.clone();
    let id = incident.id;
    let severity_five = incident.severity_level == 5;
    let mass_casualty = incident.casualty_count >= 10;
    if !severity_five && !mass_casualty {
        return;
    }
    tokio::spawn(async move {
        if severity_five
            && let Err(error) = sender
                .send(crate::features::triggers::TriggerEvent::NewSeverityFiveIncident(id))
                .await
        {
            tracing::warn!(
                error = %error,
                incident_id = %id,
                "failed to enqueue severity-five trigger"
            );
        }
        if mass_casualty
            && let Err(error) = sender
                .send(crate::features::triggers::TriggerEvent::NewMassCasualtyIncident(id))
                .await
        {
            tracing::warn!(
                error = %error,
                incident_id = %id,
                "failed to enqueue mass-casualty trigger"
            );
        }
    });
}

/// Detect intra-incident deltas after a PATCH (data.md §5B):
/// * Δcasualty ≥ 10 (jump, not +1).
/// * Severity escalation 3 → 5.
///
/// The pre-PATCH snapshot must be passed in so we don't fire on values
/// that were already at the threshold before the update.
fn fire_update_triggers(state: &AppState, previous: &Incident, updated: &Incident) {
    let delta_casualties = updated
        .casualty_count
        .saturating_sub(previous.casualty_count);
    let severity_escalated_to_five = previous.severity_level < 5 && updated.severity_level == 5;
    if delta_casualties < 10 && !severity_escalated_to_five {
        return;
    }
    let sender = state.triggers_tx.clone();
    let id = updated.id;
    tokio::spawn(async move {
        if delta_casualties >= 10
            && let Err(error) = sender
                .send(
                    crate::features::triggers::TriggerEvent::DeltaCasualtyBurst {
                        incident_id: id,
                        delta: delta_casualties,
                    },
                )
                .await
        {
            tracing::warn!(
                error = %error,
                incident_id = %id,
                "failed to enqueue Δcasualty trigger"
            );
        }
        if severity_escalated_to_five
            && let Err(error) = sender
                .send(crate::features::triggers::TriggerEvent::SeverityEscalatedToFive(id))
                .await
        {
            tracing::warn!(
                error = %error,
                incident_id = %id,
                "failed to enqueue severity-escalation trigger"
            );
        }
    });
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

/// PATCH /api/v1/incidents/{id}. Applies the partial update, fires
/// intra-incident delta triggers per data.md §5B, and enqueues a flush mark
/// so the 60s sync layer stamps `server_synced_at` on the row.
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateIncident>,
) -> Result<Json<Incident>, ApiError> {
    input.validate()?;
    if input.is_empty() {
        // No-op PATCH: still respond with the current row so clients can
        // round-trip without needing a separate GET.
        return get_one(State(state), Path(id)).await;
    }
    let (previous, updated) = match &state.database {
        Some(pool) => update_postgres(pool, &state, id, input).await?,
        None => update_in_memory(&state, id, input).await?,
    };
    state.enqueue_flush(FlushMark::new(FlushKind::Incident, id, 0));
    fire_update_triggers(&state, &previous, &updated);
    Ok(Json(updated))
}

/// DELETE /api/v1/incidents/{id}. Removes the row and emits a flush mark so
/// the 60s sync layer forgets it. Dependent `helper_allocations` rows that
/// point at this incident are removed by the FK cascade on Postgres
/// (`migrations/0004_helper_teams_allocations.sql:36`); in-memory orphans are
/// left for a future reconciliation cycle.
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    match &state.database {
        Some(pool) => delete_postgres(pool, id).await?,
        None => delete_in_memory(&state, id).await?,
    }
    state.enqueue_flush(FlushMark::new(FlushKind::Incident, id, 0));
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_in_memory(state: &AppState, id: Uuid) -> Result<(), ApiError> {
    state
        .incidents
        .write()
        .await
        .remove(&id)
        .ok_or(ApiError::NotFound)?;
    Ok(())
}

async fn delete_postgres(pool: &sqlx::PgPool, id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM incidents WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

async fn update_in_memory(
    state: &AppState,
    id: Uuid,
    update: UpdateIncident,
) -> Result<(Incident, Incident), ApiError> {
    let mut incidents = state.incidents.write().await;
    let previous = incidents.get(&id).ok_or(ApiError::NotFound)?.clone();
    let incident = incidents.get_mut(&id).ok_or(ApiError::NotFound)?;
    if let Some(title) = update.title {
        incident.title = title.trim().to_string();
    }
    if let Some(severity) = update.severity_level {
        incident.severity_level = severity;
    }
    if let Some(affected) = update.affected_people {
        incident.affected_people = affected;
    }
    if let Some(casualty) = update.casualty_count {
        incident.casualty_count = casualty;
    }
    if let Some(status) = update.status {
        incident.status = status;
    }
    if let Some(lat) = update.latitude {
        incident.latitude = lat;
    }
    if let Some(lon) = update.longitude {
        incident.longitude = lon;
    }
    incident.updated_at = Utc::now();
    let snapshot = incident.clone();
    drop(incidents);
    Ok((previous, snapshot))
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

/// Dynamic prioritization score (data.md §5A): severity 1–5 is the base
/// weight, casualty load and affected population amplify it, and
/// time-on-grid escalates stale crises so an unattended incident is never
/// starved forever. Deterministic and explainable via [`priority_reasons`].
pub fn priority_score(incident: &Incident) -> f64 {
    let severity = f64::from(incident.severity_level);
    let casualties = f64::from(incident.casualty_count);
    let affected = f64::from(incident.affected_people);
    let age_minutes = (Utc::now() - incident.created_at).num_seconds().max(0) as f64 / 60.0;
    let time_factor = (age_minutes * 0.05).min(10.0);
    (severity * 10.0) + (casualties * 0.5) + (affected * 0.02) + time_factor
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
    let age_minutes = (Utc::now() - incident.created_at).num_seconds().max(0) / 60;
    if age_minutes >= 1 {
        reasons.push(format!(
            "active for {} min without resolution, escalating time sensitivity",
            age_minutes
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
            id, title, primary_center_id, severity_level, affected_people, casualty_count,
            latitude, longitude, status,
            created_at, updated_at, server_synced_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(incident.id)
    .bind(&incident.title)
    .bind(incident.primary_center_id)
    .bind(i32::from(incident.severity_level))
    .bind(incident.affected_people as i32)
    .bind(incident.casualty_count as i32)
    .bind(incident.latitude)
    .bind(incident.longitude)
    .bind(
        serde_json::to_value(incident.status)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string(),
    )
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
        r#"SELECT id, title, primary_center_id, severity_level, affected_people, casualty_count,
                  latitude, longitude, status,
                  created_at, updated_at, server_synced_at
           FROM incidents"#,
    );
    builder.push(" WHERE 1 = 1");
    if let Some(status) = query.status {
        builder.push(" AND status = ");
        builder.push_bind(
            serde_json::to_value(status)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
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
        r#"SELECT id, title, primary_center_id, severity_level, affected_people, casualty_count,
                  latitude, longitude, status,
                  created_at, updated_at, server_synced_at
           FROM incidents WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Incident::from(row))
}

async fn update_postgres(
    pool: &sqlx::PgPool,
    state: &AppState,
    id: Uuid,
    update: UpdateIncident,
) -> Result<(Incident, Incident), ApiError> {
    let _ = state; // re-export hook future use; caller enqueues the flush mark.
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, IncidentRow>(
        r#"SELECT id, title, primary_center_id, severity_level, affected_people, casualty_count,
                  latitude, longitude, status,
                  created_at, updated_at, server_synced_at
           FROM incidents WHERE id = $1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;
    let previous = Incident::from(existing);

    let status_json = update.status.map(|status| {
        serde_json::to_value(status)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    });

    let now = Utc::now();
    sqlx::query(
        r#"UPDATE incidents
           SET title = COALESCE($1, title),
               severity_level = COALESCE($2, severity_level),
               affected_people = COALESCE($3, affected_people),
               casualty_count = COALESCE($4, casualty_count),
               status = COALESCE($5, status),
               latitude = COALESCE($6, latitude),
               longitude = COALESCE($7, longitude),
               updated_at = $8
           WHERE id = $9"#,
    )
    .bind(update.title.as_deref())
    .bind(update.severity_level.map(i32::from))
    .bind(update.affected_people.map(|v| v as i32))
    .bind(update.casualty_count.map(|v| v as i32))
    .bind(status_json)
    .bind(update.latitude)
    .bind(update.longitude)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, IncidentRow>(
        r#"SELECT id, title, primary_center_id, severity_level, affected_people, casualty_count,
                  latitude, longitude, status,
                  created_at, updated_at, server_synced_at
           FROM incidents WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    let updated = Incident::from(row);
    Ok((previous, updated))
}

#[derive(sqlx::FromRow)]
pub struct IncidentRow {
    pub id: Uuid,
    pub title: String,
    pub primary_center_id: Uuid,
    pub severity_level: i32,
    pub affected_people: i32,
    pub casualty_count: i32,
    pub latitude: f64,
    pub longitude: f64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub server_synced_at: Option<DateTime<Utc>>,
}

impl From<IncidentRow> for Incident {
    fn from(row: IncidentRow) -> Self {
        let status = serde_json::from_str(&row.status)
            .or_else(|_| serde_json::from_str(&format!("\"{}\"", row.status)))
            .unwrap_or(IncidentStatus::Active);
        Incident {
            id: row.id,
            title: row.title,
            primary_center_id: row.primary_center_id,
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
