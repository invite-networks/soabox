//! Integration tests for `/auth` and `/v1` against a mock PowerDNS.

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Harness, config_vars};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use soabox::auth::crypto;
use soabox::store::{HistoryFilter, NewUser, UserRole, UserStatus};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ZONES: &str = r#"[{"id": "a.test.", "name": "a.test.", "kind": "Native", "serial": 1, "dnssec": false, "account": "", "masters": []}, {"id": "b.test.", "name": "b.test.", "kind": "Native", "serial": 2, "dnssec": true, "account": "", "masters": []}, {"id": "2.0.192.in-addr.arpa.", "name": "2.0.192.in-addr.arpa.", "kind": "Native", "serial": 1, "account": "", "masters": []}]"#;

struct Session {
    cookie: String,
    csrf: String,
}

struct Resp {
    status: u16,
    headers: axum::http::HeaderMap,
    body: Value,
    text: String,
}

async fn send(
    h: &Harness,
    m: &str,
    uri: &str,
    s: Option<&Session>,
    body: Option<Value>,
    csrf: bool,
) -> Resp {
    let mut b = Request::builder().method(m).uri(uri);
    if let Some(s) = s {
        b = b.header("cookie", &s.cookie);
        if csrf {
            b = b.header("x-csrf-token", &s.csrf);
        }
    }
    let req = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(v.to_string())),
        None => b.body(Body::empty()),
    }
    .expect("request");
    let resp = h.send(req).await;
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Resp {
        status,
        headers,
        body: serde_json::from_str(&text).unwrap_or(Value::Null),
        text,
    }
}

async fn call(h: &Harness, m: &str, uri: &str, s: &Session, body: Option<Value>) -> Resp {
    send(h, m, uri, Some(s), body, true).await
}

async fn user(h: &Harness, name: &str, role: UserRole, status: UserStatus) -> i64 {
    h.state
        .store
        .create_user(&NewUser {
            username: name.into(),
            display_name: name.into(),
            email: String::new(),
            role,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: Some(crypto::hash_password("correct-horse-battery").expect("hash")),
            status,
        })
        .await
        .expect("user")
}

async fn login(h: &Harness, name: &str) -> Session {
    let r = send(
        h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": name, "password": "correct-horse-battery"})),
        false,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let set = r.headers["set-cookie"].to_str().expect("cookie").to_owned();
    assert!(set.contains("HttpOnly"));
    assert!(set.contains("SameSite=Lax"));
    Session {
        cookie: set.split(';').next().expect("pair").to_owned(),
        csrf: r.body["csrf_token"].as_str().expect("csrf").to_owned(),
    }
}

async fn pdns_with_zones() -> MockServer {
    let pdns = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ZONES))
        .mount(&pdns)
        .await;
    pdns
}

#[tokio::test]
async fn login_session_and_csrf() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    user(&h, "admin", UserRole::Admin, UserStatus::Active).await;

    let r = send(&h, "GET", "/v1/me", None, None, false).await;
    assert_eq!(r.status, 401);
    assert_eq!(r.body["code"], "UNAUTHENTICATED");

    let s = login(&h, "admin").await;
    let r = call(&h, "GET", "/v1/me", &s, None).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["user"]["username"], "admin");
    assert_eq!(r.body["permissions"]["manage_users"], true);

    let r = send(
        &h,
        "PUT",
        "/v1/me",
        Some(&s),
        Some(json!({"display_name": "A", "email": ""})),
        false,
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(r.body["code"], "CSRF_FAILED");
    let r = call(
        &h,
        "PUT",
        "/v1/me",
        &s,
        Some(json!({"display_name": "Ada", "email": "ada@example.test"})),
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["user"]["display_name"], "Ada");

    let r = call(&h, "POST", "/auth/logout", &s, None).await;
    assert_eq!(r.status, 204);
    let r = call(&h, "GET", "/v1/me", &s, None).await;
    assert_eq!(r.status, 401);
}

