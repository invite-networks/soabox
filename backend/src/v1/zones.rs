//! Zones, records, DNSSEC, metadata, and zone history for the UI.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::IpAddr;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::{IntoParams, ToSchema};

use super::{no_content, record, zone_in_scope};
use crate::audit;
use crate::auth::AuthUser;
use crate::authz::Principal;
use crate::errors::ApiError;
use crate::pdns::ZoneName;
use crate::pdns::model::{RrSet, Zone};
use crate::settings::Settings;
use crate::state::AppState;
use crate::store::HistoryFilter;

/// The account a zone belongs to.
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
pub struct AccountRef {
    pub id: i64,
    pub name: String,
}

/// A zone in a list.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ZoneSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub serial: u64,
    pub dnssec: bool,
    pub account: Option<AccountRef>,
    pub masters: Vec<String>,
}

/// A page of zones.
#[derive(Debug, Serialize, ToSchema)]
pub struct ZonePage {
    pub items: Vec<ZoneSummary>,
    pub total: usize,
}

/// A zone with its records.
#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneDetail {
    #[serde(flatten)]
    pub summary: ZoneSummary,
    pub soa_edit_api: String,
    pub api_rectify: bool,
    pub nsec3param: String,
    /// Record types the editor may offer in this zone, from settings.
    pub allowed_record_types: Vec<String>,
    #[schema(value_type = Vec<Object>)]
    pub rrsets: Vec<RrSet>,
}

/// Zone list query.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ZoneQuery {
    pub q: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

fn summary(
    z: &crate::pdns::model::ZoneSummary,
    accounts: &HashMap<String, (i64, String)>,
) -> Option<ZoneSummary> {
    let (name, _) = ZoneName::from_url_id(&z.id)?;
    Some(ZoneSummary {
        id: z.id.clone(),
        name: z.name.clone(),
        kind: z.kind.clone(),
        serial: z.serial,
        dnssec: z.dnssec,
        account: accounts.get(name.as_str()).map(|(id, n)| AccountRef {
            id: *id,
            name: n.clone(),
        }),
        masters: z.masters.clone(),
    })
}

/// `GET /v1/zones`
#[utoipa::path(get, path = "/v1/zones", tag = "zones", params(ZoneQuery), responses((status = 200, body = ZonePage)))]
pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ZoneQuery>,
) -> Result<Json<ZonePage>, ApiError> {
    let p = auth.principal(&state).await?;
    let accounts = state.store.all_zone_accounts().await?;
    let needle = q.q.unwrap_or_default().trim().to_ascii_lowercase();
    let mut items: Vec<ZoneSummary> = state
        .pdns
        .list_zones()
        .await?
        .iter()
        .filter(|z| {
            ZoneName::from_url_id(&z.id).is_some_and(|(n, _)| p.can_access_zone(&n))
                && (needle.is_empty() || z.name.to_ascii_lowercase().contains(&needle))
        })
        .filter_map(|z| summary(z, &accounts))
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    let total = items.len();
    let per_page = q.per_page.unwrap_or(50).clamp(1, 500);
    let page = q.page.unwrap_or(1).max(1);
    let items = items
        .into_iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .collect();
    Ok(Json(ZonePage { items, total }))
}

async fn detail(state: &AppState, zone: &ZoneName) -> Result<ZoneDetail, ApiError> {
    let z: Zone = state
        .pdns
        .get(&format!("/zones/{}", zone.to_url_id()))
        .await?;
    let accounts = state.store.all_zone_accounts().await?;
    let s = crate::pdns::model::ZoneSummary {
        id: z.id.clone(),
        name: z.name.clone(),
        kind: z.kind.clone(),
        account: z.account.clone(),
        dnssec: z.dnssec,
        serial: z.serial,
        masters: z.masters.clone(),
        extra: serde_json::Map::new(),
    };
    let text = |k: &str| {
        z.extra
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut rrsets = z.rrsets.clone();
    rrsets.sort_by(|a, b| (&a.name, &a.rtype).cmp(&(&b.name, &b.rtype)));
    Ok(ZoneDetail {
        summary: summary(&s, &accounts).ok_or_else(|| ApiError::NotFound("zone".into()))?,
        soa_edit_api: text("soa_edit_api"),
        api_rectify: z
            .extra
            .get("api_rectify")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        nsec3param: text("nsec3param"),
        allowed_record_types: {
            let settings = Settings::load(&state.store).await?;
            if is_reverse(zone) {
                settings.record_types_reverse
            } else {
                settings.record_types_forward
            }
        },
        rrsets,
    })
}

/// `GET /v1/zones/{id}`
#[utoipa::path(get, path = "/v1/zones/{id}", tag = "zones", responses((status = 200, body = ZoneDetail)))]
pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ZoneDetail>, ApiError> {
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    Ok(Json(detail(&state, &zone).await?))
}

/// A new zone.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateZone {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub masters: Vec<String>,
    #[serde(default)]
    pub nameservers: Vec<String>,
    pub template_id: Option<i64>,
    pub account_id: Option<i64>,
    pub soa_edit_api: Option<String>,
}

