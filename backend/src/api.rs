//! The `/api` passthrough: a PowerDNS 5.x API with SOAbox keys, scope, and
//! audit. See ADR-0003 and ADR-0004.

use std::collections::BTreeSet;

use axum::Extension;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Method, Uri};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde_json::Value;

use crate::audit::{self, AuditEvent};
use crate::authz::{self, Decision, Principal, RouteClass};
use crate::errors::PdnsError;
use crate::http::ClientInfo;
use crate::keys::KeyRejection;
use crate::pdns::json::to_pdns_vec;
use crate::pdns::model::{RrSet, Zone};
use crate::pdns::{UpstreamResponse, ZoneName};
use crate::state::AppState;

/// More RRsets than this in one PATCH are read with one full-zone request
/// instead of one request each.
const PER_RRSET_LIMIT: usize = 20;

/// Handles every request under `/api`.
pub async fn passthrough(
    State(state): State<AppState>,
    Extension(client): Extension<ClientInfo>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path_and_query = uri
        .path_and_query()
        .map_or_else(|| uri.path().to_owned(), |p| p.as_str().to_owned());
    let path = uri.path();
    let segments: Vec<&str> = path
        .trim_start_matches("/api")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if method == Method::OPTIONS {
        return forward_plain(&state, method, &path_and_query, &headers, body).await;
    }

    let presented = headers.get("x-api-key").and_then(|v| v.to_str().ok());
    let key = match state
        .keys
        .authenticate(&state.store, presented, client.ip)
        .await
    {
        Ok(Ok(k)) => k,
        Ok(Err(KeyRejection::RateLimited)) => return PdnsError::TooManyRequests.into_response(),
        Ok(Err(reason)) => {
            tracing::info!(?reason, client_ip = %client.ip, "API key rejected");
            return PdnsError::Unauthorized.into_response();
        }
        Err(e) => {
            tracing::error!(error = ?e, "API key lookup failed");
            return PdnsError::Internal.into_response();
        }
    };
    let principal = match Principal::for_api_key(&state.store, &key).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = ?e, "scope lookup failed");
            return PdnsError::Internal.into_response();
        }
    };

    let class = authz::classify(&method, &segments, state.pdns.server_id());
    match authz::decide(&principal, &method, &class) {
        Decision::NotFound => PdnsError::NotFound.into_response(),
        Decision::Unauthorized => PdnsError::Unauthorized.into_response(),
        Decision::AllowFiltered => {
            forward_filtered(&state, &principal, method, &path_and_query, &headers, body).await
        }
        Decision::Allow => {
            if is_write(&method) {
                let ctx = WriteCtx {
                    state: &state,
                    principal: &principal,
                    client: &client,
                    method,
                    path_and_query: &path_and_query,
                    segments: &segments,
                    headers: &headers,
                    class: &class,
                };
                audited_write(ctx, body).await
            } else {
                forward_plain(&state, method, &path_and_query, &headers, body).await
            }
        }
    }
}

fn is_write(m: &Method) -> bool {
    !matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn to_response(u: UpstreamResponse) -> Response {
    let mut resp = Response::new(Body::from(u.body));
    *resp.status_mut() = u.status;
    *resp.headers_mut() = u.headers;
    resp
}

async fn forward(
    state: &AppState,
    method: Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<UpstreamResponse, PdnsError> {
    state
        .pdns
        .forward(method, path_and_query, headers, body)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "PowerDNS request failed");
            PdnsError::BadGateway
        })
}

fn respond(result: Result<UpstreamResponse, PdnsError>) -> Response {
    match result {
        Ok(u) => to_response(u),
        Err(e) => e.into_response(),
    }
}

async fn forward_plain(
    state: &AppState,
    method: Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    respond(forward(state, method, path_and_query, headers, body).await)
}

/// Forwards a list request and keeps only entries for zones in scope.
async fn forward_filtered(
    state: &AppState,
    principal: &Principal,
    method: Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    let mut u = match forward(state, method, path_and_query, headers, body).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    if !u.status.is_success() {
        return to_response(u);
    }
    let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(&u.body) else {
        return to_response(u);
    };
    let kept: Vec<Value> = items
        .into_iter()
        .filter(|item| entry_zone(item).is_some_and(|z| principal.can_access_zone(&z)))
        .collect();
    u.body = Bytes::from(to_pdns_vec(&Value::Array(kept)));
    to_response(u)
}

