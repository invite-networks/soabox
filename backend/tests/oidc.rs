//! End-to-end OIDC sign-in against a real provider.
//!
//! Runs when `SOABOX_TEST_OIDC_ISSUER` points at a provider that signs in
//! without interaction, such as `ghcr.io/navikt/mock-oauth2-server` at
//! `http://localhost:28080/default`.

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Harness, config_vars};
use http_body_util::BodyExt;

#[tokio::test]
async fn full_code_flow_creates_user_and_session() {
    let Ok(issuer) = std::env::var("SOABOX_TEST_OIDC_ISSUER") else {
        eprintln!("SOABOX_TEST_OIDC_ISSUER not set, skipping");
        return;
    };
    let mut vars = config_vars("http://127.0.0.1:9");
    vars.insert("SOABOX_OIDC_ISSUER".into(), issuer);
    vars.insert("SOABOX_OIDC_CLIENT_ID".into(), "soabox".into());
    vars.insert("SOABOX_OIDC_CLIENT_SECRET".into(), "secret".into());
    let h = Harness::with_vars(vars).await;

    let resp = h
        .send(
            Request::get("/auth/oidc/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await;
    assert_eq!(resp.status(), 303);
    let location = resp.headers()["location"].to_str().expect("loc").to_owned();
    assert!(location.contains("code_challenge="));
    let flow_cookie = resp.headers()["set-cookie"]
        .to_str()
        .expect("cookie")
        .split(';')
        .next()
        .expect("pair")
        .to_owned();

    let provider = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client");
    let back = provider.get(&location).send().await.expect("authorize");
    assert!(
        back.status().is_redirection(),
        "provider answered {}",
        back.status()
    );
    let callback = url::Url::parse(back.headers()["location"].to_str().expect("loc")).expect("url");
    assert_eq!(callback.path(), "/auth/oidc/callback");
    let path_and_query = format!(
        "{}?{}",
        callback.path(),
        callback.query().unwrap_or_default()
    );

    let tampered = h
        .send(
            Request::get(path_and_query.replace("state=", "state=x"))
                .header("cookie", &flow_cookie)
                .body(Body::empty())
                .expect("req"),
        )
        .await;
    assert!(
        tampered.headers()["location"]
            .to_str()
            .expect("loc")
            .contains("oidc_failed")
    );

    let done = h
        .send(
            Request::get(&path_and_query)
                .header("cookie", &flow_cookie)
                .body(Body::empty())
                .expect("req"),
        )
        .await;
    assert_eq!(done.status(), 303);
    assert_eq!(done.headers()["location"], "/");
    let session = done
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with("soabox_session="))
        .expect("session cookie")
        .split(';')
        .next()
        .expect("pair")
        .to_owned();

    let me = h
        .send(
            Request::get("/v1/me")
                .header("cookie", &session)
                .body(Body::empty())
                .expect("req"),
        )
        .await;
    assert_eq!(me.status(), 200);
    let body = me.into_body().collect().await.expect("body").to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(v["user"]["auth_source"], "oidc");
    assert_eq!(v["user"]["role"], "user");

    let replay = h
        .send(
            Request::get(&path_and_query)
                .header("cookie", &flow_cookie)
                .body(Body::empty())
                .expect("req"),
        )
        .await;
    assert!(
        replay.headers()["location"]
            .to_str()
            .expect("loc")
            .contains("oidc_failed"),
        "code cannot be reused"
    );
}
