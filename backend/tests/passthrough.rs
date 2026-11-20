//! Integration tests for the `/api` passthrough against a mock PowerDNS.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::Harness;
use serde_json::{Value, json};
use soabox::store::{HistoryFilter, Role};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const ZONES: &str = r#"[{"account": "", "id": "a.test.", "kind": "Native", "name": "a.test.", "serial": 1, "url": "/api/v1/servers/localhost/zones/a.test."}, {"account": "", "id": "b.test.", "kind": "Native", "name": "b.test.", "serial": 1, "url": "/api/v1/servers/localhost/zones/b.test."}]"#;

fn json_ok(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .insert_header("x-frame-options", "deny")
        .set_body_string(body)
}

/// Returns successive responses on each call.
struct Sequence {
    calls: Arc<AtomicUsize>,
    bodies: Vec<String>,
}

impl Respond for Sequence {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let body = &self.bodies[n.min(self.bodies.len() - 1)];
        json_ok(body)
    }
}

#[tokio::test]
async fn missing_and_unknown_keys_match_powerdns() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    for key in [None, Some("bogus"), Some("real-pdns-key")] {
        let (status, headers, body) = h.call("GET", "/api/v1/servers", key, "").await;
        assert_eq!(status, 401, "key {key:?}");
        assert_eq!(body, r#"{"error": "Unauthorized"}"#);
        assert_eq!(headers["www-authenticate"], "X-API-Key realm=\"PowerDNS\"");
        assert_eq!(headers["content-type"], "application/json");
    }
    assert!(pdns.received_requests().await.expect("reqs").is_empty());
}

#[tokio::test]
async fn admin_key_is_swapped_and_headers_copied() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/statistics"))
        .and(header("x-api-key", "real-pdns-key"))
        .and(query_param("statistic", "uptime"))
        .respond_with(json_ok(r#"[{"name": "uptime"}]"#))
        .expect(1)
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Admin, &[], &[], None).await;
    let (status, headers, body) = h
        .call(
            "GET",
            "/api/v1/servers/localhost/statistics?statistic=uptime",
            Some(&key),
            "",
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body, r#"[{"name": "uptime"}]"#);
    assert_eq!(headers["x-frame-options"], "deny");
    assert!(headers.get("content-security-policy").is_none());
    let reqs = pdns.received_requests().await.expect("reqs");
    assert_eq!(reqs[0].headers["x-api-key"], "real-pdns-key");
}

#[tokio::test]
async fn scoped_key_sees_only_its_zones() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(json_ok(ZONES))
        .mount(&pdns)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/search-data"))
        .respond_with(json_ok(
            r#"[{"name": "a.test.", "object_type": "zone", "zone_id": "a.test."}, {"name": "b.test.", "object_type": "zone", "zone_id": "b.test."}, {"content": "x", "name": "www.b.test.", "object_type": "record", "zone": "b.test.", "zone_id": "b.test."}]"#,
        ))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Scoped, &["a.test."], &[], None).await;

    let (status, _, body) = h
        .call("GET", "/api/v1/servers/localhost/zones", Some(&key), "")
        .await;
    assert_eq!(status, 200);
    assert_eq!(
        body,
        r#"[{"account": "", "id": "a.test.", "kind": "Native", "name": "a.test.", "serial": 1, "url": "/api/v1/servers/localhost/zones/a.test."}]"#
    );

    let (_, _, body) = h
        .call(
            "GET",
            "/api/v1/servers/localhost/search-data?q=*",
            Some(&key),
            "",
        )
        .await;
    let v: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(v.as_array().expect("array").len(), 1);
}

#[tokio::test]
async fn account_scope_includes_account_zones() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(json_ok(ZONES))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let acct = h
        .state
        .store
        .create_account("acme", "")
        .await
        .expect("acct");
    h.state
        .store
        .set_zone_account("b.test.", acct)
        .await
        .expect("map");
    let key = h.key(Role::Scoped, &[], &[acct], None).await;
    let (_, _, body) = h
        .call("GET", "/api/v1/servers/localhost/zones", Some(&key), "")
        .await;
    let v: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(v[0]["id"], "b.test.");
    assert_eq!(v.as_array().expect("array").len(), 1);
}

