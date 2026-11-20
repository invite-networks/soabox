//! Sign-in, sessions, and the signed-in user extractor.

pub mod crypto;
pub mod local;
pub mod oidc;
pub mod provider;

use axum::extract::FromRequestParts;
use axum::http::Method;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Serialize;
use utoipa::ToSchema;

use crate::authz::Principal;
use crate::errors::ApiError;
use crate::http::ClientInfo;
use crate::settings::Settings;
use crate::state::AppState;
use crate::store::{User, UserRole, UserStatus, now};

/// Name of the session cookie. The `__Host-` prefix requires HTTPS.
pub fn session_cookie_name(state: &AppState) -> &'static str {
    if secure_cookies(state) {
        "__Host-soabox_session"
    } else {
        "soabox_session"
    }
}

/// Cookies are `Secure` whenever the public URL is HTTPS.
pub fn secure_cookies(state: &AppState) -> bool {
    state.config.public_url.scheme() == "https"
}

/// Builds the session cookie for a new session token.
pub fn session_cookie(state: &AppState, token: String) -> Cookie<'static> {
    Cookie::build((session_cookie_name(state), token))
        .path("/")
        .http_only(true)
        .secure(secure_cookies(state))
        .same_site(SameSite::Lax)
        .max_age(cookie_duration(state.config.session_max.as_secs()))
        .build()
}

/// A cookie that removes the session cookie.
pub fn clear_session_cookie(state: &AppState) -> Cookie<'static> {
    Cookie::build((session_cookie_name(state), ""))
        .path("/")
        .http_only(true)
        .secure(secure_cookies(state))
        .same_site(SameSite::Lax)
        .max_age(cookie_duration(0))
        .build()
}

fn cookie_duration(secs: u64) -> time::Duration {
    time::Duration::seconds(i64::try_from(secs).unwrap_or(i64::MAX))
}

/// What a user may do, for the UI.
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
pub struct Permissions {
    pub manage_users: bool,
    pub manage_settings: bool,
    pub manage_tsig: bool,
    pub view_server: bool,
    pub all_zones: bool,
    pub create_zones: bool,
    pub delete_zones: bool,
    pub view_history: bool,
    pub manage_dnssec: bool,
    pub manage_templates: bool,
    pub create_api_keys: bool,
}

impl Permissions {
    /// Computes permissions from role and settings.
    pub fn for_user(user: &User, s: &Settings) -> Self {
        let admin = user.role == UserRole::Admin;
        let staff = admin || user.role == UserRole::Operator;
        Self {
            manage_users: admin,
            manage_settings: admin,
            manage_tsig: admin,
            view_server: staff,
            all_zones: staff,
            create_zones: staff || s.user_can_create_zones,
            delete_zones: staff || s.user_can_delete_zones,
            view_history: staff || s.user_can_view_history,
            manage_dnssec: staff || s.user_can_manage_dnssec,
            manage_templates: staff,
            create_api_keys: admin || s.user_can_create_api_keys,
        }
    }
}

/// The signed-in user for a `/v1` request. Rejects missing or expired
/// sessions with `401` and unsafe methods without a valid CSRF token with
/// `403`.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: User,
    pub session_hash: String,
    pub csrf_token: String,
    pub client: ClientInfo,
}

impl AuthUser {
    /// Current settings.
    pub async fn settings(&self, state: &AppState) -> Result<Settings, ApiError> {
        Ok(Settings::load(&state.store).await?)
    }

    /// Permissions under current settings.
    pub async fn permissions(&self, state: &AppState) -> Result<Permissions, ApiError> {
        Ok(Permissions::for_user(
            &self.user,
            &self.settings(state).await?,
        ))
    }

    /// The authorization principal for this user.
    pub async fn principal(&self, state: &AppState) -> Result<Principal, ApiError> {
        let s = self.settings(state).await?;
        Ok(Principal::for_user(&state.store, &self.user, &s).await?)
    }

