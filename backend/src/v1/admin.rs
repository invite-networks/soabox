//! Administration: users, accounts, settings, and TSIG keys.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;

use super::me::{validate_password, validate_profile};
use super::{no_content, record};
use crate::audit;
use crate::auth::{AuthUser, crypto};
use crate::errors::ApiError;
use crate::pdns::ZoneName;
use crate::settings::{Settings, SettingsPatch};
use crate::state::AppState;
use crate::store::{NewUser, User, UserRole, UserStatus};

async fn require_admin(state: &AppState, auth: &AuthUser) -> Result<(), ApiError> {
    AuthUser::require(auth.permissions(state).await?.manage_users)
}

/// A user with their access.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserView {
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub user: User,
    pub zones: Vec<String>,
    pub account_ids: Vec<i64>,
}

async fn view(state: &AppState, user: User) -> Result<UserView, ApiError> {
    Ok(UserView {
        zones: state.store.user_zones(user.id).await?,
        account_ids: state.store.user_account_ids(user.id).await?,
        user,
    })
}

/// `GET /v1/users`
#[utoipa::path(get, path = "/v1/users", tag = "admin", responses((status = 200, body = Vec<UserView>)))]
pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<UserView>>, ApiError> {
    require_admin(&state, &auth).await?;
    let mut out = Vec::new();
    for u in state.store.list_users().await? {
        out.push(view(&state, u).await?);
    }
    Ok(Json(out))
}

/// A new local user.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUser {
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[schema(value_type = String)]
    pub role: UserRole,
    pub password: String,
}

fn valid_username(u: &str) -> Result<(), ApiError> {
    if !u.is_empty()
        && u.len() <= 64
        && u.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-@".contains(&b))
    {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "username may use letters, digits, and . _ - @ (up to 64)".into(),
        ))
    }
}

/// `POST /v1/users`
#[utoipa::path(post, path = "/v1/users", tag = "admin", request_body = CreateUser, responses((status = 201, body = UserView)))]
pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateUser>,
) -> Result<(StatusCode, Json<UserView>), ApiError> {
    require_admin(&state, &auth).await?;
    let username = req.username.trim().to_ascii_lowercase();
    valid_username(&username)?;
    validate_profile(&req.display_name, &req.email)?;
    validate_password(&req.password)?;
    if state
        .store
        .find_user_by_username(&username)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("that username is taken".into()));
    }
    let id = state
        .store
        .create_user(&NewUser {
            username: username.clone(),
            display_name: if req.display_name.trim().is_empty() {
                username.clone()
            } else {
                req.display_name.trim().to_owned()
            },
            email: req.email.trim().to_owned(),
            role: req.role,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: Some(crypto::hash_password(&req.password).map_err(ApiError::Internal)?),
            status: UserStatus::Active,
        })
        .await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "user.create",
        None,
        Ok(()),
        None,
        Some(json!({"username": username, "role": req.role})),
    )
    .await;
    let u = state
        .store
        .get_user(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("user".into()))?;
    Ok((StatusCode::CREATED, Json(view(&state, u).await?)))
}

/// Changes to a user.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUser {
    pub display_name: String,
    pub email: String,
    #[schema(value_type = String)]
    pub role: UserRole,
    #[schema(value_type = String)]
    pub status: UserStatus,
    #[serde(default)]
    pub zones: Vec<String>,
    #[serde(default)]
    pub account_ids: Vec<i64>,
}

