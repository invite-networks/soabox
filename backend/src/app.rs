//! Router assembly.

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{HeaderValue, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use tower_http::services::{ServeDir, ServeFile};

use crate::config::Mode;
use crate::state::AppState;
use crate::{api, auth, http, v1};

/// Builds the full application router.
pub fn router(state: AppState) -> Router {
    let max_body = state.config.max_body_bytes;
    let static_dir = state.config.static_dir.clone();
    let mut v1_routes = v1::router();
    if state.config.mode == Mode::Dev {
        v1_routes = v1_routes.route("/openapi.json", get(openapi_json));
    }
    let spa = ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html")));

    Router::new()
        .route("/api", any(api::passthrough))
        .route("/api/{*rest}", any(api::passthrough))
        .route("/healthz", get(http::healthz))
        .route("/readyz", get(http::readyz))
        .route("/version", get(http::version))
        .route("/auth/config", get(auth::local::config))
        .route("/auth/login", post(auth::local::login))
        .route("/auth/logout", post(auth::local::logout))
        .route("/auth/oidc/start", get(auth::oidc::start))
        .route("/auth/oidc/callback", get(auth::oidc::callback))
        .nest("/v1", v1_routes)
        .fallback_service(spa)
        .layer(middleware::from_fn(cache_headers))
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

async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        crate::openapi::json(),
    )
}

/// Long caching for fingerprinted assets, no caching for everything else
/// served to the browser.
async fn cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let mut resp = next.run(req).await;
    if path.starts_with("/api") {
        return resp;
    }
    let value = if path.starts_with("/assets/") && resp.status().is_success() {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };
    resp.headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static(value));
    resp
}