/// Turns relative template names into absolute names in `zone`.
pub fn absolute_name(name: &str, zone: &ZoneName) -> String {
    let n = name.trim();
    if n.is_empty() || n == "@" {
        zone.to_string()
    } else if n.ends_with('.') {
        n.to_ascii_lowercase()
    } else {
        format!("{}.{}", n.to_ascii_lowercase(), zone)
    }
}

/// Positions of host names in each record type's presentation format.
fn host_fields(rtype: &str) -> &'static [usize] {
    match rtype {
        "CNAME" | "NS" | "PTR" | "DNAME" | "ALIAS" => &[0],
        "MX" | "AFSDB" | "HTTPS" | "SVCB" => &[1],
        "SRV" => &[3],
        "NAPTR" => &[5],
        "RP" | "SOA" => &[0, 1],
        _ => &[],
    }
}

/// Splits presentation format into words, keeping quoted strings whole.
fn split_tokens(content: &str) -> Option<Vec<&str>> {
    let mut tokens = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            if i >= bytes.len() {
                return None;
            }
            i += 1;
        } else {
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
        }
        tokens.push(content.get(start..i)?);
    }
    Some(tokens)
}

/// Expands relative host names in template record content to names in `zone`,
/// following zone file rules: `@` is the zone and names without a trailing dot are relative.
pub fn expand_template_content(rtype: &str, content: &str, zone: &ZoneName) -> String {
    let fields = host_fields(rtype);
    if fields.is_empty() {
        return content.to_owned();
    }
    let Some(tokens) = split_tokens(content) else {
        return content.to_owned();
    };
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if fields.contains(&i) && !t.starts_with('"') {
                absolute_name(t, zone)
            } else {
                (*t).to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn template_rrsets(t: &crate::store::Template, zone: &ZoneName) -> Vec<Value> {
    let mut sets: BTreeMap<(String, String), (i64, Vec<Value>)> = BTreeMap::new();
    for r in &t.records {
        let rtype = r.rtype.to_ascii_uppercase();
        let content = expand_template_content(&rtype, &r.content, zone);
        let key = (absolute_name(&r.name, zone), rtype);
        let e = sets.entry(key).or_insert((r.ttl, Vec::new()));
        e.1.push(json!({"content": content, "disabled": false}));
    }
    sets.into_iter()
        .map(|((name, rtype), (ttl, records))| {
            json!({"name": name, "type": rtype, "ttl": ttl, "changetype": "REPLACE", "records": records})
        })
        .collect()
}

/// `POST /v1/zones`
#[utoipa::path(post, path = "/v1/zones", tag = "zones", request_body = CreateZone, responses((status = 201, body = ZoneDetail)))]
pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateZone>,
) -> Result<(StatusCode, Json<ZoneDetail>), ApiError> {
    let p = auth.principal(&state).await?;
    AuthUser::require(p.can_create_zones())?;
    let zone = ZoneName::from_name(&req.name)
        .filter(|z| z.as_str() != "." && z.as_str().len() <= 254)
        .ok_or_else(|| ApiError::Validation("zone name is not valid".into()))?;
    let kind = match req.kind.as_str() {
        "Native" | "Master" | "Slave" | "Primary" | "Secondary" => req.kind.clone(),
        _ => {
            return Err(ApiError::Validation(
                "kind must be Native, Master, or Slave".into(),
            ));
        }
    };
    let secondary = matches!(kind.as_str(), "Slave" | "Secondary") || !req.masters.is_empty();
    if secondary && !p.manage_secondaries {
        return Err(ApiError::Forbidden(
            "only Admins and Operators can create secondary zones".into(),
        ));
    }
    if !p.all_zones() {
        let existing: Vec<ZoneName> = state
            .pdns
            .list_zones()
            .await?
            .iter()
            .filter_map(|z| ZoneName::from_url_id(&z.id).map(|(n, _)| n))
            .collect();
        p.check_new_zone(&zone, &existing)
            .map_err(ApiError::Forbidden)?;
    }
    let account = match req.account_id {
        None => None,
        Some(aid) => {
            if !p.all_zones()
                && !state
                    .store
                    .user_account_ids(p.actor_id)
                    .await?
                    .contains(&aid)
            {
                return Err(ApiError::Forbidden(
                    "you are not a member of that account".into(),
                ));
            }
            Some(
                state
                    .store
                    .get_account(aid)
                    .await?
                    .ok_or_else(|| ApiError::Validation("account not found".into()))?,
            )
        }
    };
    let mut body = json!({
        "name": zone.to_string(),
        "kind": kind,
        "masters": req.masters,
        "nameservers": req.nameservers.iter().map(|n| absolute_name(n, &zone)).collect::<Vec<_>>(),
        "soa_edit_api": req.soa_edit_api.clone().unwrap_or_else(|| "DEFAULT".into()),
    });
    if let Some(a) = &account {
        body["account"] = json!(a.name);
    }
    if let Some(tid) = req.template_id {
        let t = state
            .store
            .get_template(tid)
            .await?
            .ok_or_else(|| ApiError::Validation("template not found".into()))?;
        let sets = template_rrsets(&t, &zone);
        let has_ns = sets
            .iter()
            .any(|s| s["type"] == "NS" && s["name"] == zone.as_str());
        if has_ns {
            body["nameservers"] = json!([]);
        }
        body["rrsets"] = Value::Array(sets);
    }
    let result = state.pdns.send(Method::POST, "/zones", Some(&body)).await;
    let result = result.map_err(ApiError::from);
    let detail_v = json!({"kind": kind, "account": account.as_ref().map(|a| a.name.clone()), "template_id": req.template_id});
    match &result {
        Ok(created) => {
            let created: Zone = created
                .clone()
                .and_then(|v| serde_json::from_value(v).ok())
                .ok_or_else(|| ApiError::internal(anyhow::anyhow!("PowerDNS returned no zone")))?;
            state.store.forget_zone(zone.as_str()).await?;
            if let Some(a) = &account {
                state.store.set_zone_account(zone.as_str(), a.id).await?;
            } else if !p.all_zones() {
                state.store.add_user_zone(p.actor_id, zone.as_str()).await?;
            }
            let diff = serde_json::to_value(audit::diff_rrsets(&[], &created.rrsets)).ok();
            record(
                &state,
                &auth,
                &p,
                "zone.create",
                Some(&zone),
                Ok(()),
                diff,
                Some(detail_v),
            )
            .await;
        }
        Err(e) => {
            record(
                &state,
                &auth,
                &p,
                "zone.create",
                Some(&zone),
                Err(e),
                None,
                Some(detail_v),
            )
            .await
        }
    }
    result?;
    Ok((StatusCode::CREATED, Json(detail(&state, &zone).await?)))
}

/// Zone-level changes.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateZone {
    pub kind: Option<String>,
    pub masters: Option<Vec<String>>,
    /// `null` removes the zone from its account. Omit to leave unchanged.
    #[serde(default, with = "double_option")]
    #[schema(value_type = Option<i64>)]
    pub account_id: Option<Option<i64>>,
    pub soa_edit_api: Option<String>,
    pub api_rectify: Option<bool>,
}

mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<i64>>, D::Error> {
        Option::<i64>::deserialize(d).map(Some)
    }
}

/// `PUT /v1/zones/{id}`
#[utoipa::path(put, path = "/v1/zones/{id}", tag = "zones", request_body = UpdateZone, responses((status = 204)))]
pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateZone>,
) -> Result<StatusCode, ApiError> {
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    let to_secondary = req
        .kind
        .as_deref()
        .is_some_and(|k| matches!(k, "Slave" | "Secondary" | "Consumer"));
    if (to_secondary || req.masters.is_some()) && !p.manage_secondaries {
        return Err(ApiError::Forbidden(
            "only Admins and Operators can change primaries or make a zone secondary".into(),
        ));
    }
    let mut body = serde_json::Map::new();
    if let Some(k) = &req.kind {
        body.insert("kind".into(), json!(k));
    }
    if let Some(m) = &req.masters {
        body.insert("masters".into(), json!(m));
    }
    if let Some(s) = &req.soa_edit_api {
        body.insert("soa_edit_api".into(), json!(s));
    }
    if let Some(r) = req.api_rectify {
        body.insert("api_rectify".into(), json!(r));
    }
    let mut new_account = None;
    if let Some(acct) = req.account_id {
        AuthUser::require(p.all_zones())?;
        let name = match acct {
            None => String::new(),
            Some(aid) => {
                let a = state
                    .store
                    .get_account(aid)
                    .await?
                    .ok_or_else(|| ApiError::Validation("account not found".into()))?;
                new_account = Some(a.id);
                a.name
            }
        };
        body.insert("account".into(), json!(name));
    }
    if body.is_empty() {
        return Ok(no_content());
    }
    let path = format!("/zones/{}", zone.to_url_id());
    let before: Option<Zone> = state.pdns.get(&format!("{path}?rrsets=false")).await.ok();
    let body = Value::Object(body);
    let result = state
        .pdns
        .send(Method::PUT, &path, Some(&body))
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    let mut diff = None;
    if result.is_ok() {
        if req.account_id.is_some() {
            match new_account {
                Some(a) => state.store.set_zone_account(zone.as_str(), a).await?,
                None => state.store.clear_zone_account(zone.as_str()).await?,
            }
        }
        if let (Some(b), Ok(a)) = (
            before,
            state
                .pdns
                .get::<Zone>(&format!("{path}?rrsets=false"))
                .await,
        ) {
            diff = Some(json!({"zone": audit::diff_zone_fields(&b, &a)}));
        }
    }
    record(
        &state,
        &auth,
        &p,
        "zone.update",
        Some(&zone),
        result.as_ref().map(|_| ()),
        diff,
        Some(body),
    )
    .await;
    result.map(|()| no_content())
}