    /// Returns `403` unless `allowed`.
    pub fn require(allowed: bool) -> Result<(), ApiError> {
        if allowed {
            Ok(())
        } else {
            Err(ApiError::Forbidden(
                "you do not have permission for this".into(),
            ))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let client = parts
            .extensions
            .get::<ClientInfo>()
            .cloned()
            .ok_or_else(|| ApiError::internal(anyhow::anyhow!("client info missing")))?;
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(session_cookie_name(state))
            .map(|c| c.value().to_owned())
            .filter(|v| !v.is_empty())
            .ok_or(ApiError::Unauthenticated)?;
        let hash = crypto::token_digest(&token);
        let session = state
            .store
            .get_session(&hash)
            .await?
            .ok_or(ApiError::Unauthenticated)?;

        let at = now();
        let idle = i64::try_from(state.config.session_idle.as_secs()).unwrap_or(i64::MAX);
        if session.expires_at <= at
            || session.last_seen_at.saturating_add(idle) <= at
            || session.user.status != UserStatus::Active
        {
            state.store.delete_session(&hash).await?;
            return Err(ApiError::Unauthenticated);
        }

        let unsafe_method = !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS);
        if unsafe_method {
            let presented = parts
                .headers
                .get("x-csrf-token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            if !crypto::constant_eq_str(presented, &session.csrf_token) {
                return Err(ApiError::CsrfFailed);
            }
        }

        if at - session.last_seen_at >= 60 {
            state.store.touch_session(&hash, at).await?;
        }

        Ok(Self {
            user: session.user,
            session_hash: hash,
            csrf_token: session.csrf_token,
            client,
        })
    }
}

/// Creates a session for `user` and returns the cookie to set and the CSRF
/// token.
pub async fn start_session(
    state: &AppState,
    user: &User,
    client: &ClientInfo,
    user_agent: &str,
) -> Result<(Cookie<'static>, String), ApiError> {
    let token = crypto::random_token().map_err(ApiError::Internal)?;
    let csrf = crypto::random_token().map_err(ApiError::Internal)?;
    let max = i64::try_from(state.config.session_max.as_secs()).unwrap_or(i64::MAX);
    state
        .store
        .create_session(
            &crypto::token_digest(&token),
            user.id,
            &csrf,
            now().saturating_add(max),
            &client.ip.to_string(),
            &user_agent.chars().take(256).collect::<String>(),
        )
        .await?;
    state.store.touch_user_login(user.id).await?;
    Ok((session_cookie(state, token), csrf))
}

/// Ends the session named by the request's cookie, if any.
pub async fn end_session(state: &AppState, jar: &CookieJar) -> Result<(), ApiError> {
    if let Some(c) = jar.get(session_cookie_name(state)) {
        state
            .store
            .delete_session(&crypto::token_digest(c.value()))
            .await?;
    }
    Ok(())
}

/// Creates the bootstrap Admin from configuration when no active Admin exists.
pub async fn bootstrap_admin(state: &AppState) -> anyhow::Result<()> {
    let Some(b) = &state.config.bootstrap_admin else {
        return Ok(());
    };
    if state.store.count_active_admins().await? > 0 {
        return Ok(());
    }
    if state
        .store
        .find_user_by_username(&b.username)
        .await?
        .is_some()
    {
        tracing::warn!(username = %b.username, "bootstrap admin username exists but is not an active admin; not changing it");
        return Ok(());
    }
    anyhow::ensure!(
        b.password.expose().len() >= crypto::MIN_PASSWORD_LEN,
        "SOABOX_BOOTSTRAP_ADMIN_PASSWORD must be at least {} characters",
        crypto::MIN_PASSWORD_LEN
    );
    state
        .store
        .create_user(&crate::store::NewUser {
            username: b.username.clone(),
            display_name: b.username.clone(),
            email: String::new(),
            role: UserRole::Admin,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: Some(crypto::hash_password(b.password.expose())?),
            status: UserStatus::Active,
        })
        .await?;
    tracing::info!(username = %b.username, "created bootstrap admin");
    Ok(())
}
