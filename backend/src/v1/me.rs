//! The signed-in user's profile, password, and TOTP.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::no_content;
use crate::auth::AuthUser;
use crate::auth::crypto;
use crate::auth::local::Me;
use crate::errors::ApiError;
use crate::state::AppState;

/// `GET /v1/me`
#[utoipa::path(get, path = "/v1/me", tag = "me", responses((status = 200, body = Me)))]
pub async fn get_me(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Me>, ApiError> {
    Ok(Json(Me::current(&state, &auth).await?))
}

/// Profile fields a user may change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ProfileUpdate {
    pub display_name: String,
    pub email: String,
}

/// `PUT /v1/me`
#[utoipa::path(put, path = "/v1/me", tag = "me", request_body = ProfileUpdate, responses((status = 200, body = Me)))]
pub async fn update_me(
    State(state): State<AppState>,
    mut auth: AuthUser,
    Json(req): Json<ProfileUpdate>,
) -> Result<Json<Me>, ApiError> {
    validate_profile(&req.display_name, &req.email)?;
    let u = &auth.user;
    state
        .store
        .update_user(
            u.id,
            req.display_name.trim(),
            req.email.trim(),
            u.role,
            u.status,
        )
        .await?;
    auth.user.display_name = req.display_name.trim().to_owned();
    auth.user.email = req.email.trim().to_owned();
    Ok(Json(Me::current(&state, &auth).await?))
}

/// Checks display name and email lengths and shape.
pub fn validate_profile(display_name: &str, email: &str) -> Result<(), ApiError> {
    if display_name.chars().count() > 128 {
        return Err(ApiError::Validation("display name is too long".into()));
    }
    let email = email.trim();
    if email.len() > 254 || (!email.is_empty() && !email.contains('@')) {
        return Err(ApiError::Validation("email is not valid".into()));
    }
    Ok(())
}

/// A password change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PasswordChange {
    pub current_password: String,
    pub new_password: String,
}

/// Checks password length.
pub fn validate_password(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < crypto::MIN_PASSWORD_LEN {
        return Err(ApiError::Validation(format!(
            "password must be at least {} characters",
            crypto::MIN_PASSWORD_LEN
        )));
    }
    if p.len() > 1024 {
        return Err(ApiError::Validation("password is too long".into()));
    }
    Ok(())
}

/// `PUT /v1/me/password`
#[utoipa::path(put, path = "/v1/me/password", tag = "me", request_body = PasswordChange, responses((status = 204)))]
pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PasswordChange>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !auth.user.is_local() {
        return Err(ApiError::BadRequest(
            "single sign-on users have no password here".into(),
        ));
    }
    let ok = auth
        .user
        .password_hash
        .as_deref()
        .is_some_and(|h| crypto::verify_password(&req.current_password, h));
    let key = format!("acct:{}", auth.user.id);
    if state.logins.blocked(&[key.as_str()]) {
        return Err(ApiError::RateLimited);
    }
    if !ok {
        state.logins.fail(&[key.as_str()]);
        return Err(ApiError::Validation("current password is incorrect".into()));
    }
    validate_password(&req.new_password)?;
    let hash = crypto::hash_password(&req.new_password).map_err(ApiError::Internal)?;
    state.store.set_password_hash(auth.user.id, &hash).await?;
    state
        .store
        .delete_other_sessions(auth.user.id, &auth.session_hash)
        .await?;
    Ok(no_content())
}

/// A new TOTP secret to show the user before enabling it.
#[derive(Debug, Serialize, ToSchema)]
pub struct TotpSetup {
    pub secret: String,
    pub otpauth_url: String,
}

/// `POST /v1/me/totp/setup`
#[utoipa::path(post, path = "/v1/me/totp/setup", tag = "me", responses((status = 200, body = TotpSetup)))]
pub async fn totp_setup(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<TotpSetup>, ApiError> {
    if !auth.user.is_local() {
        return Err(ApiError::BadRequest(
            "use your identity provider's second factor".into(),
        ));
    }
    if auth.user.totp_enabled {
        return Err(ApiError::Conflict(
            "two-factor sign-in is already on".into(),
        ));
    }
    let secret = crypto::random_bytes::<20>().map_err(ApiError::Internal)?;
    let enc = crypto::encrypt(crypto::PURPOSE_TOTP, &state.config.secret_key, &secret)
        .map_err(ApiError::Internal)?;
    state
        .store
        .set_totp(auth.user.id, Some(&enc), false)
        .await?;
    Ok(Json(TotpSetup {
        secret: crypto::base32_encode(&secret),
        otpauth_url: crypto::otpauth_url(&secret, &auth.user.username),
    }))
}

/// A TOTP code.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TotpCode {
    pub code: String,
}

/// A TOTP code and the current password, to turn two-factor sign-in off.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TotpDisable {
    pub code: String,
    pub password: String,
}

async fn check_code(state: &AppState, auth: &AuthUser, code: &str) -> Result<(), ApiError> {
    let key = format!("acct:{}", auth.user.id);
    if state.logins.blocked(&[key.as_str()]) {
        return Err(ApiError::RateLimited);
    }
    let secret = auth
        .user
        .totp_secret_enc
        .as_deref()
        .and_then(|enc| crypto::decrypt(crypto::PURPOSE_TOTP, &state.secret_keys(), enc))
        .ok_or_else(|| ApiError::BadRequest("start two-factor setup first".into()))?;
    let now = u64::try_from(crate::store::now()).unwrap_or_default();
    let last = u64::try_from(auth.user.totp_last_step).unwrap_or_default();
    let consumed = match crypto::totp_match(&secret, code, now, last) {
        Some(step) => {
            state
                .store
                .consume_totp_step(auth.user.id, i64::try_from(step).unwrap_or(i64::MAX))
                .await?
        }
        None => false,
    };
    if consumed {
        Ok(())
    } else {
        state.logins.fail(&[key.as_str()]);
        Err(ApiError::Validation("the code is not correct".into()))
    }
}

/// `POST /v1/me/totp/enable`
#[utoipa::path(post, path = "/v1/me/totp/enable", tag = "me", request_body = TotpCode, responses((status = 204)))]
pub async fn totp_enable(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TotpCode>,
) -> Result<axum::http::StatusCode, ApiError> {
    check_code(&state, &auth, &req.code).await?;
    state
        .store
        .set_totp(auth.user.id, auth.user.totp_secret_enc.as_deref(), true)
        .await?;
    Ok(no_content())
}

/// `DELETE /v1/me/totp`
#[utoipa::path(delete, path = "/v1/me/totp", tag = "me", request_body = TotpDisable, responses((status = 204)))]
pub async fn totp_disable(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TotpDisable>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !auth.user.totp_enabled {
        return Err(ApiError::BadRequest("two-factor sign-in is not on".into()));
    }
    let key = format!("acct:{}", auth.user.id);
    let password_ok = auth
        .user
        .password_hash
        .as_deref()
        .is_some_and(|h| crypto::verify_password(&req.password, h));
    if !password_ok {
        state.logins.fail(&[key.as_str()]);
        return Err(ApiError::Validation("current password is incorrect".into()));
    }
    check_code(&state, &auth, &req.code).await?;
    state.store.set_totp(auth.user.id, None, false).await?;
    Ok(no_content())
}
