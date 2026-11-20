//! Single sign-on settings managed in the UI: storage, secret handling,
//! live reload, key rotation, and environment precedence.

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Harness, config_vars};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use soabox::auth::crypto;
use soabox::config::Config;
use soabox::state::AppState;
use soabox::store::{HistoryFilter, NewUser, UserRole, UserStatus};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SECRET: &str = "very-secret-client-value";

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
    s: Option<&(String, String)>,
    body: Option<Value>,
) -> Resp {
    let mut b = Request::builder().method(m).uri(uri);
    if let Some((cookie, csrf)) = s {
        b = b.header("cookie", cookie).header("x-csrf-token", csrf);
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

async fn session(h: &Harness, name: &str, role: UserRole) -> (String, String) {
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
            status: UserStatus::Active,
        })
        .await
        .expect("user");
    let r = send(
        h,
        "POST",
        "/auth/login",
        None,
        Some(json!({"username": name, "password": "correct-horse-battery"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let cookie = r.headers["set-cookie"]
        .to_str()
        .expect("cookie")
        .split(';')
        .next()
        .expect("pair")
        .to_owned();
    (
        cookie,
        r.body["csrf_token"].as_str().expect("csrf").to_owned(),
    )
}

/// A provider that answers discovery for `{uri}/tenant`.
async fn provider() -> (MockServer, String) {
    let idp = MockServer::start().await;
    let issuer = format!("{}/tenant", idp.uri());
    Mock::given(method("GET"))
        .and(path("/tenant/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "jwks_uri": format!("{issuer}/keys"),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        })))
        .mount(&idp)
        .await;
    Mock::given(method("GET"))
        .and(path("/tenant/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keys": []})))
        .mount(&idp)
        .await;
    (idp, issuer)
}

async fn start_location(h: &Harness) -> String {
    let r = send(h, "GET", "/auth/oidc/start", None, None).await;
    assert_eq!(r.status, 303, "{}", r.text);
    r.headers["location"].to_str().expect("loc").to_owned()
}

#[tokio::test]
async fn admin_configures_sso_without_restart() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let admin = session(&h, "admin", UserRole::Admin).await;
    let plain = session(&h, "plain", UserRole::User).await;

    let r = send(&h, "GET", "/v1/settings/oidc", Some(&admin), None).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["source"], "database");
    assert_eq!(r.body["enabled"], false);
    assert_eq!(r.body["client_secret_set"], false);
    assert_eq!(r.body["button_label"], "Sign in with SSO");
    assert_eq!(
        r.body["redirect_uri"],
        "http://localhost:8080/auth/oidc/callback"
    );
    assert_eq!(
        send(&h, "GET", "/v1/settings/oidc", Some(&plain), None)
            .await
            .status,
        403
    );
    assert_eq!(
        send(
            &h,
            "PUT",
            "/v1/settings/oidc",
            Some(&plain),
            Some(json!({"enabled": true}))
        )
        .await
        .status,
        403
    );
    assert_eq!(
        send(&h, "GET", "/auth/oidc/start", None, None).await.status,
        404
    );

    let (_idp, issuer) = provider().await;
    let missing_secret = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({"enabled": true, "issuer": issuer, "client_id": "first"})),
    )
    .await;
    assert_eq!(missing_secret.status, 422, "{}", missing_secret.text);
    let bad_issuer = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({"enabled": true, "issuer": "ftp://x", "client_id": "a", "client_secret": "b"})),
    )
    .await;
    assert_eq!(bad_issuer.status, 422);

    let saved = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({
            "enabled": true,
            "issuer": issuer,
            "client_id": "first",
            "client_secret": SECRET,
            "button_label": "Sign in with Example",
        })),
    )
    .await;
    assert_eq!(saved.status, 200, "{}", saved.text);
    assert_eq!(saved.body["client_secret_set"], true);
    assert!(saved.body.get("client_secret").is_none());
    assert!(!saved.text.contains(SECRET));

    let cfg = send(&h, "GET", "/auth/config", None, None).await;
    assert_eq!(cfg.body["oidc_enabled"], true);
    assert_eq!(cfg.body["oidc_label"], "Sign in with Example");
    let loc = start_location(&h).await;
    assert!(loc.starts_with(&format!("{issuer}/authorize")), "{loc}");
    assert!(loc.contains("client_id=first"));

    // Keep the secret and change the client id. The next sign-in uses it.
    let changed = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({"enabled": true, "issuer": issuer, "client_id": "second", "button_label": ""})),
    )
    .await;
    assert_eq!(changed.status, 200, "{}", changed.text);
    assert_eq!(changed.body["client_secret_set"], true);
    assert_eq!(changed.body["button_label"], "Sign in with SSO");
    assert!(start_location(&h).await.contains("client_id=second"));
    let active = soabox::auth::provider::active(&h.state)
        .await
        .expect("active")
        .expect("on");
    assert_eq!(active.client_secret.expose(), SECRET);

    // The secret is encrypted at rest and never written to history.
    let rec = h
        .state
        .store
        .oidc_provider()
        .await
        .expect("row")
        .expect("some");
    let enc = rec.client_secret_enc.expect("enc");
    assert!(!String::from_utf8_lossy(&enc).contains(SECRET));
    let history = h
        .state
        .store
        .list_history(&HistoryFilter::default())
        .await
        .expect("history");
    let updates: Vec<_> = history
        .iter()
        .filter(|e| e.operation == "settings.oidc_update")
        .collect();
    assert_eq!(updates.len(), 4, "failed and successful saves are recorded");
    let all = serde_json::to_string(&history).expect("json");
    assert!(!all.contains(SECRET));
    assert!(all.contains("client_secret_changed"));

    // Turning it off hides the button and the endpoints at once.
    let off = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({"enabled": false, "issuer": issuer, "client_id": "second"})),
    )
    .await;
    assert_eq!(off.status, 200);
    assert_eq!(off.body["client_secret_set"], true);
    let cfg = send(&h, "GET", "/auth/config", None, None).await;
    assert_eq!(cfg.body["oidc_enabled"], false);
    assert_eq!(cfg.body["local_enabled"], true);
    assert_eq!(
        send(&h, "GET", "/auth/oidc/start", None, None).await.status,
        404
    );
}