/// The zone a list entry belongs to: `zone_id` in search results, `id` in
/// zone lists.
fn entry_zone(item: &Value) -> Option<ZoneName> {
    let id = item
        .get("zone_id")
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)?;
    ZoneName::from_url_id(id).map(|(z, _)| z)
}

struct WriteCtx<'a> {
    state: &'a AppState,
    principal: &'a Principal,
    client: &'a ClientInfo,
    method: Method,
    path_and_query: &'a str,
    segments: &'a [&'a str],
    headers: &'a HeaderMap,
    class: &'a RouteClass,
}

impl WriteCtx<'_> {
    fn event(&self, zone: Option<String>) -> AuditEvent<'_> {
        AuditEvent {
            principal: self.principal,
            client_ip: self.client.ip.to_string(),
            operation: audit::operation_name(&self.method, self.class, self.segments),
            zone,
            upstream_status: None,
            success: false,
            diff: None,
            detail: None,
        }
    }

    fn zone_url_id(&self) -> Option<&str> {
        // segments: v1 servers {id} zones {zone_id} ...
        self.segments.get(4).copied()
    }
}

async fn audited_write(ctx: WriteCtx<'_>, body: Bytes) -> Response {
    match ctx.class {
        RouteClass::ZoneCreate => zone_create(ctx, body).await,
        RouteClass::Zone { zone, rest, .. } if rest.is_empty() => {
            let zone = zone.clone();
            match ctx.method {
                Method::PATCH => rrsets_patch(ctx, &zone, body).await,
                Method::PUT => zone_update(ctx, &zone, body).await,
                Method::DELETE => zone_delete(ctx, &zone, body).await,
                _ => detail_only(ctx, Some(zone.to_string()), body).await,
            }
        }
        RouteClass::Zone { zone, .. } => {
            let z = zone.to_string();
            detail_only(ctx, Some(z), body).await
        }
        _ => detail_only(ctx, None, body).await,
    }
}

fn body_detail(body: &Bytes) -> Option<Value> {
    if body.is_empty() {
        return None;
    }
    serde_json::from_slice::<Value>(body)
        .ok()
        .map(|v| audit::redact(&v))
}

async fn finish(
    ctx: &WriteCtx<'_>,
    mut event: AuditEvent<'_>,
    result: &Result<UpstreamResponse, PdnsError>,
) {
    match result {
        Ok(u) => {
            event.upstream_status = Some(u.status.as_u16());
            event.success = u.status.is_success();
            if !event.success {
                event.diff = None;
                let err = serde_json::from_slice::<Value>(&u.body)
                    .ok()
                    .and_then(|v| v.get("error").cloned());
                let mut detail = event.detail.take().unwrap_or(Value::Null);
                if let Some(e) = err {
                    detail = serde_json::json!({ "request": detail, "error": e });
                }
                event.detail = (!detail.is_null()).then_some(detail);
            }
        }
        Err(_) => {
            event.success = false;
            event.diff = None;
        }
    }
    audit::record(&ctx.state.store, event).await;
}

async fn detail_only(ctx: WriteCtx<'_>, zone: Option<String>, body: Bytes) -> Response {
    let mut event = ctx.event(zone);
    event.detail = body_detail(&body);
    let result = forward(
        ctx.state,
        ctx.method.clone(),
        ctx.path_and_query,
        ctx.headers,
        body,
    )
    .await;
    finish(&ctx, event, &result).await;
    respond(result)
}

