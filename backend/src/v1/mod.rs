//! The `/v1` API used by the browser UI. See `docs/spec/v1-api.md`.

pub mod admin;
pub mod keys;
pub mod me;
pub mod server;
pub mod templates;
pub mod zones;

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};

use crate::audit::{self, AuditEvent};
use crate::auth::AuthUser;
use crate::authz::Principal;
use crate::errors::ApiError;
use crate::pdns::ZoneName;
use crate::state::AppState;

/// Builds the `/v1` router. Unknown paths answer with a JSON `404`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me::get_me).put(me::update_me))
        .route("/me/password", put(me::change_password))
        .route("/me/totp/setup", post(me::totp_setup))
        .route("/me/totp/enable", post(me::totp_enable))
        .route("/me/totp", delete(me::totp_disable))
        .route("/dashboard", get(server::dashboard))
        .route("/server", get(server::server))
        .route("/server/statistics", get(server::statistics))
        .route("/server/config", get(server::config))
        .route("/history", get(server::history))
        .route("/zones", get(zones::list).post(zones::create))
        .route(
            "/zones/{id}",
            get(zones::get).put(zones::update).delete(zones::remove),
        )
        .route(
            "/zones/{id}/rrsets",
            axum::routing::patch(zones::patch_rrsets),
        )
        .route("/zones/{id}/notify", post(zones::notify))
        .route("/zones/{id}/rectify", post(zones::rectify))
        .route("/zones/{id}/axfr-retrieve", post(zones::axfr_retrieve))
        .route("/zones/{id}/export", get(zones::export))
        .route("/zones/{id}/history", get(zones::history))
        .route("/zones/{id}/dnssec", post(zones::set_dnssec))
        .route(
            "/zones/{id}/cryptokeys",
            get(zones::list_cryptokeys).post(zones::create_cryptokey),
        )
        .route(
            "/zones/{id}/cryptokeys/{key_id}",
            put(zones::update_cryptokey).delete(zones::delete_cryptokey),
        )
        .route("/zones/{id}/metadata", get(zones::list_metadata))
        .route(
            "/zones/{id}/metadata/{kind}",
            put(zones::put_metadata).delete(zones::delete_metadata),
        )
        .route("/templates", get(templates::list).post(templates::create))
        .route(
            "/templates/{id}",
            put(templates::update).delete(templates::remove),
        )
        .route("/users", get(admin::list_users).post(admin::create_user))
        .route(
            "/users/{id}",
            put(admin::update_user).delete(admin::delete_user),
        )
        .route("/users/{id}/password", post(admin::set_user_password))
        .route("/users/{id}/revoke-sessions", post(admin::revoke_sessions))
        .route("/users/{id}/totp", delete(admin::reset_totp))
        .route(
            "/accounts",
            get(admin::list_accounts).post(admin::create_account),
        )
        .route(
            "/accounts/{id}",
            put(admin::update_account).delete(admin::delete_account),
        )
        .route(
            "/settings",
            get(admin::get_settings).put(admin::put_settings),
        )
        .route(
            "/settings/oidc",
            get(admin::get_oidc_settings).put(admin::put_oidc_settings),
        )
        .route("/tsigkeys", get(admin::list_tsig).post(admin::create_tsig))
        .route(
            "/tsigkeys/{id}",
            get(admin::get_tsig)
                .put(admin::update_tsig)
                .delete(admin::delete_tsig),
        )
        .route("/api-keys", get(keys::list).post(keys::create))
        .route("/api-keys/{id}", put(keys::update).delete(keys::remove))
        .route("/api-keys/{id}/disable", post(keys::disable))
        .route("/api-keys/{id}/enable", post(keys::enable))
        .fallback(not_found)
}

async fn not_found() -> impl IntoResponse {
    ApiError::NotFound("endpoint".into())
}

/// `204 No Content`.
pub fn no_content() -> StatusCode {
    StatusCode::NO_CONTENT
}

/// Parses a zone id from a path and checks the caller can reach it.
/// Zones out of scope answer `404`, like missing zones.
pub fn zone_in_scope(p: &Principal, id: &str) -> Result<ZoneName, ApiError> {
    let (zone, _) = ZoneName::from_url_id(id).ok_or_else(|| ApiError::NotFound("zone".into()))?;
    if p.can_access_zone(&zone) {
        Ok(zone)
    } else {
        Err(ApiError::NotFound("zone".into()))
    }
}

/// Records a UI action in history.
#[allow(clippy::too_many_arguments)]
pub async fn record(
    state: &AppState,
    auth: &AuthUser,
    principal: &Principal,
    operation: &str,
    zone: Option<&ZoneName>,
    result: Result<(), &ApiError>,
    diff: Option<serde_json::Value>,
    detail: Option<serde_json::Value>,
) {
    audit::record(
        &state.store,
        AuditEvent {
            principal,
            client_ip: auth.client.ip.to_string(),
            operation: operation.to_owned(),
            zone: zone.map(ToString::to_string),
            upstream_status: None,
            success: result.is_ok(),
            diff: if result.is_ok() { diff } else { None },
            detail: match result {
                Ok(()) => detail,
                Err(e) => Some(serde_json::json!({ "request": detail, "error": e.to_string() })),
            },
        },
    )
    .await;
}
