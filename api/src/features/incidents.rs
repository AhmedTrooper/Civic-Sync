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
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn weight(self) -> f64 {
        match self {
            Severity::Low => 1.0,
            Severity::Medium => 2.5,
            Severity::High => 4.0,
            Severity::Critical => 6.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Pending,
    Dispatched,
    Resolved,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourceNeed {
    Ambulance,
    RescueTeam,
    Helicopter,
    Hospital,
    Food,
    Shelter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Normal,
    Flood,
    Fire,
    StructuralCollapse,
    Weather,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: Uuid,
    pub region: String,
    pub severity: Severity,
    pub casualties: u32,
    pub affected_population: u32,
    pub time_sensitivity: u8,
    pub resource_needs: Vec<ResourceNeed>,
    pub environment: Environment,
    pub status: IncidentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIncident {
    pub region: String,
    pub severity: Severity,
    pub casualties: u32,
    pub affected_population: u32,
    pub time_sensitivity: u8,
    pub resource_needs: Vec<ResourceNeed>,
    pub environment: Environment,
}

impl CreateIncident {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.region.trim().is_empty() {
            return Err(ApiError::Validation("region must not be empty".into()));
        }
        if self.region.len() > 128 {
            return Err(ApiError::Validation("region exceeds 128 characters".into()));
        }
        if self.time_sensitivity == 0 || self.time_sensitivity > 10 {
            return Err(ApiError::Validation(
                "time_sensitivity must be between 1 and 10".into(),
            ));
        }
        if self.resource_needs.is_empty() {
            return Err(ApiError::Validation(
                "at least one resource_needs entry is required".into(),
            ));
        }
        Ok(())
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateIncident>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate()?;
    let now = Utc::now();
    let incident = Incident {
        id: Uuid::new_v4(),
        region: input.region.trim().to_string(),
        severity: input.severity,
        casualties: input.casualties,
        affected_population: input.affected_population,
        time_sensitivity: input.time_sensitivity,
        resource_needs: input.resource_needs,
        environment: input.environment,
        status: IncidentStatus::Pending,
        created_at: now,
        updated_at: now,
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

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Incident>>, ApiError> {
    let items = match &state.database {
        Some(pool) => list_postgres(pool).await?,
        None => state
            .incidents
            .read()
            .await
            .values()
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

pub fn priority_score(incident: &Incident) -> f64 {
    let severity = incident.severity.weight();
    let casualties = incident.casualties as f64;
    let population = incident.affected_population as f64;
    let time = incident.time_sensitivity as f64;
    let environment = match incident.environment {
        Environment::Normal => 1.0,
        Environment::Weather => 1.15,
        Environment::Fire => 1.25,
        Environment::Flood => 1.3,
        Environment::StructuralCollapse => 1.45,
    };
    ((severity * 4.0) + (casualties * 0.4) + (population * 0.05) + (time * 1.5)) * environment
}

pub fn priority_reasons(incident: &Incident) -> Vec<String> {
    let mut reasons: Vec<String> = Vec::new();
    reasons.push(format!(
        "severity {} contributes a base weight of {:.2}",
        serde_json::to_value(incident.severity)
            .ok()
            .and_then(|value| value.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".into()),
        incident.severity.weight() * 4.0
    ));
    if incident.casualties > 0 {
        reasons.push(format!(
            "{} casualty report increases urgency",
            incident.casualties
        ));
    }
    if incident.affected_population > 0 {
        reasons.push(format!(
            "{} people affected, expand coordination scope",
            incident.affected_population
        ));
    }
    reasons.push(format!(
        "time sensitivity is {}/10",
        incident.time_sensitivity
    ));
    reasons.push(format!(
        "environment {:?} escalates risk profile",
        incident.environment
    ));
    if !incident.resource_needs.is_empty() {
        let needs: Vec<String> = incident
            .resource_needs
            .iter()
            .filter_map(|need| serde_json::to_value(need).ok())
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        reasons.push(format!("requires {}", needs.join(", ")));
    }
    reasons
}

pub(crate) async fn insert_postgres(
    pool: &sqlx::PgPool,
    incident: &Incident,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"INSERT INTO incidents (
            id, region, severity, casualties, affected_population,
            time_sensitivity, resource_needs, environment, status, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
    )
    .bind(incident.id)
    .bind(&incident.region)
    .bind(serde_json::to_value(incident.severity).unwrap())
    .bind(incident.casualties as i32)
    .bind(incident.affected_population as i32)
    .bind(incident.time_sensitivity as i16)
    .bind(
        incident
            .resource_needs
            .iter()
            .map(|value| serde_json::to_value(value).unwrap())
            .collect::<Vec<_>>(),
    )
    .bind(serde_json::to_value(incident.environment).unwrap())
    .bind(serde_json::to_value(incident.status).unwrap())
    .bind(incident.created_at)
    .bind(incident.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_postgres(pool: &sqlx::PgPool) -> Result<Vec<Incident>, ApiError> {
    let rows = sqlx::query_as::<_, IncidentRow>(
        r#"SELECT id, region, severity, casualties, affected_population,
                  time_sensitivity, resource_needs, environment, status,
                  created_at, updated_at
           FROM incidents ORDER BY created_at ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Incident::from).collect::<Vec<_>>())
}

pub(crate) async fn fetch_postgres(pool: &sqlx::PgPool, id: Uuid) -> Result<Incident, ApiError> {
    let row = sqlx::query_as::<_, IncidentRow>(
        r#"SELECT id, region, severity, casualties, affected_population,
                  time_sensitivity, resource_needs, environment, status,
                  created_at, updated_at
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
    pub region: String,
    pub severity: serde_json::Value,
    pub casualties: i32,
    pub affected_population: i32,
    pub time_sensitivity: i16,
    pub resource_needs: Vec<serde_json::Value>,
    pub environment: serde_json::Value,
    pub status: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<IncidentRow> for Incident {
    fn from(row: IncidentRow) -> Self {
        let severity = serde_json::from_value(row.severity).unwrap_or(Severity::Low);
        let environment = serde_json::from_value(row.environment).unwrap_or(Environment::Normal);
        let status = serde_json::from_value(row.status).unwrap_or(IncidentStatus::Pending);
        let resource_needs = row
            .resource_needs
            .into_iter()
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect();
        Incident {
            id: row.id,
            region: row.region,
            severity,
            casualties: row.casualties as u32,
            affected_population: row.affected_population as u32,
            time_sensitivity: row.time_sensitivity as u8,
            resource_needs,
            environment,
            status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