async fn zone_create(ctx: WriteCtx<'_>, body: Bytes) -> Response {
    let mut parsed = serde_json::from_slice::<Value>(&body).ok();
    let mut target_account: Option<i64> = None;

    if !ctx.principal.all_zones() {
        let Some(account_id) = ctx.principal.create_account_id else {
            return PdnsError::Unauthorized.into_response();
        };
        let account = match ctx.state.store.get_account(account_id).await {
            Ok(Some(a)) => a,
            Ok(None) => return PdnsError::Unauthorized.into_response(),
            Err(e) => {
                tracing::error!(error = ?e, "account lookup failed");
                return PdnsError::Internal.into_response();
            }
        };
        if let Some(Value::Object(map)) = parsed.as_mut() {
            map.insert("account".into(), Value::String(account.name.clone()));
        }
        target_account = Some(account.id);
    } else if let Some(name) = parsed
        .as_ref()
        .and_then(|v| v.get("account"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        && let Ok(Some(a)) = ctx.state.store.find_account_by_name(name).await
    {
        target_account = Some(a.id);
    }

    let body = match (&parsed, ctx.principal.all_zones()) {
        (Some(v), false) => Bytes::from(serde_json::to_vec(v).unwrap_or_default()),
        _ => body,
    };
    let zone_name = parsed
        .as_ref()
        .and_then(|v| v.get("name"))
        .and_then(Value::as_str)
        .and_then(ZoneName::from_name);

    let mut event = ctx.event(zone_name.as_ref().map(ToString::to_string));
    event.detail = parsed.as_ref().map(|v| {
        let mut d = audit::redact(v);
        if let Some(obj) = d.as_object_mut() {
            obj.remove("rrsets");
        }
        d
    });
    let result = forward(
        ctx.state,
        ctx.method.clone(),
        ctx.path_and_query,
        ctx.headers,
        body,
    )
    .await;

    if let Ok(u) = &result
        && u.status.is_success()
        && let Ok(created) = serde_json::from_slice::<Zone>(&u.body)
    {
        event.diff = Some(rr_diff_value(&[], &created.rrsets));
        if let Some(z) = ZoneName::from_name(&created.name) {
            event.zone = Some(z.to_string());
            if let Some(acct) = target_account
                && let Err(e) = ctx.state.store.set_zone_account(z.as_str(), acct).await
            {
                tracing::error!(error = ?e, zone = %z, "failed to assign zone to account");
            }
            if !ctx.principal.all_zones()
                && ctx.principal.actor_type == "api_key"
                && let Err(e) = ctx
                    .state
                    .store
                    .add_api_key_zone(ctx.principal.actor_id, z.as_str())
                    .await
            {
                tracing::error!(error = ?e, zone = %z, "failed to add zone to key scope");
            }
        }
    }
    finish(&ctx, event, &result).await;
    respond(result)
}

fn rr_diff_value(before: &[RrSet], after: &[RrSet]) -> Value {
    serde_json::to_value(audit::diff_rrsets(before, after)).unwrap_or(Value::Null)
}

/// RRsets named in a PATCH body as (name, type) pairs.
fn patch_targets(body: &Bytes) -> Option<BTreeSet<(String, String)>> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let sets = v.get("rrsets")?.as_array()?;
    Some(
        sets.iter()
            .filter_map(|s| {
                Some((
                    s.get("name")?.as_str()?.to_ascii_lowercase(),
                    s.get("type")?.as_str()?.to_ascii_uppercase(),
                ))
            })
            .collect(),
    )
}

async fn read_rrsets(
    state: &AppState,
    url_id: &str,
    targets: &BTreeSet<(String, String)>,
) -> Option<Vec<RrSet>> {
    if targets.is_empty() {
        return Some(Vec::new());
    }
    if targets.len() <= PER_RRSET_LIMIT {
        let mut out = Vec::new();
        for (name, rtype) in targets {
            let qs = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("rrset_name", name)
                .append_pair("rrset_type", rtype)
                .finish();
            let z: Zone = state
                .pdns
                .get(&format!("/zones/{url_id}?{qs}"))
                .await
                .ok()?;
            out.extend(z.rrsets);
        }
        Some(out)
    } else {
        let z: Zone = state.pdns.get(&format!("/zones/{url_id}")).await.ok()?;
        Some(
            z.rrsets
                .into_iter()
                .filter(|r| {
                    targets.contains(&(r.name.to_ascii_lowercase(), r.rtype.to_ascii_uppercase()))
                })
                .collect(),
        )
    }
}

async fn rrsets_patch(ctx: WriteCtx<'_>, zone: &ZoneName, body: Bytes) -> Response {
    let url_id = ctx.zone_url_id().unwrap_or_default().to_owned();
    let targets = patch_targets(&body);
    let before = match &targets {
        Some(t) => read_rrsets(ctx.state, &url_id, t).await,
        None => None,
    };
    let mut event = ctx.event(Some(zone.to_string()));
    let result = forward(
        ctx.state,
        ctx.method.clone(),
        ctx.path_and_query,
        ctx.headers,
        body.clone(),
    )
    .await;
    if let (Ok(u), Some(t), Some(b)) = (&result, &targets, &before)
        && u.status.is_success()
        && let Some(after) = read_rrsets(ctx.state, &url_id, t).await
    {
        event.diff = Some(rr_diff_value(b, &after));
    }
    if event.diff.is_none() {
        event.detail = body_detail(&body);
    }
    finish(&ctx, event, &result).await;
    respond(result)
}

async fn zone_update(ctx: WriteCtx<'_>, zone: &ZoneName, body: Bytes) -> Response {
    let url_id = ctx.zone_url_id().unwrap_or_default().to_owned();
    let path = format!("/zones/{url_id}?rrsets=false");
    let before: Option<Zone> = ctx.state.pdns.get(&path).await.ok();
    let mut event = ctx.event(Some(zone.to_string()));
    event.detail = body_detail(&body);
    let new_account = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("account").and_then(Value::as_str).map(str::to_owned));
    let result = forward(
        ctx.state,
        ctx.method.clone(),
        ctx.path_and_query,
        ctx.headers,
        body,
    )
    .await;
    if let Ok(u) = &result
        && u.status.is_success()
    {
        if let (Some(b), Ok(a)) = (&before, ctx.state.pdns.get::<Zone>(&path).await) {
            event.diff = Some(serde_json::json!({ "zone": audit::diff_zone_fields(b, &a) }));
        }
        if let Some(name) = new_account {
            sync_zone_account(ctx.state, zone, &name).await;
        }
    }
    finish(&ctx, event, &result).await;
    respond(result)
}

