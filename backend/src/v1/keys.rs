//! API key management for the UI.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use super::{no_content, record};
use crate::auth::AuthUser;
use crate::errors::ApiError;
use crate::keys;
use crate::pdns::ZoneName;
use crate::state::AppState;
use crate::store::{ApiKeyRecord, NewApiKey, Role, UserRole};

/// Key fields from the UI.
#[derive(Debug, Deserialize, ToSchema)]
pub struct KeyInput {
    #[serde(default)]
    pub description: String,
    #[schema(value_type = String)]
    pub role: Role,
    #[serde(default)]
    pub zones: Vec<String>,
    #[serde(default)]
    pub account_ids: Vec<i64>,
    pub create_account_id: Option<i64>,
    #[serde(default)]
    pub networks: Vec<String>,
    pub expires_at: Option<i64>,
}

/// A created key with its secret, shown once.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedKey {
    #[schema(value_type = Object)]
    pub key: ApiKeyRecord,
    pub secret: String,
}

fn is_admin(auth: &AuthUser) -> bool {
    auth.user.role == UserRole::Admin
}

/// Validates input and, for non-admins, limits it to what the user can reach.
async fn checked(
    state: &AppState,
    auth: &AuthUser,
    input: KeyInput,
) -> Result<NewApiKey, ApiError> {
    let perms = auth.permissions(state).await?;
    AuthUser::require(perms.create_api_keys)?;
    if input.description.chars().count() > 200 {
        return Err(ApiError::Validation("description is too long".into()));
    }
    let zones = input
        .zones
        .iter()
        .map(|z| {
            ZoneName::from_name(z)
                .map(|z| z.to_string())
                .ok_or_else(|| ApiError::Validation(format!("invalid zone {z}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for n in &input.networks {
        if n.parse::<ipnet::IpNet>().is_err() && n.parse::<std::net::IpAddr>().is_err() {
            return Err(ApiError::Validation(format!("invalid network {n}")));
        }
    }
    for a in input
        .account_ids
        .iter()
        .chain(input.create_account_id.iter())
    {
        if state.store.get_account(*a).await?.is_none() {
            return Err(ApiError::Validation(format!("account {a} not found")));
        }
    }
    if let Some(e) = input.expires_at
        && e <= crate::store::now()
    {
        return Err(ApiError::Validation("expiry must be in the future".into()));
    }
    let mut role = input.role;
    if !is_admin(auth) {
        role = Role::Scoped;
        let p = auth.principal(state).await?;
        if !p.all_zones() {
            if let Some(z) = zones.iter().find(|z| !p.zones.contains(*z)) {
                return Err(ApiError::Forbidden(format!(
                    "you cannot grant access to {z}"
                )));
            }
            let mine = state.store.user_account_ids(auth.user.id).await?;
            if let Some(a) = input
                .account_ids
                .iter()
                .chain(input.create_account_id.iter())
                .find(|a| !mine.contains(a))
            {
                return Err(ApiError::Forbidden(format!(
                    "you are not a member of account {a}"
                )));
            }
        }
        if input.create_account_id.is_some() && !perms.create_zones {
            return Err(ApiError::Forbidden("you cannot create zones".into()));
        }
    }
    Ok(NewApiKey {
        key_hash: String::new(),
        hint: String::new(),
        description: input.description.trim().to_owned(),
        role: Some(role),
        create_account_id: input.create_account_id,
        owner_user_id: Some(auth.user.id),
        expires_at: input.expires_at,
        zones,
        account_ids: input.account_ids,
        networks: input.networks,
    })
}

async fn owned_key(state: &AppState, auth: &AuthUser, id: i64) -> Result<ApiKeyRecord, ApiError> {
    AuthUser::require(auth.permissions(state).await?.create_api_keys)?;
    let k = state
        .store
        .get_api_key(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("API key".into()))?;
    if is_admin(auth) || k.owner_user_id == Some(auth.user.id) {
        Ok(k)
    } else {
        Err(ApiError::NotFound("API key".into()))
    }
}

/// `GET /v1/api-keys`
#[utoipa::path(get, path = "/v1/api-keys", tag = "api-keys", responses((status = 200, body = Vec<Object>)))]
pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<ApiKeyRecord>>, ApiError> {
    AuthUser::require(auth.permissions(&state).await?.create_api_keys)?;
    let owner = (!is_admin(&auth)).then_some(auth.user.id);
    Ok(Json(state.store.list_api_keys(owner).await?))
}

/// `POST /v1/api-keys`
#[utoipa::path(post, path = "/v1/api-keys", tag = "api-keys", request_body = KeyInput, responses((status = 201, body = CreatedKey)))]
pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<KeyInput>,
) -> Result<(StatusCode, Json<CreatedKey>), ApiError> {
    let mut nk = checked(&state, &auth, input).await?;
    let g = keys::generate().map_err(ApiError::internal)?;
    nk.key_hash = g.hash;
    nk.hint = g.hint;
    let id = state.store.insert_api_key(&nk).await?;
    let key = state
        .store
        .get_api_key(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("API key".into()))?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "api_key.create",
        None,
        Ok(()),
        None,
        Some(json!({"id": id, "role": key.role, "description": key.description})),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(CreatedKey {
            key,
            secret: g.secret,
        }),
    ))
}

/// `PUT /v1/api-keys/{id}`
#[utoipa::path(put, path = "/v1/api-keys/{id}", tag = "api-keys", request_body = KeyInput, responses((status = 200, body = Object)))]
pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(input): Json<KeyInput>,
) -> Result<Json<ApiKeyRecord>, ApiError> {
    owned_key(&state, &auth, id).await?;
    let nk = checked(&state, &auth, input).await?;
    state.store.update_api_key(id, &nk).await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "api_key.update",
        None,
        Ok(()),
        None,
        Some(json!({"id": id})),
    )
    .await;
    Ok(Json(
        state
            .store
            .get_api_key(id)
            .await?
            .ok_or_else(|| ApiError::NotFound("API key".into()))?,
    ))
}

async fn set_disabled(
    state: AppState,
    auth: AuthUser,
    id: i64,
    disabled: bool,
) -> Result<StatusCode, ApiError> {
    owned_key(&state, &auth, id).await?;
    state.store.set_api_key_disabled(id, disabled).await?;
    let p = auth.principal(&state).await?;
    let op = if disabled {
        "api_key.disable"
    } else {
        "api_key.enable"
    };
    record(
        &state,
        &auth,
        &p,
        op,
        None,
        Ok(()),
        None,
        Some(json!({"id": id})),
    )
    .await;
    Ok(no_content())
}

/// `POST /v1/api-keys/{id}/disable`
#[utoipa::path(post, path = "/v1/api-keys/{id}/disable", tag = "api-keys", responses((status = 204)))]
pub async fn disable(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    set_disabled(state, auth, id, true).await
}

/// `POST /v1/api-keys/{id}/enable`
#[utoipa::path(post, path = "/v1/api-keys/{id}/enable", tag = "api-keys", responses((status = 204)))]
pub async fn enable(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    set_disabled(state, auth, id, false).await
}

/// `DELETE /v1/api-keys/{id}`
#[utoipa::path(delete, path = "/v1/api-keys/{id}", tag = "api-keys", responses((status = 204)))]
pub async fn remove(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    owned_key(&state, &auth, id).await?;
    state.store.delete_api_key(id).await?;
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "api_key.delete",
        None,
        Ok(()),
        None,
        Some(json!({"id": id})),
    )
    .await;
    Ok(no_content())
}