#[tokio::test]
async fn bad_credentials_and_rate_limit() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    user(&h, "bob", UserRole::User, UserStatus::Active).await;
    for _ in 0..10 {
        let r = send(
            &h,
            "POST",
            "/auth/login",
            None,
            Some(json!({"username": "bob", "password": "wrong-password"})),
            false,
        )
        .await;
        assert_eq!(r.status, 401);
    }
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "bob", "password": "correct-horse-battery"})),
        false,
    )
    .await;
    assert_eq!(r.status, 429);
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "nobody", "password": "x"})),
        false,
    )
    .await;
    assert_eq!(r.status, 429, "same client address is blocked");
}

#[tokio::test]
async fn disabled_pending_and_sso_only() {
    let pdns = MockServer::start().await;
    let mut vars = config_vars(&pdns.uri());
    vars.insert(
        "SOABOX_OIDC_ISSUER".into(),
        "https://idp.example.test".into(),
    );
    vars.insert("SOABOX_OIDC_CLIENT_ID".into(), "c".into());
    vars.insert("SOABOX_OIDC_CLIENT_SECRET".into(), "s".into());
    let h = Harness::with_vars(vars).await;
    user(&h, "off", UserRole::User, UserStatus::Disabled).await;
    user(&h, "wait", UserRole::User, UserStatus::Pending).await;
    user(&h, "plain", UserRole::User, UserStatus::Active).await;
    user(&h, "boss", UserRole::Admin, UserStatus::Active).await;
    for name in ["off", "wait"] {
        let r = send(
            &h,
            "POST",
            "/auth/login",
            None,
            Some(json!({"username": name, "password": "correct-horse-battery"})),
            false,
        )
        .await;
        assert_eq!(r.status, 403, "{name}");
    }
    let cfg = send(&h, "GET", "/auth/config", None, None, false).await;
    assert_eq!(cfg.body["oidc_enabled"], true);
    assert_eq!(cfg.body["local_enabled"], true);
    login(&h, "plain").await;

    soabox::settings::Settings::apply(
        &h.state.store,
        soabox::settings::SettingsPatch {
            local_login_with_oidc: Some(soabox::settings::LocalLogin::Deny),
            ..Default::default()
        },
    )
    .await
    .expect("db")
    .expect("valid");
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "plain", "password": "correct-horse-battery"})),
        false,
    )
    .await;
    assert_eq!(r.status, 403);
    login(&h, "boss").await;
    let cfg = send(&h, "GET", "/auth/config", None, None, false).await;
    assert_eq!(cfg.body["local_enabled"], false);
}

#[tokio::test]
async fn totp_enrollment_and_sign_in() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let uid = user(&h, "tina", UserRole::User, UserStatus::Active).await;
    let s = login(&h, "tina").await;
    let r = call(&h, "POST", "/v1/me/totp/setup", &s, None).await;
    assert_eq!(r.status, 200);
    assert!(
        r.body["otpauth_url"]
            .as_str()
            .expect("url")
            .starts_with("otpauth://totp/")
    );
    let enc = h
        .state
        .store
        .get_user(uid)
        .await
        .expect("q")
        .expect("u")
        .totp_secret_enc
        .expect("enc");
    let secret =
        crypto::decrypt(crypto::PURPOSE_TOTP, &h.state.secret_keys(), &enc).expect("decrypt");
    assert_ne!(enc, secret, "stored encrypted");
    let now = u64::try_from(soabox::store::now()).expect("time");

    let bad = call(
        &h,
        "POST",
        "/v1/me/totp/enable",
        &s,
        Some(json!({"code": "000000"})),
    )
    .await;
    assert_eq!(bad.status, 422);
    let code = crypto::totp_at(&secret, now / 30);
    let r = call(
        &h,
        "POST",
        "/v1/me/totp/enable",
        &s,
        Some(json!({"code": code})),
    )
    .await;
    assert_eq!(r.status, 204);

    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "tina", "password": "correct-horse-battery"})),
        false,
    )
    .await;
    assert_eq!(r.status, 401);
    assert_eq!(r.body["code"], "TOTP_REQUIRED");
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "tina", "password": "correct-horse-battery", "totp_code": code})),
        false,
    )
    .await;
    assert_eq!(r.status, 401, "a code cannot be used twice");
    let next = crypto::totp_at(&secret, now / 30 + 1);
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "tina", "password": "correct-horse-battery", "totp_code": next})),
        false,
    )
    .await;
    assert_eq!(r.status, 200);

    // Wrong codes count against the account-wide limit, not only the client,
    // so guesses spread across many addresses still lock the account. The
    // replayed code above already counted once, so 98 more leaves one below
    // the limit of 100.
    for _ in 0..98 {
        h.state.login_users.fail(&["user:tina"]);
    }
    assert!(!h.state.login_users.blocked(&["user:tina"]));
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(
            json!({"username": "tina", "password": "correct-horse-battery", "totp_code": "000000"}),
        ),
        false,
    )
    .await;
    assert_eq!(r.status, 401);
    assert!(
        h.state.login_users.blocked(&["user:tina"]),
        "a wrong TOTP code reaches the account limiter"
    );
}

