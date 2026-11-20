//! Compatibility suite: the same request sent to a real PowerDNS directly and
//! through SOAbox must produce the same status, content type, and body.
//!
//! Runs only when `SOABOX_COMPAT_PDNS_URL` and `SOABOX_COMPAT_PDNS_KEY` are
//! set, for example against `docker compose up pdns`.

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Harness, config_vars};
use http_body_util::BodyExt;
use serde_json::Value;
use soabox::store::Role;

struct Compat {
    h: Harness,
    direct: reqwest::Client,
    base: String,
    pdns_key: String,
    admin: String,
}

#[derive(Debug, PartialEq)]
struct Answer {
    status: u16,
    content_type: Option<String>,
    body: String,
}

fn env() -> Option<(String, String)> {
    Some((
        std::env::var("SOABOX_COMPAT_PDNS_URL").ok()?,
        std::env::var("SOABOX_COMPAT_PDNS_KEY").ok()?,
    ))
}

impl Compat {
    async fn new() -> Option<Self> {
        let (base, key) = env()?;
        let mut vars = config_vars(&base);
        vars.insert("SOABOX_PDNS_API_KEY".into(), key.clone());
        let h = Harness::with_vars(vars).await;
        let admin = h.key(Role::Admin, &[], &[], None).await;
        Some(Self {
            h,
            direct: reqwest::Client::new(),
            base,
            pdns_key: key,
            admin,
        })
    }

    async fn direct(&self, method: &str, uri: &str, body: &str) -> Answer {
        let resp = self
            .direct
            .request(
                method.parse().expect("method"),
                format!("{}{uri}", self.base),
            )
            .header("X-API-Key", &self.pdns_key)
            .header("content-type", "application/json")
            .body(body.to_owned())
            .send()
            .await
            .expect("direct request");
        Answer {
            status: resp.status().as_u16(),
            content_type: resp
                .headers()
                .get("content-type")
                .map(|v| v.to_str().unwrap_or_default().to_owned()),
            body: resp.text().await.expect("text"),
        }
    }