/// `DELETE /v1/zones/{id}`
#[utoipa::path(delete, path = "/v1/zones/{id}", tag = "zones", responses((status = 204)))]
pub async fn remove(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    AuthUser::require(p.all_zones() || p.can_delete_zones)?;
    let path = format!("/zones/{}", zone.to_url_id());
    let before: Option<Zone> = state.pdns.get(&path).await.ok();
    let result = state
        .pdns
        .send(Method::DELETE, &path, None)
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    let diff = if result.is_ok() {
        state.store.forget_zone(zone.as_str()).await?;
        before.and_then(|b| serde_json::to_value(audit::diff_rrsets(&b.rrsets, &[])).ok())
    } else {
        None
    };
    record(
        &state,
        &auth,
        &p,
        "zone.delete",
        Some(&zone),
        result.as_ref().map(|_| ()),
        diff,
        None,
    )
    .await;
    result.map(|()| no_content())
}

/// One RRset change.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RrSetChange {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    #[serde(default)]
    pub ttl: Option<u32>,
    pub changetype: String,
    #[serde(default)]
    #[schema(value_type = Vec<Object>)]
    pub records: Vec<crate::pdns::model::Record>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Vec<Object>>)]
    pub comments: Option<Vec<crate::pdns::model::Comment>>,
}

/// A batch of RRset changes.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RrSetPatch {
    pub rrsets: Vec<RrSetChange>,
}

fn is_reverse(zone: &ZoneName) -> bool {
    zone.as_str().ends_with(".in-addr.arpa.") || zone.as_str().ends_with(".ip6.arpa.")
}

async fn read_sets(
    state: &AppState,
    zone: &ZoneName,
    keys: &BTreeSet<(String, String)>,
) -> Result<Vec<RrSet>, ApiError> {
    let mut out = Vec::new();
    for (name, rtype) in keys {
        out.extend(state.pdns.get_rrset(zone, name, rtype).await?);
    }
    Ok(out)
}

/// `PATCH /v1/zones/{id}/rrsets`
#[utoipa::path(patch, path = "/v1/zones/{id}/rrsets", tag = "zones", request_body = RrSetPatch, responses((status = 204)))]
pub async fn patch_rrsets(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<RrSetPatch>,
) -> Result<StatusCode, ApiError> {
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    let settings = Settings::load(&state.store).await?;
    let allowed = if is_reverse(&zone) {
        &settings.record_types_reverse
    } else {
        &settings.record_types_forward
    };
    if req.rrsets.is_empty() {
        return Err(ApiError::Validation("no changes".into()));
    }
    let mut keys = BTreeSet::new();
    for c in &req.rrsets {
        let t = c.rtype.to_ascii_uppercase();
        if !allowed.contains(&t) {
            return Err(ApiError::Validation(format!(
                "record type {t} is not allowed in this zone"
            )));
        }
        if !matches!(c.changetype.as_str(), "REPLACE" | "DELETE") {
            return Err(ApiError::Validation(
                "changetype must be REPLACE or DELETE".into(),
            ));
        }
        if c.changetype == "REPLACE" && c.ttl.is_none() {
            return Err(ApiError::Validation(format!("{} {t} needs a TTL", c.name)));
        }
        if !zone.contains_name(&c.name) {
            return Err(ApiError::Validation(format!(
                "{} is not in this zone",
                c.name
            )));
        }
        keys.insert((c.name.to_ascii_lowercase(), t));
    }
    let before = read_sets(&state, &zone, &keys).await?;
    let body = serde_json::to_value(json!({"rrsets": req.rrsets})).unwrap_or_default();
    let result = state
        .pdns
        .send(
            Method::PATCH,
            &format!("/zones/{}", zone.to_url_id()),
            Some(&body),
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    let mut after = Vec::new();
    let diff = if result.is_ok() {
        after = read_sets(&state, &zone, &keys).await.unwrap_or_default();
        serde_json::to_value(audit::diff_rrsets(&before, &after)).ok()
    } else {
        None
    };
    record(
        &state,
        &auth,
        &p,
        "rrsets.patch",
        Some(&zone),
        result.as_ref().map(|_| ()),
        diff,
        (!result.is_ok()).then_some(body),
    )
    .await;
    result?;
    if settings.auto_ptr && !is_reverse(&zone) {
        auto_ptr(&state, &auth, &p, &before, &after).await;
    }
    Ok(no_content())
}

/// The reverse DNS name for an address, for example `4.3.2.1.in-addr.arpa.`.
pub fn reverse_name(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            format!("{}.{}.{}.{}.in-addr.arpa.", o[3], o[2], o[1], o[0])
        }
        IpAddr::V6(v6) => {
            let mut s = String::new();
            for b in v6.octets().iter().rev() {
                s.push_str(&format!("{:x}.{:x}.", b & 0x0f, b >> 4));
            }
            s.push_str("ip6.arpa.");
            s
        }
    }
}