#[tokio::test]
async fn totp_steps_are_consumed_once() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let uid = user(&h, "once", UserRole::User, UserStatus::Active).await;
    let store = &h.state.store;
    let (a, b) = tokio::join!(
        store.consume_totp_step(uid, 42),
        store.consume_totp_step(uid, 42)
    );
    let wins = [a.expect("a"), b.expect("b")]
        .iter()
        .filter(|w| **w)
        .count();
    assert_eq!(wins, 1, "exactly one concurrent request consumes a step");
    assert!(!store.consume_totp_step(uid, 41).await.expect("older"));
    assert!(store.consume_totp_step(uid, 43).await.expect("newer"));
}

#[tokio::test]
async fn idle_sessions_expire() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    user(&h, "idle", UserRole::User, UserStatus::Active).await;
    let s = login(&h, "idle").await;
    let token = s.cookie.split_once('=').expect("pair").1;
    h.state
        .store
        .touch_session(&crypto::token_digest(token), 0)
        .await
        .expect("touch");
    let r = call(&h, "GET", "/v1/me", &s, None).await;
    assert_eq!(r.status, 401);
}

#[tokio::test]
async fn user_sees_and_edits_only_assigned_zones() {
    let pdns = pdns_with_zones().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones/a.test."))
        .and(query_param("rrset_name", "www.a.test."))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"id": "a.test.", "name": "a.test.", "rrsets": []}"#),
        )
        .mount(&pdns)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/servers/localhost/zones/a.test."))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    let uid = user(&h, "uma", UserRole::User, UserStatus::Active).await;
    h.state
        .store
        .set_user_access(uid, &["a.test.".into()], &[])
        .await
        .expect("access");
    let s = login(&h, "uma").await;

    let r = call(&h, "GET", "/v1/zones", &s, None).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["total"], 1);
    assert_eq!(r.body["items"][0]["name"], "a.test.");

    let r = call(&h, "GET", "/v1/zones/b.test.", &s, None).await;
    assert_eq!(r.status, 404);

    let patch = json!({"rrsets": [{"name": "www.a.test.", "type": "A", "ttl": 300, "changetype": "REPLACE", "records": [{"content": "192.0.2.5", "disabled": false}]}]});
    let r = call(&h, "PATCH", "/v1/zones/a.test./rrsets", &s, Some(patch)).await;
    assert_eq!(r.status, 204, "{}", r.text);

    let bad = json!({"rrsets": [{"name": "www.a.test.", "type": "LUA", "ttl": 300, "changetype": "REPLACE", "records": []}]});
    let r = call(&h, "PATCH", "/v1/zones/a.test./rrsets", &s, Some(bad)).await;
    assert_eq!(r.status, 422);
    let outside = json!({"rrsets": [{"name": "www.b.test.", "type": "A", "ttl": 300, "changetype": "REPLACE", "records": []}]});
    let r = call(&h, "PATCH", "/v1/zones/a.test./rrsets", &s, Some(outside)).await;
    assert_eq!(r.status, 422);

    let rows = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(rows[0].actor_type, "user");
    assert_eq!(rows[0].actor_name, "uma");
    assert_eq!(rows[0].operation, "rrsets.patch");

    for (m, uri) in [
        ("GET", "/v1/users"),
        ("GET", "/v1/settings"),
        ("GET", "/v1/tsigkeys"),
        ("GET", "/v1/server"),
        ("POST", "/v1/zones"),
        ("DELETE", "/v1/zones/a.test."),
    ] {
        let body = (m == "POST").then(|| json!({"name": "x.test", "kind": "Native"}));
        let r = call(&h, m, uri, &s, body).await;
        assert_eq!(r.status, 403, "{m} {uri}");
    }
    let r = call(&h, "GET", "/v1/history", &s, None).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["items"].as_array().expect("items").len(), 1);
}