/// `PUT /v1/users/{id}`
#[utoipa::path(put, path = "/v1/users/{id}", tag = "admin", request_body = UpdateUser, responses((status = 200, body = UserView)))]
pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateUser>,
) -> Result<Json<UserView>, ApiError> {
    require_admin(&state, &auth).await?;
    validate_profile(&req.display_name, &req.email)?;
    let user = state
        .store
        .get_user(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("user".into()))?;
    let demoting_admin = user.role == UserRole::Admin
        && user.status == UserStatus::Active
        && (req.role != UserRole::Admin || req.status != UserStatus::Active);
    if demoting_admin && state.store.count_active_admins().await? <= 1 {
        return Err(ApiError::Conflict(
            "at least one active Admin must remain".into(),
        ));
    }
    let zones = req
        .zones
        .iter()
        .map(|z| {
            ZoneName::from_name(z)
                .map(|z| z.to_string())
                .ok_or_else(|| ApiError::Validation(format!("invalid zone {z}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for a in &req.account_ids {
        if state.store.get_account(*a).await?.is_none() {
            return Err(ApiError::Validation(format!("account {a} not found")));
        }
    }
    state
        .store
        .update_user(
            id,
            req.display_name.trim(),
            req.email.trim(),
            req.role,
            req.status,
        )
        .await?;
    state
        .store
        .set_user_access(id, &zones, &req.account_ids)
        .await?;
    if user.role != req.role || req.status != UserStatus::Active {
        state.store.delete_user_sessions(id).await?;
    }
    let p = auth.principal(&state).await?;
    record(&state, &auth, &p, "user.update", None, Ok(()), None, Some(json!({
        "username": user.username, "role": req.role, "status": req.status, "zones": zones, "account_ids": req.account_ids,
    }))).await;
    let u = state
        .store
        .get_user(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("user".into()))?;
    Ok(Json(view(&state, u).await?))
}

/// A new password for a user.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPassword {
    pub password: String,
}

/// `POST /v1/users/{id}/password`
#[utoipa::path(post, path = "/v1/users/{id}/password", tag = "admin", request_body = SetPassword, responses((status = 204)))]
pub async fn set_user_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<SetPassword>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &auth).await?;
    let user = state
        .store
        .get_user(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("user".into()))?;
    if !user.is_local() {
        return Err(ApiError::BadRequest(
            "single sign-on users have no password here".into(),
        ));
    }
    validate_password(&req.password)?;
    state
        .store
        .set_password_hash(
            id,
            &crypto::hash_password(&req.password).map_err(ApiError::Internal)?,
        )
        .await?;
    state.store.delete_user_sessions(id).await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "user.password_reset",
        None,
        Ok(()),
        None,
        Some(json!({"username": user.username})),
    )
    .await;
    Ok(no_content())
}

/// `POST /v1/users/{id}/revoke-sessions`
#[utoipa::path(post, path = "/v1/users/{id}/revoke-sessions", tag = "admin", responses((status = 204)))]
pub async fn revoke_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &auth).await?;
    state.store.delete_user_sessions(id).await?;
    Ok(no_content())
}

/// `DELETE /v1/users/{id}/totp`
#[utoipa::path(delete, path = "/v1/users/{id}/totp", tag = "admin", responses((status = 204)))]
pub async fn reset_totp(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &auth).await?;
    if !state.store.set_totp(id, None, false).await? {
        return Err(ApiError::NotFound("user".into()));
    }
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "user.totp_reset",
        None,
        Ok(()),
        None,
        Some(json!({"user_id": id})),
    )
    .await;
    Ok(no_content())
}

/// `DELETE /v1/users/{id}`
#[utoipa::path(delete, path = "/v1/users/{id}", tag = "admin", responses((status = 204)))]
pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &auth).await?;
    if id == auth.user.id {
        return Err(ApiError::Conflict("you cannot delete yourself".into()));
    }
    let user = state
        .store
        .get_user(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("user".into()))?;
    if user.role == UserRole::Admin
        && user.status == UserStatus::Active
        && state.store.count_active_admins().await? <= 1
    {
        return Err(ApiError::Conflict(
            "at least one active Admin must remain".into(),
        ));
    }
    state.store.disable_user_api_keys(id).await?;
    state.store.delete_user(id).await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "user.delete",
        None,
        Ok(()),
        None,
        Some(json!({"username": user.username})),
    )
    .await;
    Ok(no_content())
}

/// An account with its zones and members.
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountView {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub zones: Vec<String>,
    pub user_ids: Vec<i64>,
}

async fn account_view(state: &AppState, a: crate::store::Account) -> Result<AccountView, ApiError> {
    Ok(AccountView {
        user_ids: state.store.account_user_ids(a.id).await?,
        id: a.id,
        name: a.name,
        description: a.description,
        zones: a.zones,
    })
}

/// `GET /v1/accounts`
#[utoipa::path(get, path = "/v1/accounts", tag = "admin", responses((status = 200, body = Vec<AccountView>)))]
pub async fn list_accounts(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<AccountView>>, ApiError> {
    let staff = auth.permissions(&state).await?.all_zones;
    let mine = state.store.user_account_ids(auth.user.id).await?;
    let mut out = Vec::new();
    for a in state.store.list_accounts().await? {
        if staff || mine.contains(&a.id) {
            out.push(account_view(&state, a).await?);
        }
    }
    Ok(Json(out))
}

/// Account fields.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AccountInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub zones: Option<Vec<String>>,
    pub user_ids: Option<Vec<i64>>,
}

fn valid_account_name(n: &str) -> Result<(), ApiError> {
    if !n.is_empty()
        && n.len() <= 64
        && n.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._- ".contains(&b))
    {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "account names may use letters, digits, spaces, and . _ - (up to 64)".into(),
        ))
    }
}