fn address_map(sets: &[RrSet]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for s in sets.iter().filter(|s| s.rtype == "A" || s.rtype == "AAAA") {
        for r in s.records.iter().filter(|r| !r.disabled) {
            if let Ok(ip) = r.content.parse::<IpAddr>() {
                out.insert(reverse_name(ip), s.name.to_ascii_lowercase());
            }
        }
    }
    out
}

/// Keeps PTR records in reverse zones the caller can edit in step with A
/// and AAAA changes. Each PTR change is its own history entry.
async fn auto_ptr(
    state: &AppState,
    auth: &AuthUser,
    p: &Principal,
    before: &[RrSet],
    after: &[RrSet],
) {
    let old = address_map(before);
    let new = address_map(after);
    if old == new {
        return;
    }
    let Ok(zones) = state.pdns.list_zones().await else {
        return;
    };
    let reverse: Vec<ZoneName> = zones
        .iter()
        .filter_map(|z| ZoneName::from_url_id(&z.id).map(|(n, _)| n))
        .filter(|z| is_reverse(z) && p.can_access_zone(z))
        .collect();
    let zone_for = |name: &str| {
        reverse
            .iter()
            .filter(|z| z.contains_name(name))
            .max_by_key(|z| z.as_str().len())
            .cloned()
    };
    for (ptr, target) in &new {
        if old.get(ptr) == Some(target) {
            continue;
        }
        if let Some(z) = zone_for(ptr) {
            let body = json!({"rrsets": [{"name": ptr, "type": "PTR", "ttl": 3600, "changetype": "REPLACE",
                "records": [{"content": target, "disabled": false}]}]});
            ptr_change(state, auth, p, &z, body).await;
        }
    }
    for (ptr, target) in &old {
        if new.contains_key(ptr) {
            continue;
        }
        let Some(z) = zone_for(ptr) else { continue };
        let current = state
            .pdns
            .get_rrset(&z, ptr, "PTR")
            .await
            .unwrap_or_default();
        let points_here = current
            .iter()
            .flat_map(|s| &s.records)
            .any(|r| r.content.eq_ignore_ascii_case(target));
        if points_here {
            let body = json!({"rrsets": [{"name": ptr, "type": "PTR", "changetype": "DELETE"}]});
            ptr_change(state, auth, p, &z, body).await;
        }
    }
}