#[tokio::test]
async fn admin_creates_zone_from_template_in_account() {
    let pdns = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/servers/localhost/zones"))
        .respond_with(|req: &wiremock::Request| {
            let v: Value = serde_json::from_slice(&req.body).expect("json");
            assert_eq!(v["name"], "new.test.");
            assert_eq!(v["account"], "acme");
            assert_eq!(v["nameservers"], json!([]));
            assert_eq!(v["rrsets"].as_array().expect("rrsets").len(), 2);
            ResponseTemplate::new(201).set_body_json(
                json!({"id": "new.test.", "name": "new.test.", "rrsets": v["rrsets"]}),
            )
        })
        .expect(1)
        .mount(&pdns)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/servers/localhost/zones/new.test."))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"id": "new.test.", "name": "new.test.", "kind": "Native", "rrsets": []}),
        ))
        .mount(&pdns)
        .await;
    let h = Harness::new(&pdns.uri()).await;
    user(&h, "admin", UserRole::Admin, UserStatus::Active).await;
    let s = login(&h, "admin").await;

    let r = call(
        &h,
        "POST",
        "/v1/accounts",
        &s,
        Some(json!({"name": "acme", "description": "Acme"})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.text);
    let acct = r.body["id"].as_i64().expect("id");
    let r = call(
        &h,
        "POST",
        "/v1/templates",
        &s,
        Some(json!({"name": "basic", "records": [
            {"name": "@", "type": "NS", "ttl": 3600, "content": "ns1.example.net."},
            {"name": "www", "type": "A", "ttl": 300, "content": "192.0.2.1"}
        ]})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.text);
    let tid = r.body["id"].as_i64().expect("id");

    let r = call(&h, "POST", "/v1/zones", &s, Some(json!({"name": "NEW.test", "kind": "Native", "nameservers": ["ns1.example.net."], "template_id": tid, "account_id": acct}))).await;
    assert_eq!(r.status, 201, "{}", r.text);
    assert_eq!(r.body["account"]["name"], "acme");
    assert_eq!(
        h.state.store.zone_account("new.test.").await.expect("q"),
        Some(acct)
    );
}

#[tokio::test]
async fn user_admin_and_last_admin_protection() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let admin_id = user(&h, "admin", UserRole::Admin, UserStatus::Active).await;
    let s = login(&h, "admin").await;
    let r = call(
        &h,
        "POST",
        "/v1/users",
        &s,
        Some(json!({"username": "Carol", "role": "operator", "password": "short"})),
    )
    .await;
    assert_eq!(r.status, 422);
    let r = call(
        &h,
        "POST",
        "/v1/users",
        &s,
        Some(json!({"username": "Carol", "role": "operator", "password": "long-enough-password"})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.text);
    assert_eq!(r.body["username"], "carol");
    let carol = r.body["id"].as_i64().expect("id");
    let r = call(
        &h,
        "POST",
        "/v1/users",
        &s,
        Some(json!({"username": "carol", "role": "user", "password": "long-enough-password"})),
    )
    .await;
    assert_eq!(r.status, 409);

    let r = call(
        &h,
        "PUT",
        &format!("/v1/users/{admin_id}"),
        &s,
        Some(json!({"display_name": "a", "email": "", "role": "user", "status": "active"})),
    )
    .await;
    assert_eq!(r.status, 409, "last admin cannot be demoted");
    let r = call(&h, "DELETE", &format!("/v1/users/{admin_id}"), &s, None).await;
    assert_eq!(r.status, 409);

    let r = call(&h, "PUT", &format!("/v1/users/{carol}"), &s, Some(json!({"display_name": "Carol", "email": "", "role": "user", "status": "disabled", "zones": ["a.test"]}))).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["zones"], json!(["a.test."]));
    let r = call(
        &h,
        "POST",
        &format!("/v1/users/{carol}/password"),
        &s,
        Some(json!({"password": "another-long-password"})),
    )
    .await;
    assert_eq!(r.status, 204);
    let r = call(&h, "GET", "/v1/users", &s, None).await;
    assert_eq!(r.body.as_array().expect("users").len(), 2);
    assert!(r.text.find("password_hash").is_none());
    let r = call(&h, "DELETE", &format!("/v1/users/{carol}"), &s, None).await;
    assert_eq!(r.status, 204);
}

