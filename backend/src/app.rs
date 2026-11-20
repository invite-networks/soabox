//! Router assembly.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{any, get};

use crate::state::AppState;
use crate::{api, http};

/// Builds the full application router.
pub fn router(state: AppState) -> Router {
    let max_body = state.config.max_body_bytes;
    Router::new()
        .route("/api", any(api::passthrough))
        .route("/api/{*rest}", any(api::passthrough))
        .route("/healthz", get(http::healthz))
        .route("/readyz", get(http::readyz))
        .route("/version", get(http::version))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            http::security_headers,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            http::client_info,
        ))
        .layer(DefaultBodyLimit::max(max_body))
        .with_state(state)
}
