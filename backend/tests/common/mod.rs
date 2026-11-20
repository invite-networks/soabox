//! Shared helpers for integration tests.
#![allow(dead_code)]

use std::collections::HashMap;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use soabox::config::Config;
use soabox::keys;
use soabox::state::AppState;
use soabox::store::{NewApiKey, Role, Store};
use tower::ServiceExt;

pub struct Harness {
    pub state: AppState,
    pub app: Router,
    _dir: tempfile::TempDir,
}

pub fn config_vars(pdns_url: &str) -> HashMap<String, String> {
    [
        ("SOABOX_MODE", "dev"),
        ("SOABOX_PUBLIC_URL", "http://localhost:8080"),
        ("SOABOX_PDNS_URL", pdns_url),
        ("SOABOX_PDNS_API_KEY", "real-pdns-key"),
        ("SOABOX_PDNS_TIMEOUT_SECS", "2"),
        (
            "SOABOX_SECRET_KEY",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

impl Harness {
    pub async fn new(pdns_url: &str) -> Self {
        Self::with_vars(config_vars(pdns_url)).await
    }

    pub async fn with_vars(vars: HashMap<String, String>) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = Config::from_map(&vars).expect("config");
        let store = Store::connect(&format!("sqlite://{}/t.db", dir.path().display()))
            .await
            .expect("store");
        let state = AppState::with_store(config, store).expect("state");
        let app = soabox::app::router(state.clone());
        Self {
            state,
            app,
            _dir: dir,
        }
    }

    pub async fn key(
        &self,
        role: Role,
        zones: &[&str],
        accounts: &[i64],
        create_in: Option<i64>,
    ) -> String {
        let g = keys::generate().expect("gen");
        self.state
            .store
            .insert_api_key(&NewApiKey {
                key_hash: g.hash,
                hint: g.hint,
                description: "test".into(),
                role: Some(role),
                create_account_id: create_in,
                zones: zones.iter().map(|z| (*z).to_owned()).collect(),
                account_ids: accounts.to_vec(),
                ..Default::default()
            })
            .await
            .expect("insert");
        g.secret
    }

    pub async fn send(&self, req: Request<Body>) -> Response<Body> {
        self.app.clone().oneshot(req).await.expect("response")
    }

    pub async fn call(
        &self,
        method: &str,
        uri: &str,
        key: Option<&str>,
        body: &str,
    ) -> (u16, axum::http::HeaderMap, String) {
        let mut b = Request::builder().method(method).uri(uri);
        if let Some(k) = key {
            b = b.header("x-api-key", k);
        }
        if !body.is_empty() {
            b = b.header("content-type", "application/json");
        }
        let resp = self
            .send(b.body(Body::from(body.to_owned())).expect("req"))
            .await;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        (
            status,
            headers,
            String::from_utf8_lossy(&bytes).into_owned(),
        )
    }
}