#[tokio::test]
async fn user_api_keys_are_limited_to_their_zones() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let uid = user(&h, "kim", UserRole::User, UserStatus::Active).await;
    h.state
        .store
        .set_user_access(uid, &["a.test.".into()], &[])
        .await
        .expect("access");
    let s = login(&h, "kim").await;
    let r = call(
        &h,
        "POST",
        "/v1/api-keys",
        &s,
        Some(json!({"role": "admin", "zones": ["a.test."]})),
    )
    .await;
    assert_eq!(r.status, 403, "setting off");

    soabox::settings::Settings::apply(
        &h.state.store,
        soabox::settings::SettingsPatch {
            user_can_create_api_keys: Some(true),
            ..Default::default()
        },
    )
    .await
    .expect("db")
    .expect("valid");
    let r = call(
        &h,
        "POST",
        "/v1/api-keys",
        &s,
        Some(json!({"role": "admin", "zones": ["b.test."]})),
    )
    .await;
    assert_eq!(r.status, 403);
    let r = call(
        &h,
        "POST",
        "/v1/api-keys",
        &s,
        Some(json!({"role": "admin", "zones": ["a.test."], "description": "ci"})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.text);
    assert_eq!(
        r.body["key"]["role"], "scoped",
        "non-admins only get scoped keys"
    );
    let secret = r.body["secret"].as_str().expect("secret");
    assert_eq!(secret.len(), 43);
    let id = r.body["key"]["id"].as_i64().expect("id");
    let r = call(&h, "POST", &format!("/v1/api-keys/{id}/disable"), &s, None).await;
    assert_eq!(r.status, 204);
    let r = call(&h, "GET", "/v1/api-keys", &s, None).await;
    assert_eq!(r.body[0]["disabled"], true);
    assert!(r.text.find("key_hash").is_none());
}

#[tokio::test]
async fn settings_validation_and_static_fallback() {
    let pdns = MockServer::start().await;
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(
        dir.path().join("index.html"),
        "<!doctype html><title>SOAbox</title>",
    )
    .expect("write");
    std::fs::create_dir(dir.path().join("assets")).expect("dir");
    std::fs::write(dir.path().join("assets/app.js"), "x").expect("write");
    let mut vars = config_vars(&pdns.uri());
    vars.insert("SOABOX_STATIC_DIR".into(), dir.path().display().to_string());
    let h = Harness::with_vars(vars).await;
    user(&h, "admin", UserRole::Admin, UserStatus::Active).await;
    let s = login(&h, "admin").await;
    let r = call(
        &h,
        "PUT",
        "/v1/settings",
        &s,
        Some(json!({"record_types_forward": ["NOPE"]})),
    )
    .await;
    assert_eq!(r.status, 422);
    let r = call(
        &h,
        "PUT",
        "/v1/settings",
        &s,
        Some(json!({"auto_ptr": true})),
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["auto_ptr"], true);

    let r = send(&h, "GET", "/zones/a.test.", None, None, false).await;
    assert_eq!(r.status, 200);
    assert!(r.text.contains("SOAbox"));
    assert_eq!(r.headers["cache-control"], "no-store");
    assert!(
        r.headers["content-security-policy"]
            .to_str()
            .expect("csp")
            .contains("frame-ancestors 'none'")
    );
    let r = send(&h, "GET", "/assets/app.js", None, None, false).await;
    assert!(
        r.headers["cache-control"]
            .to_str()
            .expect("cc")
            .contains("immutable")
    );
    let r = call(&h, "GET", "/v1/nope", &s, None).await;
    assert_eq!(r.status, 404);
    assert_eq!(r.body["code"], "NOT_FOUND");
    let r = call(&h, "GET", "/v1/openapi.json", &s, None).await;
    assert_eq!(r.status, 200, "docs exposed in dev mode");
}