async fn sync_zone_account(state: &AppState, zone: &ZoneName, account: &str) {
    let r = if account.is_empty() {
        state.store.clear_zone_account(zone.as_str()).await
    } else {
        match state.store.find_account_by_name(account).await {
            Ok(Some(a)) => state.store.set_zone_account(zone.as_str(), a.id).await,
            Ok(None) => Ok(()),
            Err(e) => Err(e),
        }
    };
    if let Err(e) = r {
        tracing::error!(error = ?e, zone = %zone, "failed to sync zone account");
    }
}

async fn zone_delete(ctx: WriteCtx<'_>, zone: &ZoneName, body: Bytes) -> Response {
    let url_id = ctx.zone_url_id().unwrap_or_default().to_owned();
    let before: Option<Zone> = ctx.state.pdns.get(&format!("/zones/{url_id}")).await.ok();
    let mut event = ctx.event(Some(zone.to_string()));
    let result = forward(
        ctx.state,
        ctx.method.clone(),
        ctx.path_and_query,
        ctx.headers,
        body,
    )
    .await;
    if let Ok(u) = &result
        && u.status.is_success()
    {
        if let Some(b) = &before {
            event.diff = Some(rr_diff_value(&b.rrsets, &[]));
        }
        if !url_id.contains("..")
            && let Err(e) = ctx.state.store.forget_zone(zone.as_str()).await
        {
            tracing::error!(error = ?e, zone = %zone, "failed to clean up deleted zone");
        }
    }
    finish(&ctx, event, &result).await;
    respond(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_targets_parse() {
        let b = Bytes::from_static(
            br#"{"rrsets": [{"name": "WWW.a.test.", "type": "a"}, {"name": "x.a.test.", "type": "TXT"}]}"#,
        );
        let t = patch_targets(&b).expect("targets");
        assert!(t.contains(&("www.a.test.".into(), "A".into())));
        assert_eq!(t.len(), 2);
        assert!(patch_targets(&Bytes::from_static(b"{bad")).is_none());
    }

    #[test]
    fn entry_zone_reads_id_or_zone_id() {
        let v = serde_json::json!({"zone_id": "A.test.", "name": "x"});
        assert_eq!(entry_zone(&v).expect("z").as_str(), "a.test.");
        let v = serde_json::json!({"id": "0=2F26.x.", "name": "x"});
        assert_eq!(entry_zone(&v).expect("z").as_str(), "0/26.x.");
        assert!(entry_zone(&serde_json::json!({"name": "x"})).is_none());
    }
}