#[tokio::test]
async fn out_of_scope_zone_looks_missing_and_is_not_forwarded() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Scoped, &["a.test."], &[], None).await;
    for (m, uri) in [
        ("GET", "/api/v1/servers/localhost/zones/b.test."),
        ("PATCH", "/api/v1/servers/localhost/zones/B.TEST"),
        ("DELETE", "/api/v1/servers/localhost/zones/b.test."),
        ("GET", "/api/v1/servers/localhost/zones/b.test./export"),
        ("GET", "/api/v1/servers/localhost/zones/b.test..view1"),
        ("GET", "/api/v1/servers/other/zones/a.test."),
    ] {
        let (status, headers, body) = h.call(m, uri, Some(&key), "{}").await;
        assert_eq!(status, 404, "{m} {uri}");
        assert_eq!(body, r#"{"error": "Not Found"}"#);
        assert!(headers.get("www-authenticate").is_none());
    }
    assert!(pdns.received_requests().await.expect("reqs").is_empty());
}

#[tokio::test]
async fn scoped_key_cannot_use_server_endpoints() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Scoped, &["a.test."], &[], None).await;
    let op = h.key(Role::Operator, &[], &[], None).await;
    for uri in [
        "/api/v1/servers/localhost/tsigkeys",
        "/api/v1/servers/localhost/config",
        "/api/v1/servers/localhost/statistics",
        "/api/v1/servers/localhost/unknown",
    ] {
        let (status, _, body) = h.call("GET", uri, Some(&key), "").await;
        assert_eq!(status, 401, "{uri}");
        assert_eq!(body, r#"{"error": "Unauthorized"}"#);
    }
    let (status, _, _) = h
        .call("GET", "/api/v1/servers/localhost/tsigkeys", Some(&op), "")
        .await;
    assert_eq!(status, 401);
    let (status, _, _) = h
        .call(
            "POST",
            "/api/v1/servers/localhost/zones",
            Some(&key),
            r#"{"name": "c.test."}"#,
        )
        .await;
    assert_eq!(status, 401);
    assert!(pdns.received_requests().await.expect("reqs").is_empty());
}

#[tokio::test]
async fn patch_is_audited_with_diff() {
    let pdns = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones/a.test."))
        .and(query_param("rrset_name", "www.a.test."))
        .respond_with(Sequence {
            calls: calls.clone(),
            bodies: vec![
                r#"{"id": "a.test.", "name": "a.test.", "rrsets": []}"#.into(),
                r#"{"id": "a.test.", "name": "a.test.", "rrsets": [{"name": "www.a.test.", "type": "A", "ttl": 300, "records": [{"content": "192.0.2.1", "disabled": false}]}]}"#.into(),
            ],
        })
        .mount(&pdns)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/servers/localhost/zones/a.test."))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Scoped, &["a.test."], &[], None).await;
    let body = r#"{"rrsets": [{"name": "www.a.test.", "type": "A", "ttl": 300, "changetype": "REPLACE", "records": [{"content": "192.0.2.1", "disabled": false}]}]}"#;
    let (status, _, resp) = h
        .call(
            "PATCH",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            body,
        )
        .await;
    assert_eq!(status, 204);
    assert_eq!(resp, "");
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.operation, "rrsets.patch");
    assert_eq!(r.zone_name.as_deref(), Some("a.test."));
    assert_eq!(r.outcome, "success");
    assert_eq!(r.actor_type, "api_key");
    assert_eq!(r.client_ip, "127.0.0.1");
    let diff = r.diff.as_ref().expect("diff");
    assert_eq!(diff[0]["name"], "www.a.test.");
    assert_eq!(diff[0]["before"], Value::Null);
    assert_eq!(diff[0]["after"]["records"][0]["content"], "192.0.2.1");
}

#[tokio::test]
async fn failed_write_is_recorded_as_failure() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(json_ok(
            r#"{"id": "a.test.", "name": "a.test.", "rrsets": []}"#,
        ))
        .mount(&pdns)
        .await;
    Mock::given(method("PATCH"))
        .respond_with(
            ResponseTemplate::new(422)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"error": "RRset x.other. IN A: Name is out of zone"}"#),
        )
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Admin, &[], &[], None).await;
    let (status, _, body) = h
        .call(
            "PATCH",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            r#"{"rrsets": [{"name": "x.other.", "type": "A", "changetype": "REPLACE", "records": []}]}"#,
        )
        .await;
    assert_eq!(status, 422);
    assert_eq!(
        body,
        r#"{"error": "RRset x.other. IN A: Name is out of zone"}"#
    );
    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(rows[0].outcome, "failure");
    assert_eq!(rows[0].upstream_status, Some(422));
    assert!(rows[0].diff.is_none());
    assert_eq!(
        rows[0].detail.as_ref().expect("detail")["error"],
        "RRset x.other. IN A: Name is out of zone"
    );
}