/// `POST /v1/accounts`
#[utoipa::path(post, path = "/v1/accounts", tag = "admin", request_body = AccountInput, responses((status = 201, body = AccountView)))]
pub async fn create_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<AccountInput>,
) -> Result<(StatusCode, Json<AccountView>), ApiError> {
    require_admin(&state, &auth).await?;
    valid_account_name(req.name.trim())?;
    if state
        .store
        .find_account_by_name(req.name.trim())
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(
            "an account with that name exists".into(),
        ));
    }
    let id = state
        .store
        .create_account(req.name.trim(), req.description.trim())
        .await?;
    apply_account_members(
        &state,
        id,
        req.name.trim(),
        req.zones.as_deref(),
        req.user_ids.as_deref(),
    )
    .await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "account.create",
        None,
        Ok(()),
        None,
        Some(json!({"name": req.name})),
    )
    .await;
    let a = state
        .store
        .get_account(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("account".into()))?;
    Ok((StatusCode::CREATED, Json(account_view(&state, a).await?)))
}

async fn apply_account_members(
    state: &AppState,
    id: i64,
    name: &str,
    zones: Option<&[String]>,
    user_ids: Option<&[i64]>,
) -> Result<(), ApiError> {
    if let Some(zones) = zones {
        let zones = zones
            .iter()
            .map(|z| {
                ZoneName::from_name(z)
                    .map(|z| z.to_string())
                    .ok_or_else(|| ApiError::Validation(format!("invalid zone {z}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let old = state.store.account_zones(id).await?;
        state.store.set_account_zones(id, &zones).await?;
        for z in zones.iter().filter(|z| !old.contains(z)) {
            sync_pdns_account(state, z, name).await;
        }
        for z in old.iter().filter(|z| !zones.contains(z)) {
            sync_pdns_account(state, z, "").await;
        }
    }
    if let Some(ids) = user_ids {
        state.store.set_account_users(id, ids).await?;
    }
    Ok(())
}

/// Keeps the PowerDNS `account` field in step. Best effort.
async fn sync_pdns_account(state: &AppState, zone: &str, account: &str) {
    let Some(z) = ZoneName::from_name(zone) else {
        return;
    };
    let body = json!({"account": account});
    if let Err(e) = state
        .pdns
        .send(
            Method::PUT,
            &format!("/zones/{}", z.to_url_id()),
            Some(&body),
        )
        .await
    {
        tracing::warn!(zone = %zone, error = %e, "could not update PowerDNS account field");
    }
}

/// `PUT /v1/accounts/{id}`
#[utoipa::path(put, path = "/v1/accounts/{id}", tag = "admin", request_body = AccountInput, responses((status = 200, body = AccountView)))]
pub async fn update_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<AccountInput>,
) -> Result<Json<AccountView>, ApiError> {
    require_admin(&state, &auth).await?;
    valid_account_name(req.name.trim())?;
    let existing = state
        .store
        .get_account(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("account".into()))?;
    if let Some(other) = state.store.find_account_by_name(req.name.trim()).await?
        && other.id != id
    {
        return Err(ApiError::Conflict(
            "an account with that name exists".into(),
        ));
    }
    state
        .store
        .update_account(id, req.name.trim(), req.description.trim())
        .await?;
    apply_account_members(
        &state,
        id,
        req.name.trim(),
        req.zones.as_deref(),
        req.user_ids.as_deref(),
    )
    .await?;
    if existing.name != req.name.trim() && req.zones.is_none() {
        for z in &existing.zones {
            sync_pdns_account(&state, z, req.name.trim()).await;
        }
    }
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "account.update",
        None,
        Ok(()),
        None,
        Some(json!({"name": req.name, "zones": req.zones, "user_ids": req.user_ids})),
    )
    .await;
    let a = state
        .store
        .get_account(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("account".into()))?;
    Ok(Json(account_view(&state, a).await?))
}

/// `DELETE /v1/accounts/{id}`
#[utoipa::path(delete, path = "/v1/accounts/{id}", tag = "admin", responses((status = 204)))]
pub async fn delete_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &auth).await?;
    let a = state
        .store
        .get_account(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("account".into()))?;
    for z in &a.zones {
        sync_pdns_account(&state, z, "").await;
    }
    state.store.delete_account(id).await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "account.delete",
        None,
        Ok(()),
        None,
        Some(json!({"name": a.name})),
    )
    .await;
    Ok(no_content())
}