    async fn soabox_with(&self, key: &str, method: &str, uri: &str, body: &str) -> Answer {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-api-key", key)
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .expect("request");
        let resp = self.h.send(req).await;
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap_or_default().to_owned());
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        Answer {
            status,
            content_type,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    async fn soabox(&self, method: &str, uri: &str, body: &str) -> Answer {
        self.soabox_with(&self.admin, method, uri, body).await
    }

    async fn same(&self, method: &str, uri: &str, body: &str) {
        let d = self.direct(method, uri, body).await;
        let s = self.soabox(method, uri, body).await;
        assert_eq!(d, s, "{method} {uri}");
    }

    async fn cleanup(&self, zones: &[&str]) {
        for z in zones {
            let _ = self
                .direct(
                    "DELETE",
                    &format!("/api/v1/servers/localhost/zones/{z}"),
                    "",
                )
                .await;
        }
    }
}

macro_rules! compat_or_skip {
    () => {
        match Compat::new().await {
            Some(c) => c,
            None => {
                eprintln!("SOABOX_COMPAT_PDNS_URL not set, skipping");
                return;
            }
        }
    };
}

fn normalize_zone(body: &str, from: &str, to: &str) -> Value {
    let mut v: Value = serde_json::from_str(&body.replace(from, to)).expect("json");
    if let Some(obj) = v.as_object_mut() {
        for k in ["serial", "edited_serial", "notified_serial", "last_check"] {
            obj.remove(k);
        }
        if let Some(Value::Array(rrsets)) = obj.get_mut("rrsets") {
            for r in rrsets.iter_mut() {
                if r["type"] == "SOA" {
                    r["records"] = Value::Null;
                }
                if let Some(Value::Array(comments)) = r.get_mut("comments") {
                    for c in comments {
                        if let Some(o) = c.as_object_mut() {
                            o.remove("modified_at");
                        }
                    }
                }
            }
            rrsets.sort_by_key(|r| format!("{}{}", r["name"], r["type"]));
        }
    }
    v
}

#[tokio::test]
async fn discovery_and_reads_match() {
    let c = compat_or_skip!();
    c.cleanup(&["read.compat."]).await;
    c.direct(
        "POST",
        "/api/v1/servers/localhost/zones",
        r#"{"name": "read.compat.", "kind": "Native", "nameservers": ["ns1.read.compat."], "rrsets": [{"name": "www.read.compat.", "type": "A", "ttl": 300, "records": [{"content": "192.0.2.1", "disabled": false}]}]}"#,
    )
    .await;
    for uri in [
        "/api",
        "/api/v1",
        "/api/v1/servers",
        "/api/v1/servers/localhost",
        "/api/v1/servers/localhost/zones",
        "/api/v1/servers/localhost/zones?zone=read.compat.",
        "/api/v1/servers/localhost/zones/read.compat.",
        "/api/v1/servers/localhost/zones/read.compat",
        "/api/v1/servers/localhost/zones/READ.compat.",
        "/api/v1/servers/localhost/zones/read.compat.?rrsets=false",
        "/api/v1/servers/localhost/zones/read.compat.?rrset_name=www.read.compat.&rrset_type=A",
        "/api/v1/servers/localhost/zones/read.compat./export",
        "/api/v1/servers/localhost/zones/read.compat./cryptokeys",
        "/api/v1/servers/localhost/zones/read.compat./metadata",
        "/api/v1/servers/localhost/search-data?q=*read.compat*&max=100",
        "/api/v1/servers/localhost/config",
        "/api/v1/servers/localhost/tsigkeys",
        "/api/v1/servers/localhost/autoprimaries",
        "/api/v1/servers/localhost/views",
        "/api/v1/servers/localhost/networks",
    ] {
        c.same("GET", uri, "").await;
    }
    c.cleanup(&["read.compat."]).await;
}

#[tokio::test]
async fn errors_match() {
    let c = compat_or_skip!();
    for (m, uri, body) in [
        ("GET", "/api/v1/servers/localhost/zones/missing.compat.", ""),
        ("GET", "/api/v1/servers/other", ""),
        ("GET", "/api/v1/servers/other/zones", ""),
        ("GET", "/api/v1/servers/localhost/bogus", ""),
        ("PUT", "/api/v1/servers", ""),
        ("POST", "/api/v1/servers/localhost/zones", "{bad"),
        (
            "PATCH",
            "/api/v1/servers/localhost/zones/missing.compat.",
            r#"{"rrsets": []}"#,
        ),
    ] {
        c.same(m, uri, body).await;
    }
    let d = c
        .direct("GET", "/api/v1/servers/localhost/zones/missing.compat.", "")
        .await;
    let s = c
        .soabox_with("not-a-key", "GET", "/api/v1/servers/localhost/zones/x.", "")
        .await;
    assert_eq!(s.status, 401);
    let unauth = reqwest::Client::new()
        .get(format!("{}/api/v1/servers", c.base))
        .send()
        .await
        .expect("req");
    assert_eq!(unauth.status().as_u16(), s.status);
    assert_eq!(unauth.text().await.expect("text"), s.body);
    assert_eq!(d.status, 404);
}

#[tokio::test]
async fn scoped_out_of_scope_matches_missing_zone() {
    let c = compat_or_skip!();
    c.cleanup(&["hidden.compat."]).await;
    c.direct(
        "POST",
        "/api/v1/servers/localhost/zones",
        r#"{"name": "hidden.compat.", "kind": "Native", "nameservers": []}"#,
    )
    .await;
    let scoped = c.h.key(Role::Scoped, &["other.compat."], &[], None).await;
    let missing = c
        .direct("GET", "/api/v1/servers/localhost/zones/missing.compat.", "")
        .await;
    let hidden = c
        .soabox_with(
            &scoped,
            "GET",
            "/api/v1/servers/localhost/zones/hidden.compat.",
            "",
        )
        .await;
    assert_eq!(missing, hidden);
    let list = c
        .soabox_with(&scoped, "GET", "/api/v1/servers/localhost/zones", "")
        .await;
    assert_eq!(list.body, "[]");
    c.cleanup(&["hidden.compat."]).await;
}

#[tokio::test]
async fn writes_produce_identical_zones() {
    let c = compat_or_skip!();
    let a = "twin-a.compat.";
    let b = "twin-b.compat.";
    c.cleanup(&[a, b]).await;

    let create = |z: &str| {
        format!(
            r#"{{"name": "{z}", "kind": "Native", "nameservers": ["ns1.{z}"], "soa_edit_api": "DEFAULT"}}"#
        )
    };
    let d = c
        .direct("POST", "/api/v1/servers/localhost/zones", &create(a))
        .await;
    let s = c
        .soabox("POST", "/api/v1/servers/localhost/zones", &create(b))
        .await;
    assert_eq!(d.status, s.status);
    assert_eq!(d.content_type, s.content_type);
    assert_eq!(
        normalize_zone(&d.body, a, "Z."),
        normalize_zone(&s.body, b, "Z.")
    );

    let patches = |z: &str| {
        vec![
            format!(
                r#"{{"rrsets": [{{"name": "www.{z}", "type": "A", "ttl": 300, "changetype": "REPLACE", "records": [{{"content": "192.0.2.1", "disabled": false}}, {{"content": "192.0.2.2", "disabled": true}}], "comments": [{{"content": "web", "account": "ops"}}]}}]}}"#
            ),
            format!(
                r#"{{"rrsets": [{{"name": "txt.{z}", "type": "TXT", "ttl": 60, "changetype": "REPLACE", "records": [{{"content": "\"hello\"", "disabled": false}}]}}]}}"#
            ),
            format!(
                r#"{{"rrsets": [{{"name": "www.{z}", "type": "A", "changetype": "DELETE"}}]}}"#
            ),
            r#"{"rrsets": [{"name": "bad.example.", "type": "A", "ttl": 1, "changetype": "REPLACE", "records": [{"content": "1.2.3.4", "disabled": false}]}]}"#.to_string(),
        ]
    };
    for (pa, pb) in patches(a).into_iter().zip(patches(b)) {
        let d = c
            .direct(
                "PATCH",
                &format!("/api/v1/servers/localhost/zones/{a}"),
                &pa,
            )
            .await;
        let s = c
            .soabox(
                "PATCH",
                &format!("/api/v1/servers/localhost/zones/{b}"),
                &pb,
            )
            .await;
        assert_eq!(
            (d.status, d.content_type.clone()),
            (s.status, s.content_type.clone())
        );
        assert_eq!(d.body.replace(a, "Z."), s.body.replace(b, "Z."));
    }

    let put = r#"{"kind": "Master"}"#;
    let d = c
        .direct("PUT", &format!("/api/v1/servers/localhost/zones/{a}"), put)
        .await;
    let s = c
        .soabox("PUT", &format!("/api/v1/servers/localhost/zones/{b}"), put)
        .await;
    assert_eq!(d, s);

    for sub in ["notify", "rectify"] {
        let d = c
            .direct(
                "PUT",
                &format!("/api/v1/servers/localhost/zones/{a}/{sub}"),
                "",
            )
            .await;
        let s = c
            .soabox(
                "PUT",
                &format!("/api/v1/servers/localhost/zones/{b}/{sub}"),
                "",
            )
            .await;
        assert_eq!(d, s, "{sub}");
    }

    let za = c
        .direct("GET", &format!("/api/v1/servers/localhost/zones/{a}"), "")
        .await;
    let zb = c
        .direct("GET", &format!("/api/v1/servers/localhost/zones/{b}"), "")
        .await;
    assert_eq!(
        normalize_zone(&za.body, a, "Z."),
        normalize_zone(&zb.body, b, "Z.")
    );

    let d = c
        .direct(
            "DELETE",
            &format!("/api/v1/servers/localhost/zones/{a}"),
            "",
        )
        .await;
    let s = c
        .soabox(
            "DELETE",
            &format!("/api/v1/servers/localhost/zones/{b}"),
            "",
        )
        .await;
    assert_eq!(d, s);

    let history =
        c.h.state
            .store
            .list_history(&soabox::store::HistoryFilter {
                zone: Some(b.into()),
                ..Default::default()
            })
            .await
            .expect("history");
    let ops: Vec<&str> = history.iter().map(|h| h.operation.as_str()).collect();
    assert_eq!(
        ops,
        vec![
            "zone.delete",
            "zone.rectify",
            "zone.notify",
            "zone.update",
            "rrsets.patch",
            "rrsets.patch",
            "rrsets.patch",
            "rrsets.patch",
            "zone.create"
        ]
    );
    assert_eq!(history[4].outcome, "failure");
}