#[tokio::test]
async fn scoped_zone_create_sets_account_and_scope() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(json_ok(ZONES))
        .mount(&pdns)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(|req: &Request| {
            let v: Value = serde_json::from_slice(&req.body).expect("json");
            assert_eq!(v["account"], "acme");
            ResponseTemplate::new(201)
                .insert_header("content-type", "application/json")
                .set_body_string(
                    json!({"id": "new.test.", "name": "new.test.", "account": "acme", "rrsets": [{"name": "new.test.", "type": "SOA", "ttl": 3600, "records": [{"content": "ns. h. 1 2 3 4 5", "disabled": false}]}]})
                        .to_string(),
                )
        })
        .expect(1)
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let acct = h
        .state
        .store
        .create_account("acme", "")
        .await
        .expect("acct");
    let key = h.key(Role::Scoped, &[], &[], Some(acct)).await;
    let (status, _, _) = h
        .call(
            "POST",
            "/api/v1/servers/localhost/zones",
            Some(&key),
            r#"{"name": "NEW.test", "kind": "Native", "account": "someone-else"}"#,
        )
        .await;
    assert_eq!(status, 201);
    assert_eq!(
        h.state.store.zone_account("new.test.").await.expect("q"),
        Some(acct)
    );
    let keys = h.state.store.list_api_keys(None).await.expect("keys");
    assert_eq!(keys[0].zones, vec!["new.test."]);
    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(rows[0].operation, "zone.create");
    assert_eq!(rows[0].zone_name.as_deref(), Some("new.test."));
}