#[tokio::test]
async fn rotate_secrets_re_encrypts_the_client_secret() {
    let pdns = MockServer::start().await;
    let h = Harness::new(&pdns.uri()).await;
    let admin = session(&h, "admin", UserRole::Admin).await;
    let r = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({
            "enabled": true,
            "issuer": "https://login.example.test/tenant/v2.0",
            "client_id": "c",
            "client_secret": SECRET,
        })),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let old_key = config_vars("x")["SOABOX_SECRET_KEY"].clone();
    let new_key = "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=";

    let mut vars = config_vars(&pdns.uri());
    vars.insert("SOABOX_SECRET_KEY".into(), new_key.into());
    vars.insert("SOABOX_SECRET_KEY_PREVIOUS".into(), old_key);
    let rotating = AppState::with_store(
        Config::from_map(&vars).expect("config"),
        h.state.store.clone(),
    )
    .expect("state");
    assert_eq!(
        soabox::cli::rotate_secrets(&rotating)
            .await
            .expect("rotate"),
        1
    );

    vars.remove("SOABOX_SECRET_KEY_PREVIOUS");
    let rotated = AppState::with_store(
        Config::from_map(&vars).expect("config"),
        h.state.store.clone(),
    )
    .expect("state");
    let active = soabox::auth::provider::active(&rotated)
        .await
        .expect("active")
        .expect("decrypts with the new key alone");
    assert_eq!(active.client_secret.expose(), SECRET);
    assert!(
        soabox::auth::provider::active(&h.state)
            .await
            .expect("active")
            .is_none(),
        "the old key alone no longer decrypts it"
    );
}

#[tokio::test]
async fn environment_takes_precedence_and_is_read_only() {
    let pdns = MockServer::start().await;
    let mut vars = config_vars(&pdns.uri());
    vars.insert(
        "SOABOX_OIDC_ISSUER".into(),
        "https://env.example.test/v2.0".into(),
    );
    vars.insert("SOABOX_OIDC_CLIENT_ID".into(), "env-client".into());
    vars.insert("SOABOX_OIDC_CLIENT_SECRET".into(), SECRET.into());
    vars.insert("SOABOX_OIDC_BUTTON_LABEL".into(), "Env button".into());
    let h = Harness::with_vars(vars).await;
    h.state
        .store
        .put_oidc_provider(&soabox::store::OidcProviderRecord {
            enabled: true,
            issuer: "https://db.example.test".into(),
            client_id: "db-client".into(),
            client_secret_enc: Some(
                crypto::encrypt(
                    crypto::PURPOSE_OIDC_CLIENT_SECRET,
                    &h.state.config.secret_key,
                    b"db",
                )
                .expect("enc"),
            ),
            button_label: "DB button".into(),
            updated_at: 0,
        })
        .await
        .expect("put");
    let admin = session(&h, "admin", UserRole::Admin).await;

    let r = send(&h, "GET", "/v1/settings/oidc", Some(&admin), None).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["source"], "environment");
    assert_eq!(r.body["enabled"], true);
    assert_eq!(r.body["client_id"], "env-client");
    assert_eq!(r.body["client_secret_set"], true);
    assert!(!r.text.contains(SECRET));

    let put = send(
        &h,
        "PUT",
        "/v1/settings/oidc",
        Some(&admin),
        Some(json!({"enabled": false})),
    )
    .await;
    assert_eq!(put.status, 409, "{}", put.text);

    let cfg = send(&h, "GET", "/auth/config", None, None).await;
    assert_eq!(cfg.body["oidc_enabled"], true);
    assert_eq!(cfg.body["oidc_label"], "Env button");
    let active = soabox::auth::provider::active(&h.state)
        .await
        .expect("active")
        .expect("on");
    assert_eq!(active.client_id, "env-client");
}
