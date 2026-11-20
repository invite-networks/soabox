//! Audit: what a write changed, and recording it.

use std::collections::BTreeMap;

use http::Method;
use serde::Serialize;
use serde_json::{Value, json};

use crate::authz::{Principal, RouteClass};
use crate::pdns::model::{Record, RrSet, Zone};
use crate::store::{NewHistory, Store};

/// The state of one RRset at a point in time.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RrState {
    pub ttl: u32,
    pub records: Vec<Record>,
}

/// One changed RRset.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RrChange {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    pub before: Option<RrState>,
    pub after: Option<RrState>,
}

/// Fields compared at zone level.
const ZONE_FIELDS: &[&str] = &[
    "kind",
    "account",
    "masters",
    "dnssec",
    "nsec3param",
    "nsec3narrow",
    "soa_edit",
    "soa_edit_api",
    "api_rectify",
    "catalog",
    "master_tsig_key_ids",
    "slave_tsig_key_ids",
];

fn key(r: &RrSet) -> (String, String) {
    (r.name.to_ascii_lowercase(), r.rtype.to_ascii_uppercase())
}

fn state(r: &RrSet) -> RrState {
    let mut records = r.records.clone();
    records.sort();
    RrState {
        ttl: r.ttl,
        records,
    }
}

/// Compares RRsets before and after a write. Unchanged sets are omitted.
pub fn diff_rrsets(before: &[RrSet], after: &[RrSet]) -> Vec<RrChange> {
    let b: BTreeMap<_, _> = before.iter().map(|r| (key(r), state(r))).collect();
    let a: BTreeMap<_, _> = after.iter().map(|r| (key(r), state(r))).collect();
    let mut keys: Vec<_> = b.keys().chain(a.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let before = b.get(&k).filter(|s| !s.records.is_empty()).cloned();
            let after = a.get(&k).filter(|s| !s.records.is_empty()).cloned();
            (before != after).then_some(RrChange {
                name: k.0,
                rtype: k.1,
                before,
                after,
            })
        })
        .collect()
}

/// Compares zone-level fields.
pub fn diff_zone_fields(before: &Zone, after: &Zone) -> Vec<Value> {
    let b = serde_json::to_value(before).unwrap_or(Value::Null);
    let a = serde_json::to_value(after).unwrap_or(Value::Null);
    ZONE_FIELDS
        .iter()
        .filter_map(|f| {
            let bv = b.get(*f).cloned().unwrap_or(Value::Null);
            let av = a.get(*f).cloned().unwrap_or(Value::Null);
            (bv != av).then(|| json!({"field": f, "before": bv, "after": av}))
        })
        .collect()
}

/// Replaces secret material in a request body before it is stored.
pub fn redact(v: &Value) -> Value {
    const SECRET_FIELDS: &[&str] = &["key", "privatekey", "password", "secret", "api_key"];
    match v {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, val)| {
                    if SECRET_FIELDS.contains(&k.to_ascii_lowercase().as_str()) {
                        (k.clone(), Value::String("[redacted]".into()))
                    } else {
                        (k.clone(), redact(val))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        other => other.clone(),
    }
}

/// A stable name for the operation a write performs.
pub fn operation_name(method: &Method, class: &RouteClass, segments: &[&str]) -> String {
    let verb = match *method {
        Method::POST => "create",
        Method::PUT => "update",
        Method::PATCH => "patch",
        Method::DELETE => "delete",
        _ => "call",
    };
    match class {
        RouteClass::ZoneCreate => "zone.create".into(),
        RouteClass::Zone { rest, .. } => match rest.first().map(String::as_str) {
            None if *method == Method::PATCH => "rrsets.patch".into(),
            None => format!("zone.{verb}"),
            Some("notify") => "zone.notify".into(),
            Some("axfr-retrieve") => "zone.axfr_retrieve".into(),
            Some("rectify") => "zone.rectify".into(),
            Some("cryptokeys") => format!("cryptokey.{verb}"),
            Some("metadata") => format!("metadata.{verb}"),
            Some(other) => format!("zone.{other}.{verb}"),
        },
        RouteClass::ServerAdmin | RouteClass::ServerRead => {
            let what = segments.get(3).copied().unwrap_or("server");
            match what {
                "tsigkeys" => format!("tsigkey.{verb}"),
                "autoprimaries" => format!("autoprimary.{verb}"),
                "cache" => "cache.flush".into(),
                "views" => format!("view.{verb}"),
                "networks" => format!("network.{verb}"),
                "config" => "config.update".into(),
                other => format!("{other}.{verb}"),
            }
        }
        _ => format!("{} /{}", method, segments.join("/")),
    }
}

/// Inputs for one history row.
#[derive(Debug, Clone)]
pub struct AuditEvent<'a> {
    pub principal: &'a Principal,
    pub client_ip: String,
    pub operation: String,
    pub zone: Option<String>,
    pub upstream_status: Option<u16>,
    pub success: bool,
    pub diff: Option<Value>,
    pub detail: Option<Value>,
}