#[tokio::test]
async fn oidc_provisioning_modes() {
    use soabox::auth::oidc::{OidcIdentity, Provisioned, provision};
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    user(&h, "dana@example.test", UserRole::User, UserStatus::Active).await;
    let id = OidcIdentity {
        issuer: "https://idp.test".into(),
        subject: "sub-1".into(),
        preferred_username: Some("Dana@Example.test".into()),
        email: Some("dana@example.test".into()),
        name: Some("Dana".into()),
    };
    let mut settings = soabox::settings::Settings::default();
    match provision(&h.state, &id, &settings)
        .await
        .expect("provision")
    {
        Provisioned::Active(u) => {
            assert_eq!(
                u.username, "dana@example.test-2",
                "existing local name is not taken over"
            );
            assert_eq!(u.role, UserRole::User);
            assert_eq!(u.auth_source, "oidc");
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        provision(&h.state, &id, &settings).await.expect("again"),
        Provisioned::Active(_)
    ));
    settings.oidc_new_user = soabox::settings::NewOidcUser::Approve;
    let id2 = OidcIdentity {
        subject: "sub-2".into(),
        preferred_username: Some("eve".into()),
        ..id
    };
    assert!(matches!(
        provision(&h.state, &id2, &settings).await.expect("p"),
        Provisioned::Pending
    ));
    assert!(matches!(
        provision(&h.state, &id2, &settings).await.expect("p"),
        Provisioned::Pending
    ));
}

#[tokio::test]
async fn bootstrap_admin_is_created_once() {
    let pdns = MockServer::start().await;
    let mut vars = config_vars(&pdns.uri());
    vars.insert("SOABOX_BOOTSTRAP_ADMIN_USERNAME".into(), "root".into());
    vars.insert(
        "SOABOX_BOOTSTRAP_ADMIN_PASSWORD".into(),
        "correct-horse-battery".into(),
    );
    let h = Harness::with_vars(vars).await;
    soabox::auth::bootstrap_admin(&h.state)
        .await
        .expect("bootstrap");
    soabox::auth::bootstrap_admin(&h.state)
        .await
        .expect("idempotent");
    assert_eq!(h.state.store.count_active_admins().await.expect("count"), 1);
    login(&h, "root").await;
}

#[tokio::test]
async fn password_change_and_totp_disable() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let uid = user(&h, "pat", UserRole::User, UserStatus::Active).await;
    let s = login(&h, "pat").await;
    let r = call(
        &h,
        "PUT",
        "/v1/me/password",
        &s,
        Some(json!({"current_password": "wrong", "new_password": "new-long-password"})),
    )
    .await;
    assert_eq!(r.status, 422);
    let r = call(
        &h,
        "PUT",
        "/v1/me/password",
        &s,
        Some(json!({"current_password": "correct-horse-battery", "new_password": "short"})),
    )
    .await;
    assert_eq!(r.status, 422);
    let r = call(&h, "PUT", "/v1/me/password", &s, Some(json!({"current_password": "correct-horse-battery", "new_password": "new-long-password"}))).await;
    assert_eq!(r.status, 204);
    let r = send(
        &h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": "pat", "password": "new-long-password"})),
        false,
    )
    .await;
    assert_eq!(r.status, 200);

    let r = call(
        &h,
        "DELETE",
        "/v1/me/totp",
        &s,
        Some(json!({"code": "123456", "password": "new-long-password"})),
    )
    .await;
    assert_eq!(r.status, 400, "not enabled yet");
    call(&h, "POST", "/v1/me/totp/setup", &s, None).await;
    let enc = h
        .state
        .store
        .get_user(uid)
        .await
        .expect("q")
        .expect("u")
        .totp_secret_enc
        .expect("enc");
    let secret = crypto::decrypt(crypto::PURPOSE_TOTP, &h.state.secret_keys(), &enc).expect("dec");
    let code = crypto::totp_at(
        &secret,
        u64::try_from(soabox::store::now()).expect("t") / 30,
    );
    assert_eq!(
        call(
            &h,
            "POST",
            "/v1/me/totp/enable",
            &s,
            Some(json!({"code": code}))
        )
        .await
        .status,
        204
    );
    assert_eq!(
        call(&h, "POST", "/v1/me/totp/setup", &s, None).await.status,
        409
    );
    assert_eq!(
        call(
            &h,
            "DELETE",
            "/v1/me/totp",
            &s,
            Some(json!({"code": "000000", "password": "new-long-password"}))
        )
        .await
        .status,
        422
    );
    let next = crypto::totp_at(
        &secret,
        u64::try_from(soabox::store::now()).expect("t") / 30 + 1,
    );
    assert_eq!(
        call(
            &h,
            "DELETE",
            "/v1/me/totp",
            &s,
            Some(json!({"code": next, "password": "wrong"}))
        )
        .await
        .status,
        422,
        "password is required"
    );
    assert_eq!(
        call(
            &h,
            "DELETE",
            "/v1/me/totp",
            &s,
            Some(json!({"code": code, "password": "new-long-password"}))
        )
        .await
        .status,
        422,
        "the enable code cannot be replayed"
    );
    assert_eq!(
        call(
            &h,
            "DELETE",
            "/v1/me/totp",
            &s,
            Some(json!({"code": next, "password": "new-long-password"}))
        )
        .await
        .status,
        204
    );
    assert!(
        !h.state
            .store
            .get_user(uid)
            .await
            .expect("q")
            .expect("u")
            .totp_enabled
    );
    let r = call(
        &h,
        "PUT",
        "/v1/me",
        &s,
        Some(json!({"display_name": "P", "email": "not-an-email"})),
    )
    .await;
    assert_eq!(r.status, 422);
}

