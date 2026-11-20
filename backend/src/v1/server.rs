//! Dashboard, server views, and global history.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};

use crate::auth::AuthUser;
use crate::errors::ApiError;
use crate::state::AppState;
use crate::store::{HistoryEntry, HistoryFilter};

/// Dashboard summary.
#[derive(Debug, Serialize, ToSchema)]
pub struct Dashboard {
    pub zone_count: usize,
    pub recent_history: Vec<HistoryEntry>,
    pub server: Option<ServerSummary>,
}

/// Server version and uptime.
#[derive(Debug, Serialize, ToSchema)]
pub struct ServerSummary {
    pub version: String,
    pub uptime: Option<String>,
}

/// `GET /v1/dashboard`
#[utoipa::path(get, path = "/v1/dashboard", tag = "server", responses((status = 200, body = Dashboard)))]
pub async fn dashboard(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Dashboard>, ApiError> {
    let p = auth.principal(&state).await?;
    let perms = auth.permissions(&state).await?;
    let zones = state.pdns.list_zones().await?;
    let visible: Vec<String> = zones
        .iter()
        .filter_map(|z| crate::pdns::ZoneName::from_url_id(&z.id).map(|(n, _)| n))
        .filter(|z| p.can_access_zone(z))
        .map(|z| z.to_string())
        .collect();
    let recent_history = if perms.view_history {
        state
            .store
            .list_history(&HistoryFilter {
                zones: (!p.all_zones()).then(|| visible.clone()),
                limit: 10,
                ..Default::default()
            })
            .await?
    } else {
        Vec::new()
    };
    let server = if perms.view_server {
        let info: Value = state.pdns.get("").await?;
        let stats: Value = state
            .pdns
            .get("/statistics?statistic=uptime")
            .await
            .unwrap_or(Value::Null);
        Some(ServerSummary {
            version: info["version"].as_str().unwrap_or_default().to_owned(),
            uptime: stats
                .as_array()
                .and_then(|a| a.first())
                .and_then(|s| s["value"].as_str())
                .map(str::to_owned),
        })
    } else {
        None
    };
    Ok(Json(Dashboard {
        zone_count: visible.len(),
        recent_history,
        server,
    }))
}

async fn require_server(state: &AppState, auth: &AuthUser) -> Result<(), ApiError> {
    AuthUser::require(auth.permissions(state).await?.view_server)
}

/// `GET /v1/server`
#[utoipa::path(get, path = "/v1/server", tag = "server", responses((status = 200, body = Object)))]
pub async fn server(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, ApiError> {
    require_server(&state, &auth).await?;
    let v: Value = state.pdns.get("").await?;
    Ok(Json(serde_json::json!({
        "id": v["id"], "daemon_type": v["daemon_type"], "version": v["version"],
    })))
}

/// `GET /v1/server/statistics`
#[utoipa::path(get, path = "/v1/server/statistics", tag = "server", responses((status = 200, body = Object)))]
pub async fn statistics(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, ApiError> {
    require_server(&state, &auth).await?;
    Ok(Json(state.pdns.get("/statistics").await?))
}

/// `GET /v1/server/config`
#[utoipa::path(get, path = "/v1/server/config", tag = "server", responses((status = 200, body = Object)))]
pub async fn config(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, ApiError> {
    require_server(&state, &auth).await?;
    Ok(Json(state.pdns.get("/config").await?))
}

/// History filters.
#[derive(Debug, Deserialize, IntoParams)]
pub struct HistoryQuery {
    pub zone: Option<String>,
    pub actor: Option<String>,
    pub operation: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub before_id: Option<i64>,
    pub limit: Option<i64>,
}

/// A page of history.
#[derive(Debug, Serialize, ToSchema)]
pub struct HistoryPage {
    pub items: Vec<HistoryEntry>,
}

/// `GET /v1/history`
#[utoipa::path(get, path = "/v1/history", tag = "history", params(HistoryQuery), responses((status = 200, body = HistoryPage)))]
pub async fn history(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<HistoryPage>, ApiError> {
    AuthUser::require(auth.permissions(&state).await?.view_history)?;
    let p = auth.principal(&state).await?;
    let zone = q
        .zone
        .as_deref()
        .filter(|z| !z.is_empty())
        .and_then(crate::pdns::ZoneName::from_name)
        .map(|z| z.to_string());
    let items = state
        .store
        .list_history(&HistoryFilter {
            zones: (!p.all_zones()).then(|| p.zones.iter().cloned().collect()),
            zone,
            actor: q.actor.filter(|s| !s.is_empty()),
            operation: q.operation.filter(|s| !s.is_empty()),
            since: q.since,
            until: q.until,
            before_id: q.before_id,
            limit: q.limit.unwrap_or(50),
        })
        .await?;
    Ok(Json(HistoryPage { items }))
}
