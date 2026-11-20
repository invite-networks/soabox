//! Local sign-in, sign-out, and the sign-in page configuration.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{AuthUser, Permissions, crypto};
use crate::errors::{ApiError, ErrorBody};
use crate::http::ClientInfo;
use crate::settings::{LocalLogin, Settings};
use crate::state::AppState;
use crate::store::{User, UserRole, UserStatus};

/// What the sign-in page offers.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthConfig {
    pub local_enabled: bool,
    pub oidc_enabled: bool,
    pub oidc_label: String,
}

/// Credentials for local sign-in.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

/// The signed-in user, their CSRF token, and permissions.
#[derive(Debug, Serialize, ToSchema)]
pub struct Me {
    pub user: MeUser,
    pub csrf_token: String,
    pub permissions: Permissions,
}

/// Profile fields of the signed-in user.
#[derive(Debug, Serialize, ToSchema)]
pub struct MeUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub auth_source: String,
    pub totp_enabled: bool,
}

impl Me {
    /// Builds the response for a user.
    pub fn new(user: &User, csrf_token: String, settings: &Settings) -> Self {
        Self {
            permissions: Permissions::for_user(user, settings),
            user: MeUser {
                id: user.id,
                username: user.username.clone(),
                display_name: user.display_name.clone(),
                email: user.email.clone(),
                role: user.role.to_string(),
                auth_source: user.auth_source.clone(),
                totp_enabled: user.totp_enabled,
            },
            csrf_token,
        }
    }

    /// Builds the response for the current request's user.
    pub async fn current(state: &AppState, auth: &AuthUser) -> Result<Self, ApiError> {
        let s = auth.settings(state).await?;
        Ok(Self::new(&auth.user, auth.csrf_token.clone(), &s))
    }
}

/// `GET /auth/config`
#[utoipa::path(get, path = "/auth/config", tag = "auth", responses((status = 200, body = AuthConfig)))]
pub async fn config(State(state): State<AppState>) -> Result<Json<AuthConfig>, ApiError> {
    let s = Settings::load(&state.store).await?;
    let oidc = super::provider::active(&state).await?;
    Ok(Json(AuthConfig {
        local_enabled: oidc.is_none() || s.local_login_with_oidc == LocalLogin::Allow,
        oidc_enabled: oidc.is_some(),
        oidc_label: oidc.map(|o| o.button_label).unwrap_or_default(),
    }))
}

/// `POST /auth/login`
#[utoipa::path(post, path = "/auth/login", tag = "auth", request_body = LoginRequest,
    responses((status = 200, body = Me), (status = 401, body = ErrorBody), (status = 403, body = ErrorBody), (status = 429, body = ErrorBody)))]
pub async fn login(
    State(state): State<AppState>,
    Extension(client): Extension<ClientInfo>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let name = req.username.to_ascii_lowercase();
    let ip_key = format!("ip:{}", client.ip);
    let pair_key = format!("pair:{}:{name}", client.ip);
    let user_key = format!("user:{name}");
    let keys = [ip_key.as_str(), pair_key.as_str()];
    if state.logins.blocked(&keys) || state.login_users.blocked(&[user_key.as_str()]) {
        return Err(ApiError::RateLimited);
    }

    let found = state.store.find_user_by_username(&req.username).await?;
    let user = match found {
        Some(u) if u.is_local() => {
            let ok = u
                .password_hash
                .as_deref()
                .is_some_and(|h| crypto::verify_password(&req.password, h));
            if !ok {
                state.logins.fail(&keys);
                state.login_users.fail(&[user_key.as_str()]);
                return Err(ApiError::Unauthenticated);
            }
            u
        }
        _ => {
            crypto::dummy_verify(&req.password);
            state.logins.fail(&keys);
            return Err(ApiError::Unauthenticated);
        }
    };

    match user.status {
        UserStatus::Active => {}
        UserStatus::Pending => {
            return Err(ApiError::Forbidden(
                "this account is waiting for approval".into(),
            ));
        }
        UserStatus::Disabled => return Err(ApiError::Forbidden("this account is disabled".into())),
    }

    let settings = Settings::load(&state.store).await?;
    if settings.local_login_with_oidc == LocalLogin::Deny
        && user.role != UserRole::Admin
        && super::provider::active(&state).await?.is_some()
    {
        return Err(ApiError::Forbidden(
            "local sign-in is turned off, use single sign-on".into(),
        ));
    }

    if user.totp_enabled {
        let Some(code) = req.totp_code.as_deref().filter(|c| !c.is_empty()) else {
            return Err(ApiError::TotpRequired);
        };
        let secret = user
            .totp_secret_enc
            .as_deref()
            .and_then(|enc| crypto::decrypt(crypto::PURPOSE_TOTP, &state.secret_keys(), enc));
        let now = u64::try_from(crate::store::now()).unwrap_or_default();
        let last = u64::try_from(user.totp_last_step).unwrap_or_default();
        let Some(step) = secret.and_then(|s| crypto::totp_match(&s, code, now, last)) else {
            state.logins.fail(&keys);
            return Err(ApiError::Unauthenticated);
        };
        state
            .store
            .set_totp_last_step(user.id, i64::try_from(step).unwrap_or(i64::MAX))
            .await?;
    }

    state.logins.succeed(&[pair_key.as_str()]);
    super::end_session(&state, &jar).await?;
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let (cookie, csrf) = super::start_session(&state, &user, &client, ua).await?;
    tracing::info!(user = %user.username, client_ip = %client.ip, "signed in");
    Ok((jar.add(cookie), Json(Me::new(&user, csrf, &settings))))
}

/// `POST /auth/logout`
#[utoipa::path(post, path = "/auth/logout", tag = "auth", responses((status = 204)))]
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ApiError> {
    super::end_session(&state, &jar).await?;
    Ok((
        jar.add(super::clear_session_cookie(&state)),
        StatusCode::NO_CONTENT,
    ))
}