/// `GET /v1/settings`
#[utoipa::path(get, path = "/v1/settings", tag = "admin", responses((status = 200, body = Settings)))]
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Settings>, ApiError> {
    AuthUser::require(auth.permissions(&state).await?.manage_settings)?;
    Ok(Json(Settings::load(&state.store).await?))
}

/// `PUT /v1/settings`
#[utoipa::path(put, path = "/v1/settings", tag = "admin", request_body = SettingsPatch, responses((status = 200, body = Settings)))]
pub async fn put_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<SettingsPatch>,
) -> Result<Json<Settings>, ApiError> {
    AuthUser::require(auth.permissions(&state).await?.manage_settings)?;
    let s = Settings::apply(&state.store, req)
        .await?
        .map_err(ApiError::Validation)?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "settings.update",
        None,
        Ok(()),
        None,
        serde_json::to_value(&s).ok(),
    )
    .await;
    Ok(Json(s))
}

async fn require_tsig(state: &AppState, auth: &AuthUser) -> Result<(), ApiError> {
    AuthUser::require(auth.permissions(state).await?.manage_tsig)
}

fn tsig_id(id: &str) -> Result<String, ApiError> {
    if !id.is_empty()
        && id.len() <= 255
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-=".contains(&b))
    {
        Ok(id.to_owned())
    } else {
        Err(ApiError::NotFound("TSIG key".into()))
    }
}

/// `GET /v1/tsigkeys`
#[utoipa::path(get, path = "/v1/tsigkeys", tag = "admin", responses((status = 200, body = Object)))]
pub async fn list_tsig(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, ApiError> {
    require_tsig(&state, &auth).await?;
    let mut v: Value = state.pdns.get("/tsigkeys").await?;
    if let Some(items) = v.as_array_mut() {
        for i in items {
            if let Some(o) = i.as_object_mut() {
                o.remove("key");
            }
        }
    }
    Ok(Json(v))
}

/// A new TSIG key.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct NewTsig {
    pub name: String,
    pub algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// `POST /v1/tsigkeys`
#[utoipa::path(post, path = "/v1/tsigkeys", tag = "admin", request_body = NewTsig, responses((status = 201, body = Object)))]
pub async fn create_tsig(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<NewTsig>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    require_tsig(&state, &auth).await?;
    let body = serde_json::to_value(&req).unwrap_or_default();
    let p = auth.principal(&state).await?;
    let result = state
        .pdns
        .send(Method::POST, "/tsigkeys", Some(&body))
        .await
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "tsigkey.create",
        None,
        result.as_ref().map(|_| ()),
        None,
        Some(audit::redact(&body)),
    )
    .await;
    Ok((StatusCode::CREATED, Json(result?.unwrap_or(Value::Null))))
}

/// `GET /v1/tsigkeys/{id}`
#[utoipa::path(get, path = "/v1/tsigkeys/{id}", tag = "admin", responses((status = 200, body = Object)))]
pub async fn get_tsig(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_tsig(&state, &auth).await?;
    Ok(Json(
        state
            .pdns
            .get(&format!("/tsigkeys/{}", tsig_id(&id)?))
            .await?,
    ))
}

/// TSIG key changes.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateTsig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// `PUT /v1/tsigkeys/{id}`
#[utoipa::path(put, path = "/v1/tsigkeys/{id}", tag = "admin", request_body = UpdateTsig, responses((status = 200, body = Object)))]
pub async fn update_tsig(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateTsig>,
) -> Result<Json<Value>, ApiError> {
    require_tsig(&state, &auth).await?;
    let id = tsig_id(&id)?;
    let body = serde_json::to_value(&req).unwrap_or_default();
    let p = auth.principal(&state).await?;
    let result = state
        .pdns
        .send(Method::PUT, &format!("/tsigkeys/{id}"), Some(&body))
        .await
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "tsigkey.update",
        None,
        result.as_ref().map(|_| ()),
        None,
        Some(audit::redact(
            &json!({"id": id, "algorithm": req.algorithm, "key": req.key}),
        )),
    )
    .await;
    Ok(Json(result?.unwrap_or(Value::Null)))
}

/// `DELETE /v1/tsigkeys/{id}`
#[utoipa::path(delete, path = "/v1/tsigkeys/{id}", tag = "admin", responses((status = 204)))]
pub async fn delete_tsig(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_tsig(&state, &auth).await?;
    let id = tsig_id(&id)?;
    let p = auth.principal(&state).await?;
    let result = state
        .pdns
        .send(Method::DELETE, &format!("/tsigkeys/{id}"), None)
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "tsigkey.delete",
        None,
        result.as_ref().map(|_| ()),
        None,
        Some(json!({"id": id})),
    )
    .await;
    result.map(|()| no_content())
}