#[tokio::test]
async fn zone_delete_cleans_up_references() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(json_ok(r#"{"id": "a.test.", "name": "a.test.", "rrsets": [{"name": "a.test.", "type": "NS", "ttl": 3600, "records": [{"content": "ns1.", "disabled": false}]}]}"#))
        .mount(&pdns)
        .await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let acct = h
        .state
        .store
        .create_account("acme", "")
        .await
        .expect("acct");
    h.state
        .store
        .set_zone_account("a.test.", acct)
        .await
        .expect("map");
    let key = h.key(Role::Operator, &[], &[], None).await;
    let (status, _, _) = h
        .call(
            "DELETE",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            "",
        )
        .await;
    assert_eq!(status, 204);
    assert_eq!(
        h.state.store.zone_account("a.test.").await.expect("q"),
        None
    );
    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(rows[0].operation, "zone.delete");
    assert_eq!(rows[0].diff.as_ref().expect("diff")[0]["type"], "NS");
}

#[tokio::test]
async fn server_admin_writes_are_redacted() {
    let pdns = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/servers/localhost/tsigkeys"))
        .respond_with(ResponseTemplate::new(201).set_body_string("{}"))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let key = h.key(Role::Admin, &[], &[], None).await;
    let (status, _, _) = h
        .call(
            "POST",
            "/api/v1/servers/localhost/tsigkeys",
            Some(&key),
            r#"{"name": "k1", "algorithm": "hmac-sha256", "key": "c2VjcmV0c2VjcmV0"}"#,
        )
        .await;
    assert_eq!(status, 201);
    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(rows[0].operation, "tsigkey.create");
    let detail = rows[0].detail.as_ref().expect("detail");
    assert_eq!(detail["key"], "[redacted]");
    assert_eq!(detail["name"], "k1");
}

#[tokio::test]
async fn unreachable_pdns_is_bad_gateway() {
    let h = Harness::new("http://127.0.0.1:9").await;
    let key = h.key(Role::Admin, &[], &[], None).await;
    let (status, _, body) = h.call("GET", "/api/v1/servers", Some(&key), "").await;
    assert_eq!(status, 502);
    assert_eq!(body, r#"{"error": "Bad Gateway"}"#);
    let (status, _, body) = h.call("GET", "/readyz", None, "").await;
    assert_eq!(status, 503);
    assert!(body.contains("\"pdns\":\"failed\""));
}

#[tokio::test]
async fn repeated_bad_keys_are_rate_limited() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    for _ in 0..20 {
        let (status, _, _) = h.call("GET", "/api/v1/servers", Some("bad"), "").await;
        assert_eq!(status, 401);
    }
    let (status, _, body) = h.call("GET", "/api/v1/servers", Some("bad"), "").await;
    assert_eq!(status, 429);
    assert_eq!(body, r#"{"error": "Too Many Requests"}"#);
}

#[tokio::test]
async fn health_endpoints_and_security_headers() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost"))
        .respond_with(json_ok("{}"))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let (status, headers, body) = h.call("GET", "/healthz", None, "").await;
    assert_eq!(status, 200);
    assert_eq!(body, r#"{"status":"ok"}"#);
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert!(
        headers["content-security-policy"]
            .to_str()
            .expect("csp")
            .contains("default-src 'self'")
    );
    assert!(headers.get("x-request-id").is_some());
    let (status, _, _) = h.call("GET", "/readyz", None, "").await;
    assert_eq!(status, 200);
    let (status, _, body) = h.call("GET", "/version", None, "").await;
    assert_eq!(status, 200);
    assert!(body.contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn options_preflight_is_forwarded_without_key() {
    let pdns = MockServer::start().await;
    Mock::given(method("OPTIONS"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("access-control-allow-headers", "Content-Type, X-API-Key"),
        )
        .expect(1)
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let (status, headers, _) = h.call("OPTIONS", "/api/v1/servers", None, "").await;
    assert_eq!(status, 200);
    assert_eq!(
        headers["access-control-allow-headers"],
        "Content-Type, X-API-Key"
    );
}

#[tokio::test]
async fn dot_segment_traversal_is_refused() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let scoped = h.key(Role::Scoped, &["a.test."], &[], None).await;
    let operator = h.key(Role::Operator, &[], &[], None).await;
    for key in [&scoped, &operator] {
        for uri in [
            "/api/v1/servers/localhost/zones/a.test./../../tsigkeys",
            "/api/v1/servers/localhost/zones/a.test./%2e%2e/%2e%2e/tsigkeys",
            "/api/v1/servers/localhost/zones/a.test.%2F..%2F..%2Ftsigkeys",
            "/api/v1/servers/localhost/zones/a.test.%5C..%5C..%5Ctsigkeys",
            "/api/v1/servers/localhost/zones/a.test./../b.test.",
        ] {
            let (status, _, body) = h.call("GET", uri, Some(key), "").await;
            assert_eq!(status, 404, "{uri}");
            assert_eq!(body, r#"{"error": "Not Found"}"#);
        }
    }
    assert!(
        pdns.received_requests().await.expect("reqs").is_empty(),
        "nothing reached PowerDNS"
    );
}

async fn owned_key(
    h: &Harness,
    owner_role: soabox::store::UserRole,
    key_role: Role,
    zones: &[&str],
) -> (String, i64) {
    let uid = h
        .state
        .store
        .create_user(&soabox::store::NewUser {
            username: format!("owner-{}", uuid::Uuid::new_v4().simple()),
            display_name: String::new(),
            email: String::new(),
            role: owner_role,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: None,
            status: soabox::store::UserStatus::Active,
        })
        .await
        .expect("user");
    h.state
        .store
        .set_user_access(
            uid,
            &zones.iter().map(|z| (*z).to_owned()).collect::<Vec<_>>(),
            &[],
        )
        .await
        .expect("access");
    let g = soabox::keys::generate().expect("gen");
    h.state
        .store
        .insert_api_key(&soabox::store::NewApiKey {
            key_hash: g.hash,
            hint: g.hint,
            role: Some(key_role),
            owner_user_id: Some(uid),
            zones: zones.iter().map(|z| (*z).to_owned()).collect(),
            ..Default::default()
        })
        .await
        .expect("key");
    (g.secret, uid)
}

#[tokio::test]
async fn keys_follow_their_owner() {
    use soabox::store::{UserRole, UserStatus};
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(json_ok("[]"))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let (key, uid) = owned_key(&h, UserRole::Admin, Role::Admin, &[]).await;
    let (s, _, _) = h
        .call("GET", "/api/v1/servers/localhost/tsigkeys", Some(&key), "")
        .await;
    assert_eq!(s, 200);

    h.state
        .store
        .update_user(uid, "", "", UserRole::User, UserStatus::Active)
        .await
        .expect("demote");
    let (s, _, _) = h
        .call("GET", "/api/v1/servers/localhost/tsigkeys", Some(&key), "")
        .await;
    assert_eq!(s, 401, "a demoted owner's admin key loses admin reach");
    let (s, _, body) = h
        .call(
            "GET",
            "/api/v1/servers/localhost/zones/x.test.",
            Some(&key),
            "",
        )
        .await;
    assert_eq!((s, body.as_str()), (404, r#"{"error": "Not Found"}"#));

    h.state
        .store
        .update_user(uid, "", "", UserRole::Admin, UserStatus::Disabled)
        .await
        .expect("disable");
    let (s, _, _) = h.call("GET", "/api/v1/servers", Some(&key), "").await;
    assert_eq!(s, 401, "a disabled owner's keys stop working");
}

#[tokio::test]
async fn user_owned_keys_follow_user_policy() {
    use soabox::store::UserRole;
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let (key, _) = owned_key(&h, UserRole::User, Role::Scoped, &["a.test."]).await;
    let (s, _, _) = h
        .call(
            "DELETE",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            "",
        )
        .await;
    assert_eq!(s, 401, "user_can_delete_zones is off");
    let lua = r#"{"rrsets": [{"name": "x.a.test.", "type": "LUA", "ttl": 60, "changetype": "REPLACE", "records": [{"content": "A \"1.2.3.4\"", "disabled": false}]}]}"#;
    let (s, _, body) = h
        .call(
            "PATCH",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            lua,
        )
        .await;
    assert_eq!(s, 422);
    assert!(body.contains("not allowed"));
    let (s, _, _) = h
        .call(
            "PUT",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            r#"{"dnssec": true}"#,
        )
        .await;
    assert_eq!(s, 401, "user_can_manage_dnssec is off");
    let (s, _, _) = h
        .call(
            "PUT",
            "/api/v1/servers/localhost/zones/a.test.",
            Some(&key),
            r#"{"account": "other"}"#,
        )
        .await;
    assert_eq!(s, 401, "scoped keys cannot move zones between accounts");
    assert!(pdns.received_requests().await.expect("reqs").is_empty());
    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert!(
        rows.iter().all(|r| r.outcome == "failure"),
        "refusals are audited"
    );
}

#[tokio::test]
async fn risky_metadata_and_secondaries_are_admin_only() {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(json_ok(ZONES))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let operator = h.key(Role::Operator, &[], &[], None).await;
    let acct = h
        .state
        .store
        .create_account("acme", "")
        .await
        .expect("acct");
    let scoped = h.key(Role::Scoped, &["a.test."], &[], Some(acct)).await;
    for kind in [
        "LUA-AXFR-SCRIPT",
        "AXFR-SOURCE",
        "ALSO-NOTIFY",
        "FORWARD-DNSUPDATE",
    ] {
        let (s, _, _) = h
            .call(
                "PUT",
                &format!("/api/v1/servers/localhost/zones/a.test./metadata/{kind}"),
                Some(&operator),
                r#"{"metadata": ["x"]}"#,
            )
            .await;
        assert_eq!(s, 401, "{kind}");
    }
    let (s, _, _) = h
        .call(
            "POST",
            "/api/v1/servers/localhost/zones/a.test./metadata",
            Some(&scoped),
            r#"{"kind": "LUA-AXFR-SCRIPT", "metadata": ["x"]}"#,
        )
        .await;
    assert_eq!(s, 401);
    let (s, _, _) = h
        .call(
            "POST",
            "/api/v1/servers/localhost/zones",
            Some(&scoped),
            r#"{"name": "new.test.", "kind": "Slave", "masters": ["10.0.0.1"]}"#,
        )
        .await;
    assert_eq!(s, 401, "scoped keys cannot create secondaries");
    let (s, _, body) = h
        .call(
            "POST",
            "/api/v1/servers/localhost/zones",
            Some(&scoped),
            r#"{"name": "sub.b.test.", "kind": "Native"}"#,
        )
        .await;
    assert_eq!(s, 422, "cannot create a zone below another tenant's zone");
    assert!(body.contains("overlaps"));
    let posts = pdns
        .received_requests()
        .await
        .expect("reqs")
        .iter()
        .filter(|r| r.method.as_str() != "GET")
        .count();
    assert_eq!(posts, 0, "no write reached PowerDNS");
}