async fn ptr_change(
    state: &AppState,
    auth: &AuthUser,
    p: &Principal,
    zone: &ZoneName,
    body: Value,
) {
    let name = body["rrsets"][0]["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let keys: BTreeSet<_> = [(name, "PTR".to_owned())].into_iter().collect();
    let before = read_sets(state, zone, &keys).await.unwrap_or_default();
    let result = state
        .pdns
        .send(
            Method::PATCH,
            &format!("/zones/{}", zone.to_url_id()),
            Some(&body),
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    let diff = if result.is_ok() {
        let after = read_sets(state, zone, &keys).await.unwrap_or_default();
        serde_json::to_value(audit::diff_rrsets(&before, &after)).ok()
    } else {
        None
    };
    record(
        state,
        auth,
        p,
        "rrsets.patch",
        Some(zone),
        result.as_ref().map(|_| ()),
        diff,
        Some(json!({"auto_ptr": true})),
    )
    .await;
}

/// A short PowerDNS result message.
#[derive(Debug, Serialize, ToSchema)]
pub struct ActionResult {
    pub result: String,
}

async fn zone_action(
    state: AppState,
    auth: AuthUser,
    id: String,
    action: &str,
    op: &str,
) -> Result<Json<ActionResult>, ApiError> {
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    let result = state
        .pdns
        .send(
            Method::PUT,
            &format!("/zones/{}/{action}", zone.to_url_id()),
            None,
        )
        .await
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        op,
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        None,
    )
    .await;
    let v = result?.unwrap_or(Value::Null);
    Ok(Json(ActionResult {
        result: v["result"].as_str().unwrap_or("done").to_owned(),
    }))
}

/// `POST /v1/zones/{id}/notify`
#[utoipa::path(post, path = "/v1/zones/{id}/notify", tag = "zones", responses((status = 200, body = ActionResult)))]
pub async fn notify(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ActionResult>, ApiError> {
    zone_action(state, auth, id, "notify", "zone.notify").await
}

/// `POST /v1/zones/{id}/rectify`
#[utoipa::path(post, path = "/v1/zones/{id}/rectify", tag = "zones", responses((status = 200, body = ActionResult)))]
pub async fn rectify(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ActionResult>, ApiError> {
    zone_action(state, auth, id, "rectify", "zone.rectify").await
}

/// `POST /v1/zones/{id}/axfr-retrieve`
#[utoipa::path(post, path = "/v1/zones/{id}/axfr-retrieve", tag = "zones", responses((status = 200, body = ActionResult)))]
pub async fn axfr_retrieve(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ActionResult>, ApiError> {
    zone_action(state, auth, id, "axfr-retrieve", "zone.axfr_retrieve").await
}

/// Zone file text.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ZoneExport {
    pub zone: String,
}

/// `GET /v1/zones/{id}/export`
#[utoipa::path(get, path = "/v1/zones/{id}/export", tag = "zones", responses((status = 200, body = ZoneExport)))]
pub async fn export(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ZoneExport>, ApiError> {
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    Ok(Json(
        state
            .pdns
            .get(&format!("/zones/{}/export", zone.to_url_id()))
            .await?,
    ))
}

/// `GET /v1/zones/{id}/history`
#[utoipa::path(get, path = "/v1/zones/{id}/history", tag = "zones", responses((status = 200, body = super::server::HistoryPage)))]
pub async fn history(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<super::server::HistoryQuery>,
) -> Result<Json<super::server::HistoryPage>, ApiError> {
    AuthUser::require(auth.permissions(&state).await?.view_history)?;
    let p = auth.principal(&state).await?;
    let zone = zone_in_scope(&p, &id)?;
    let items = state
        .store
        .list_history(&HistoryFilter {
            zone: Some(zone.to_string()),
            before_id: q.before_id,
            limit: q.limit.unwrap_or(50),
            ..Default::default()
        })
        .await?;
    Ok(Json(super::server::HistoryPage { items }))
}

async fn dnssec_scope(
    state: &AppState,
    auth: &AuthUser,
    id: &str,
) -> Result<(Principal, ZoneName), ApiError> {
    AuthUser::require(auth.permissions(state).await?.manage_dnssec)?;
    let p = auth.principal(state).await?;
    let zone = zone_in_scope(&p, id)?;
    Ok((p, zone))
}

/// Turn DNSSEC on or off.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DnssecToggle {
    pub enabled: bool,
    pub nsec3param: Option<String>,
}

/// `POST /v1/zones/{id}/dnssec`
#[utoipa::path(post, path = "/v1/zones/{id}/dnssec", tag = "dnssec", request_body = DnssecToggle, responses((status = 204)))]
pub async fn set_dnssec(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<DnssecToggle>,
) -> Result<StatusCode, ApiError> {
    let (p, zone) = dnssec_scope(&state, &auth, &id).await?;
    let mut body = json!({"dnssec": req.enabled});
    if let Some(n) = &req.nsec3param {
        body["nsec3param"] = json!(n);
    }
    let result = state
        .pdns
        .send(
            Method::PUT,
            &format!("/zones/{}", zone.to_url_id()),
            Some(&body),
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    let op = if req.enabled {
        "dnssec.enable"
    } else {
        "dnssec.disable"
    };
    record(
        &state,
        &auth,
        &p,
        op,
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        Some(body),
    )
    .await;
    result.map(|()| no_content())
}

/// `GET /v1/zones/{id}/cryptokeys`
#[utoipa::path(get, path = "/v1/zones/{id}/cryptokeys", tag = "dnssec", responses((status = 200, body = Object)))]
pub async fn list_cryptokeys(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (_, zone) = dnssec_scope(&state, &auth, &id).await?;
    Ok(Json(
        state
            .pdns
            .get(&format!("/zones/{}/cryptokeys", zone.to_url_id()))
            .await?,
    ))
}

/// A new DNSSEC key.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct NewCryptokey {
    pub keytype: String,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits: Option<u32>,
}

/// `POST /v1/zones/{id}/cryptokeys`
#[utoipa::path(post, path = "/v1/zones/{id}/cryptokeys", tag = "dnssec", request_body = NewCryptokey, responses((status = 201, body = Object)))]
pub async fn create_cryptokey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<NewCryptokey>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (p, zone) = dnssec_scope(&state, &auth, &id).await?;
    let mut req = req;
    if req.algorithm.as_deref().is_none_or(str::is_empty) {
        req.algorithm = Some("ecdsap256sha256".into());
    }
    if !matches!(req.keytype.as_str(), "ksk" | "zsk" | "csk") {
        return Err(ApiError::Validation(
            "keytype must be ksk, zsk, or csk".into(),
        ));
    }
    let body = serde_json::to_value(&req).unwrap_or_default();
    let result = state
        .pdns
        .send(
            Method::POST,
            &format!("/zones/{}/cryptokeys", zone.to_url_id()),
            Some(&body),
        )
        .await
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "cryptokey.create",
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        Some(body),
    )
    .await;
    let mut v = result?.unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.remove("privatekey");
    }
    Ok((StatusCode::CREATED, Json(v)))
}

