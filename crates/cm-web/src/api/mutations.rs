//! Mutation history endpoint.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Json;
use chrono::{DateTime, Utc};
use cm_core::{ContextStore, MutationAction, MutationRecord, MutationSource};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::api::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct MutationsQuery {
    entry_id: Option<String>,
    action: Option<String>,
    source: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<u32>,
}

pub async fn list_mutations(
    State(state): State<Arc<AppState>>,
    Query(q): Query<MutationsQuery>,
) -> Result<Json<Vec<MutationRecord>>, ApiError> {
    let entry_id = q
        .entry_id
        .map(|s| {
            Uuid::parse_str(&s).map_err(|_| {
                ApiError(cm_core::CmError::Validation(format!(
                    "invalid entry_id UUID: '{s}'"
                )))
            })
        })
        .transpose()?;

    let action = q.action.map(|s| parse_action(&s)).transpose()?;
    let source = q.source.map(|s| parse_source(&s)).transpose()?;

    let since = q.since.map(|s| parse_datetime(&s, "since")).transpose()?;
    let until = q.until.map(|s| parse_datetime(&s, "until")).transpose()?;

    let limit = q.limit.unwrap_or(50).clamp(1, 200);

    let records = state
        .store
        .list_mutations(entry_id, action, source, since, until, limit)
        .await?;

    Ok(Json(records))
}

fn parse_action(s: &str) -> Result<MutationAction, ApiError> {
    serde_json::from_value::<MutationAction>(serde_json::Value::String(s.to_owned())).map_err(
        |_| {
            ApiError(cm_core::CmError::Validation(format!(
                "invalid action: '{s}' (expected create, update, forget, supersede)"
            )))
        },
    )
}

fn parse_source(s: &str) -> Result<MutationSource, ApiError> {
    serde_json::from_value::<MutationSource>(serde_json::Value::String(s.to_owned())).map_err(
        |_| {
            ApiError(cm_core::CmError::Validation(format!(
                "invalid source: '{s}' (expected mcp, cli, web, helix)"
            )))
        },
    )
}

fn parse_datetime(s: &str, field: &str) -> Result<DateTime<Utc>, ApiError> {
    s.parse::<DateTime<Utc>>().map_err(|_| {
        ApiError(cm_core::CmError::Validation(format!(
            "invalid {field} timestamp: '{s}' (expected ISO 8601)"
        )))
    })
}