/// Stores a history row. Failures are logged and never returned, because the
/// change already happened in PowerDNS. See ADR-0009.
pub async fn record(store: &Store, e: AuditEvent<'_>) {
    let row = NewHistory {
        actor_type: e.principal.actor_type.to_owned(),
        actor_id: Some(e.principal.actor_id),
        actor_name: e.principal.actor_name.clone(),
        client_ip: e.client_ip,
        operation: e.operation.clone(),
        zone_name: e.zone.clone(),
        outcome: if e.success { "success" } else { "failure" }.to_owned(),
        upstream_status: e.upstream_status.map(i64::from),
        diff: e.diff,
        detail: e.detail,
    };
    if let Err(err) = store.insert_history(&row).await {
        tracing::error!(
            error = ?err,
            operation = %e.operation,
            zone = ?e.zone,
            "failed to store history"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rr(name: &str, t: &str, ttl: u32, contents: &[&str]) -> RrSet {
        RrSet {
            name: name.into(),
            rtype: t.into(),
            ttl,
            records: contents
                .iter()
                .map(|c| Record {
                    content: (*c).into(),
                    disabled: false,
                })
                .collect(),
            comments: vec![],
        }
    }

    #[test]
    fn diff_detects_add_change_delete_and_ignores_order() {
        let before = vec![
            rr("www.a.test.", "A", 300, &["192.0.2.1", "192.0.2.2"]),
            rr("old.a.test.", "A", 300, &["192.0.2.9"]),
            rr("same.a.test.", "TXT", 60, &["\"x\""]),
        ];
        let after = vec![
            rr("www.a.test.", "A", 600, &["192.0.2.2", "192.0.2.1"]),
            rr("new.a.test.", "AAAA", 300, &["2001:db8::1"]),
            rr("same.a.test.", "TXT", 60, &["\"x\""]),
        ];
        let d = diff_rrsets(&before, &after);
        assert_eq!(d.len(), 3);
        let names: Vec<_> = d.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["new.a.test.", "old.a.test.", "www.a.test."]);
        assert!(d[0].before.is_none() && d[0].after.is_some());
        assert!(d[1].before.is_some() && d[1].after.is_none());
        assert_eq!(d[2].after.as_ref().expect("after").ttl, 600);
    }

    #[test]
    fn redacts_nested_secrets() {
        let v = json!({"name": "k", "key": "c2VjcmV0", "items": [{"privatekey": "x", "ok": 1}]});
        let r = redact(&v);
        assert_eq!(r["key"], "[redacted]");
        assert_eq!(r["items"][0]["privatekey"], "[redacted]");
        assert_eq!(r["items"][0]["ok"], 1);
        assert_eq!(r["name"], "k");
    }

    #[test]
    fn operation_names() {
        let seg = |p: &'static str| -> Vec<&'static str> { p.split('/').collect() };
        let zone = |rest: &[&str]| RouteClass::Zone {
            zone: crate::pdns::ZoneName::from_name("a.test").expect("z"),
            variant: None,
            rest: rest.iter().map(|s| (*s).to_owned()).collect(),
        };
        assert_eq!(
            operation_name(&Method::PATCH, &zone(&[]), &[]),
            "rrsets.patch"
        );
        assert_eq!(
            operation_name(&Method::DELETE, &zone(&[]), &[]),
            "zone.delete"
        );
        assert_eq!(
            operation_name(&Method::PUT, &zone(&["notify"]), &[]),
            "zone.notify"
        );
        assert_eq!(
            operation_name(&Method::POST, &zone(&["cryptokeys"]), &[]),
            "cryptokey.create"
        );
        assert_eq!(
            operation_name(
                &Method::POST,
                &RouteClass::ServerAdmin,
                &seg("v1/servers/localhost/tsigkeys")
            ),
            "tsigkey.create"
        );
        assert_eq!(
            operation_name(
                &Method::PUT,
                &RouteClass::ServerAdmin,
                &seg("v1/servers/localhost/cache/flush")
            ),
            "cache.flush"
        );
        assert_eq!(
            operation_name(&Method::POST, &RouteClass::ZoneCreate, &[]),
            "zone.create"
        );
    }

    #[test]
    fn zone_field_diff() {
        let z: Zone = serde_json::from_value(
            json!({"id": "a.test.", "name": "a.test.", "kind": "Native", "account": ""}),
        )
        .expect("z");
        let mut z2 = z.clone();
        z2.kind = "Master".into();
        let d = diff_zone_fields(&z, &z2);
        assert_eq!(
            d,
            vec![json!({"field": "kind", "before": "Native", "after": "Master"})]
        );
    }
}