/// Key state changes.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CryptokeyUpdate {
    pub active: bool,
    pub published: bool,
}

/// `PUT /v1/zones/{id}/cryptokeys/{key_id}`
#[utoipa::path(put, path = "/v1/zones/{id}/cryptokeys/{key_id}", tag = "dnssec", request_body = CryptokeyUpdate, responses((status = 204)))]
pub async fn update_cryptokey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, key_id)): Path<(String, u64)>,
    Json(req): Json<CryptokeyUpdate>,
) -> Result<StatusCode, ApiError> {
    let (p, zone) = dnssec_scope(&state, &auth, &id).await?;
    let body = serde_json::to_value(&req).unwrap_or_default();
    let result = state
        .pdns
        .send(
            Method::PUT,
            &format!("/zones/{}/cryptokeys/{key_id}", zone.to_url_id()),
            Some(&body),
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "cryptokey.update",
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        Some(json!({"id": key_id, "active": req.active, "published": req.published})),
    )
    .await;
    result.map(|()| no_content())
}

/// `DELETE /v1/zones/{id}/cryptokeys/{key_id}`
#[utoipa::path(delete, path = "/v1/zones/{id}/cryptokeys/{key_id}", tag = "dnssec", responses((status = 204)))]
pub async fn delete_cryptokey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, key_id)): Path<(String, u64)>,
) -> Result<StatusCode, ApiError> {
    let (p, zone) = dnssec_scope(&state, &auth, &id).await?;
    let result = state
        .pdns
        .send(
            Method::DELETE,
            &format!("/zones/{}/cryptokeys/{key_id}", zone.to_url_id()),
            None,
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "cryptokey.delete",
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        Some(json!({"id": key_id})),
    )
    .await;
    result.map(|()| no_content())
}

/// `GET /v1/zones/{id}/metadata`
#[utoipa::path(get, path = "/v1/zones/{id}/metadata", tag = "dnssec", responses((status = 200, body = Object)))]
pub async fn list_metadata(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (_, zone) = dnssec_scope(&state, &auth, &id).await?;
    Ok(Json(
        state
            .pdns
            .get(&format!("/zones/{}/metadata", zone.to_url_id()))
            .await?,
    ))
}

/// Metadata values for one kind.
#[derive(Debug, Deserialize, ToSchema)]
pub struct MetadataValues {
    pub metadata: Vec<String>,
}

fn valid_kind(kind: &str) -> Result<(), ApiError> {
    if !kind.is_empty()
        && kind.len() <= 64
        && kind.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        Ok(())
    } else {
        Err(ApiError::Validation("metadata kind is not valid".into()))
    }
}

/// `PUT /v1/zones/{id}/metadata/{kind}`
#[utoipa::path(put, path = "/v1/zones/{id}/metadata/{kind}", tag = "dnssec", request_body = MetadataValues, responses((status = 204)))]
pub async fn put_metadata(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, kind)): Path<(String, String)>,
    Json(req): Json<MetadataValues>,
) -> Result<StatusCode, ApiError> {
    let (p, zone) = dnssec_scope(&state, &auth, &id).await?;
    valid_kind(&kind)?;
    p.check_metadata_kind(&kind).map_err(ApiError::Forbidden)?;
    let body = json!({"kind": kind, "metadata": req.metadata});
    let result = state
        .pdns
        .send(
            Method::PUT,
            &format!("/zones/{}/metadata/{kind}", zone.to_url_id()),
            Some(&body),
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "metadata.update",
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        Some(body),
    )
    .await;
    result.map(|()| no_content())
}