#[tokio::test]
async fn rotate_secrets_and_create_admin_cli() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let uid = user(&h, "rot", UserRole::User, UserStatus::Active).await;
    let enc = crypto::encrypt(
        crypto::PURPOSE_TOTP,
        &h.state.config.secret_key,
        b"secret-bytes",
    )
    .expect("enc");
    h.state
        .store
        .set_totp(uid, Some(&enc), true)
        .await
        .expect("totp");
    assert_eq!(
        soabox::cli::rotate_secrets(&h.state).await.expect("rotate"),
        1
    );
    let fresh = h
        .state
        .store
        .get_user(uid)
        .await
        .expect("q")
        .expect("u")
        .totp_secret_enc
        .expect("enc");
    assert_ne!(fresh, enc);
    assert_eq!(
        crypto::decrypt(crypto::PURPOSE_TOTP, &h.state.secret_keys(), &fresh).expect("dec"),
        b"secret-bytes"
    );
    let pw = soabox::cli::create_admin(&h.state.store, "rescue")
        .await
        .expect("admin");
    assert!(pw.len() >= 32);
    assert!(
        soabox::cli::create_admin(&h.state.store, "rescue")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn ui_policy_for_metadata_secondaries_and_overlap() {
    let pdns = pdns_with_zones().await;
    let h = Harness::new(&pdns.uri()).await;
    user(&h, "op", UserRole::Operator, UserStatus::Active).await;
    let uid = user(&h, "u", UserRole::User, UserStatus::Active).await;
    h.state
        .store
        .set_user_access(uid, &["a.test.".into()], &[])
        .await
        .expect("access");
    soabox::settings::Settings::apply(
        &h.state.store,
        soabox::settings::SettingsPatch {
            user_can_create_zones: Some(true),
            ..Default::default()
        },
    )
    .await
    .expect("db")
    .expect("valid");
    let op = login(&h, "op").await;
    let r = call(
        &h,
        "PUT",
        "/v1/zones/a.test./metadata/LUA-AXFR-SCRIPT",
        &op,
        Some(json!({"metadata": ["x"]})),
    )
    .await;
    assert_eq!(r.status, 403);
    let u = login(&h, "u").await;
    let r = call(
        &h,
        "POST",
        "/v1/zones",
        &u,
        Some(json!({"name": "s.test", "kind": "Slave", "masters": ["10.0.0.1"]})),
    )
    .await;
    assert_eq!(r.status, 403);
    let r = call(
        &h,
        "POST",
        "/v1/zones",
        &u,
        Some(json!({"name": "x.b.test", "kind": "Native"})),
    )
    .await;
    assert_eq!(r.status, 403);
    assert!(
        r.body["detail"]
            .as_str()
            .expect("detail")
            .contains("overlaps")
    );
    let r = call(
        &h,
        "PUT",
        "/v1/zones/a.test.",
        &u,
        Some(json!({"masters": ["10.0.0.1"]})),
    )
    .await;
    assert_eq!(r.status, 403);
}
