//! Zone templates.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;

use super::{no_content, record};
use crate::auth::AuthUser;
use crate::errors::ApiError;
use crate::settings::KNOWN_TYPES;
use crate::state::AppState;
use crate::store::{Template, TemplateRecord};

/// Template fields.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TemplateInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub records: Vec<TemplateRecord>,
}

fn validate(t: &TemplateInput) -> Result<Vec<TemplateRecord>, ApiError> {
    let name = t.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::Validation(
            "template name must be 1 to 64 characters".into(),
        ));
    }
    if t.records.len() > 500 {
        return Err(ApiError::Validation(
            "a template can hold up to 500 records".into(),
        ));
    }
    t.records
        .iter()
        .map(|r| {
            let rtype = r.rtype.trim().to_ascii_uppercase();
            if !KNOWN_TYPES.contains(&rtype.as_str()) || rtype == "SOA" {
                return Err(ApiError::Validation(format!(
                    "record type {rtype} is not allowed in templates"
                )));
            }
            if !(0..=2_147_483_647).contains(&r.ttl) {
                return Err(ApiError::Validation("TTL is out of range".into()));
            }
            let n = r.name.trim();
            if n.is_empty() || n.len() > 253 || n.contains(char::is_whitespace) {
                return Err(ApiError::Validation(format!(
                    "record name {n:?} is not valid"
                )));
            }
            if r.content.trim().is_empty() || r.content.len() > 65_535 {
                return Err(ApiError::Validation(
                    "record content cannot be empty".into(),
                ));
            }
            Ok(TemplateRecord {
                name: n.to_owned(),
                rtype,
                ttl: r.ttl,
                content: r.content.trim().to_owned(),
            })
        })
        .collect()
}

/// `GET /v1/templates`
#[utoipa::path(get, path = "/v1/templates", tag = "templates", responses((status = 200, body = Vec<Template>)))]
pub async fn list(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Template>>, ApiError> {
    Ok(Json(state.store.list_templates().await?))
}

async fn save(
    state: &AppState,
    auth: &AuthUser,
    id: Option<i64>,
    req: TemplateInput,
) -> Result<Template, ApiError> {
    AuthUser::require(auth.permissions(state).await?.manage_templates)?;
    let records = validate(&req)?;
    let saved = state
        .store
        .save_template(id, req.name.trim(), req.description.trim(), &records)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(d) if d.message().to_ascii_lowercase().contains("unique") => {
                ApiError::Conflict("a template with that name exists".into())
            }
            other => ApiError::from(other),
        })?
        .ok_or_else(|| ApiError::NotFound("template".into()))?;
    let p = auth.principal(state).await?;
    let op = if id.is_some() {
        "template.update"
    } else {
        "template.create"
    };
    record(
        state,
        auth,
        &p,
        op,
        None,
        Ok(()),
        None,
        Some(json!({"name": req.name})),
    )
    .await;
    state
        .store
        .get_template(saved)
        .await?
        .ok_or_else(|| ApiError::NotFound("template".into()))
}

/// `POST /v1/templates`
#[utoipa::path(post, path = "/v1/templates", tag = "templates", request_body = TemplateInput, responses((status = 201, body = Template)))]
pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TemplateInput>,
) -> Result<(StatusCode, Json<Template>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(save(&state, &auth, None, req).await?),
    ))
}

/// `PUT /v1/templates/{id}`
#[utoipa::path(put, path = "/v1/templates/{id}", tag = "templates", request_body = TemplateInput, responses((status = 200, body = Template)))]
pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<TemplateInput>,
) -> Result<Json<Template>, ApiError> {
    Ok(Json(save(&state, &auth, Some(id), req).await?))
}

/// `DELETE /v1/templates/{id}`
#[utoipa::path(delete, path = "/v1/templates/{id}", tag = "templates", responses((status = 204)))]
pub async fn remove(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    AuthUser::require(auth.permissions(&state).await?.manage_templates)?;
    if !state.store.delete_template(id).await? {
        return Err(ApiError::NotFound("template".into()));
    }
    let p = auth.principal(&state).await?;
    record(
        &state,
        &auth,
        &p,
        "template.delete",
        None,
        Ok(()),
        None,
        Some(json!({"id": id})),
    )
    .await;
    Ok(no_content())
}