/// `DELETE /v1/zones/{id}/metadata/{kind}`
#[utoipa::path(delete, path = "/v1/zones/{id}/metadata/{kind}", tag = "dnssec", responses((status = 204)))]
pub async fn delete_metadata(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, kind)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let (p, zone) = dnssec_scope(&state, &auth, &id).await?;
    valid_kind(&kind)?;
    p.check_metadata_kind(&kind).map_err(ApiError::Forbidden)?;
    let result = state
        .pdns
        .send(
            Method::DELETE,
            &format!("/zones/{}/metadata/{kind}", zone.to_url_id()),
            None,
        )
        .await
        .map(|_| ())
        .map_err(ApiError::from);
    record(
        &state,
        &auth,
        &p,
        "metadata.delete",
        Some(&zone),
        result.as_ref().map(|_| ()),
        None,
        Some(json!({"kind": kind})),
    )
    .await;
    result.map(|()| no_content())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_names() {
        assert_eq!(
            reverse_name("192.0.2.10".parse().expect("ip")),
            "10.2.0.192.in-addr.arpa."
        );
        let v6 = reverse_name("2001:db8::1".parse().expect("ip"));
        assert!(v6.starts_with("1.0.0.0.0.0.0.0."));
        assert!(v6.ends_with(".8.b.d.0.1.0.0.2.ip6.arpa."));
    }

    #[test]
    fn absolute_names() {
        let z = ZoneName::from_name("example.test").expect("z");
        assert_eq!(absolute_name("@", &z), "example.test.");
        assert_eq!(absolute_name("WWW", &z), "www.example.test.");
        assert_eq!(absolute_name("ns1.other.", &z), "ns1.other.");
    }

    #[test]
    fn template_content_expands_relative_hosts() {
        let z = ZoneName::from_name("a.test").expect("z");
        assert_eq!(expand_template_content("CNAME", "www", &z), "www.a.test.");
        assert_eq!(expand_template_content("CNAME", "@", &z), "a.test.");
        assert_eq!(
            expand_template_content("CNAME", "cdn.other.net.", &z),
            "cdn.other.net."
        );
        assert_eq!(
            expand_template_content("MX", "10 mail", &z),
            "10 mail.a.test."
        );
        assert_eq!(expand_template_content("MX", "0 .", &z), "0 .");
        assert_eq!(
            expand_template_content("SRV", "10 5 5060 sip", &z),
            "10 5 5060 sip.a.test."
        );
        assert_eq!(
            expand_template_content("NAPTR", r#"100 10 "U" "E2U+sip" "!^.*$!sip:a@b!" sip"#, &z),
            r#"100 10 "U" "E2U+sip" "!^.*$!sip:a@b!" sip.a.test."#
        );
        assert_eq!(
            expand_template_content("SOA", "ns1 hostmaster 1 10800 3600 604800 3600", &z),
            "ns1.a.test. hostmaster.a.test. 1 10800 3600 604800 3600"
        );
        assert_eq!(
            expand_template_content("TXT", "\"v=spf1 -all\"", &z),
            "\"v=spf1 -all\""
        );
        assert_eq!(expand_template_content("A", "192.0.2.1", &z), "192.0.2.1");
        assert_eq!(expand_template_content("MX", "10 \"bad", &z), "10 \"bad");
    }

    #[test]
    fn template_expansion_groups_rrsets() {
        let t = crate::store::Template {
            id: 1,
            name: "t".into(),
            description: String::new(),
            records: vec![
                crate::store::TemplateRecord {
                    name: "@".into(),
                    rtype: "ns".into(),
                    ttl: 300,
                    content: "ns1.x.".into(),
                },
                crate::store::TemplateRecord {
                    name: "@".into(),
                    rtype: "NS".into(),
                    ttl: 300,
                    content: "ns2.x.".into(),
                },
                crate::store::TemplateRecord {
                    name: "www".into(),
                    rtype: "A".into(),
                    ttl: 60,
                    content: "192.0.2.1".into(),
                },
            ],
        };
        let sets = template_rrsets(&t, &ZoneName::from_name("a.test").expect("z"));
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0]["records"].as_array().expect("r").len(), 2);
        assert_eq!(sets[1]["name"], "www.a.test.");
    }

    #[test]
    fn address_maps() {
        let sets = vec![RrSet {
            name: "WWW.a.test.".into(),
            rtype: "A".into(),
            ttl: 60,
            records: vec![
                crate::pdns::model::Record {
                    content: "192.0.2.1".into(),
                    disabled: false,
                },
                crate::pdns::model::Record {
                    content: "192.0.2.2".into(),
                    disabled: true,
                },
            ],
            comments: vec![],
        }];
        let m = address_map(&sets);
        assert_eq!(m.len(), 1);
        assert_eq!(m["1.2.0.192.in-addr.arpa."], "www.a.test.");
    }
}
