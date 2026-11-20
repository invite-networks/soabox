//! The `/v1` API end to end against a real PowerDNS. Runs when
//! `SOABOX_COMPAT_PDNS_URL` and `SOABOX_COMPAT_PDNS_KEY` are set.

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Harness, config_vars};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use soabox::auth::crypto;
use soabox::store::{NewUser, UserRole, UserStatus};

struct Client<'a> {
    h: &'a Harness,
    cookie: String,
    csrf: String,
}

impl Client<'_> {
    async fn call(&self, m: &str, uri: &str, body: Option<Value>) -> (u16, Value) {
        let mut b = Request::builder()
            .method(m)
            .uri(uri)
            .header("cookie", &self.cookie)
            .header("x-csrf-token", &self.csrf);
        let req = match body {
            Some(v) => {
                b = b.header("content-type", "application/json");
                b.body(Body::from(v.to_string()))
            }
            None => b.body(Body::empty()),
        }
        .expect("req");
        let resp = self.h.send(req).await;
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn ok(&self, m: &str, uri: &str, body: Option<Value>) -> Value {
        let (s, v) = self.call(m, uri, body).await;
        assert!((200..300).contains(&s), "{m} {uri} -> {s} {v}");
        v
    }
}

async fn sign_in<'a>(h: &'a Harness, name: &str, role: UserRole) -> Client<'a> {
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
    let resp = h
        .send(
            Request::post("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"username": name, "password": "correct-horse-battery"}).to_string(),
                ))
                .expect("req"),
        )
        .await;
    assert_eq!(resp.status(), 200);
    let cookie = resp.headers()["set-cookie"]
        .to_str()
        .expect("c")
        .split(';')
        .next()
        .expect("p")
        .to_owned();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    Client {
        h,
        cookie,
        csrf: v["csrf_token"].as_str().expect("csrf").to_owned(),
    }
}

#[tokio::test]
async fn ui_api_against_real_powerdns() {
    let (Ok(url), Ok(key)) = (
        std::env::var("SOABOX_COMPAT_PDNS_URL"),
        std::env::var("SOABOX_COMPAT_PDNS_KEY"),
    ) else {
        eprintln!("SOABOX_COMPAT_PDNS_URL not set, skipping");
        return;
    };
    let mut vars = config_vars(&url);
    vars.insert("SOABOX_PDNS_API_KEY".into(), key);
    let h = Harness::with_vars(vars).await;
    let admin = sign_in(&h, "admin", UserRole::Admin).await;
    let zone = "ui.compat.";
    let rev = "2.0.192.in-addr.arpa.";
    for z in [zone, rev] {
        let _ = admin.call("DELETE", &format!("/v1/zones/{z}"), None).await;
    }

    // Accounts, templates, zones.
    let acct = admin
        .ok(
            "POST",
            "/v1/accounts",
            Some(json!({"name": "team-ui", "description": "UI team"})),
        )
        .await;
    let acct_id = acct["id"].as_i64().expect("id");
    let tpl = admin
        .ok("POST", "/v1/templates", Some(json!({"name": "web", "records": [{"name": "www", "type": "A", "ttl": 300, "content": "192.0.2.10"}]})))
        .await;
    let tid = tpl["id"].as_i64().expect("id");
    admin.ok("PUT", &format!("/v1/templates/{tid}"), Some(json!({"name": "web", "description": "web", "records": [{"name": "www", "type": "A", "ttl": 300, "content": "192.0.2.10"}]}))).await;
    let (s, _) = admin.call("POST", "/v1/templates", Some(json!({"name": "bad", "records": [{"name": "@", "type": "SOA", "ttl": 1, "content": "x"}]}))).await;
    assert_eq!(s, 422);

    let z = admin
        .ok("POST", "/v1/zones", Some(json!({"name": zone, "kind": "Native", "nameservers": ["ns1.ui.compat."], "template_id": tid, "account_id": acct_id})))
        .await;
    assert_eq!(z["account"]["name"], "team-ui");
    assert!(
        z["rrsets"]
            .as_array()
            .expect("rrsets")
            .iter()
            .any(|r| r["name"] == "www.ui.compat.")
    );
    admin
        .ok(
            "POST",
            "/v1/zones",
            Some(json!({"name": rev, "kind": "Native", "nameservers": ["ns1.ui.compat."]})),
        )
        .await;
    let (s, _) = admin
        .call(
            "POST",
            "/v1/zones",
            Some(json!({"name": zone, "kind": "Native"})),
        )
        .await;
    assert_eq!(s, 422, "duplicate zone is an upstream error");
    let (s, _) = admin
        .call(
            "POST",
            "/v1/zones",
            Some(json!({"name": "x.compat.", "kind": "Bogus"})),
        )
        .await;
    assert_eq!(s, 422);

    let list = admin
        .ok("GET", "/v1/zones?q=ui.compat&per_page=10", None)
        .await;
    assert_eq!(list["total"], 1);

    // Records with automatic PTR.
    admin
        .ok("PUT", "/v1/settings", Some(json!({"auto_ptr": true})))
        .await;
    admin
        .ok("PATCH", &format!("/v1/zones/{zone}/rrsets"), Some(json!({"rrsets": [{"name": "mail.ui.compat.", "type": "A", "ttl": 300, "changetype": "REPLACE", "records": [{"content": "192.0.2.25", "disabled": false}]}]})))
        .await;
    let r = admin.ok("GET", &format!("/v1/zones/{rev}"), None).await;
    assert!(
        r["rrsets"]
            .as_array()
            .expect("rrsets")
            .iter()
            .any(|s| s["type"] == "PTR" && s["records"][0]["content"] == "mail.ui.compat.")
    );
    admin
        .ok("PATCH", &format!("/v1/zones/{zone}/rrsets"), Some(json!({"rrsets": [{"name": "mail.ui.compat.", "type": "A", "changetype": "DELETE"}]})))
        .await;
    let r = admin.ok("GET", &format!("/v1/zones/{rev}"), None).await;
    assert!(
        !r["rrsets"]
            .as_array()
            .expect("rrsets")
            .iter()
            .any(|s| s["type"] == "PTR")
    );

    // Zone settings and actions.
    admin
        .ok(
            "PUT",
            &format!("/v1/zones/{zone}"),
            Some(json!({"kind": "Master", "soa_edit_api": "INCEPTION-INCREMENT"})),
        )
        .await;
    admin
        .ok(
            "PUT",
            &format!("/v1/zones/{zone}"),
            Some(json!({"account_id": null})),
        )
        .await;
    assert_eq!(h.state.store.zone_account(zone).await.expect("q"), None);
    admin
        .ok(
            "PUT",
            &format!("/v1/zones/{zone}"),
            Some(json!({"account_id": acct_id})),
        )
        .await;
    let d = admin.ok("GET", &format!("/v1/zones/{zone}"), None).await;
    assert_eq!(d["kind"], "Master");
    assert_eq!(d["soa_edit_api"], "INCEPTION-INCREMENT");
    for action in ["notify", "rectify"] {
        let v = admin
            .ok("POST", &format!("/v1/zones/{zone}/{action}"), None)
            .await;
        assert!(v["result"].is_string());
    }
    let e = admin
        .ok("GET", &format!("/v1/zones/{zone}/export"), None)
        .await;
    assert!(e["zone"].as_str().expect("text").contains("www.ui.compat."));

    // DNSSEC and metadata.
    admin
        .ok(
            "POST",
            &format!("/v1/zones/{zone}/dnssec"),
            Some(json!({"enabled": true})),
        )
        .await;
    let keys = admin
        .ok("GET", &format!("/v1/zones/{zone}/cryptokeys"), None)
        .await;
    let kid = keys[0]["id"].as_u64().expect("key id");
    assert!(keys[0]["ds"].is_array());
    let added = admin
        .ok(
            "POST",
            &format!("/v1/zones/{zone}/cryptokeys"),
            Some(json!({"keytype": "zsk", "active": false})),
        )
        .await;
    assert!(added.get("privatekey").is_none());
    let new_id = added["id"].as_u64().expect("id");
    admin
        .ok(
            "PUT",
            &format!("/v1/zones/{zone}/cryptokeys/{new_id}"),
            Some(json!({"active": true, "published": true})),
        )
        .await;
    admin
        .ok(
            "DELETE",
            &format!("/v1/zones/{zone}/cryptokeys/{new_id}"),
            None,
        )
        .await;
    assert!(kid > 0);
    admin
        .ok(
            "PUT",
            &format!("/v1/zones/{zone}/metadata/ALLOW-AXFR-FROM"),
            Some(json!({"metadata": ["192.0.2.0/24"]})),
        )
        .await;
    let md = admin
        .ok("GET", &format!("/v1/zones/{zone}/metadata"), None)
        .await;
    assert!(
        md.as_array()
            .expect("md")
            .iter()
            .any(|m| m["kind"] == "ALLOW-AXFR-FROM")
    );
    admin
        .ok(
            "DELETE",
            &format!("/v1/zones/{zone}/metadata/ALLOW-AXFR-FROM"),
            None,
        )
        .await;
    let (s, _) = admin
        .call(
            "PUT",
            &format!("/v1/zones/{zone}/metadata/bad%20kind"),
            Some(json!({"metadata": []})),
        )
        .await;
    assert_eq!(s, 422);
    admin
        .ok(
            "POST",
            &format!("/v1/zones/{zone}/dnssec"),
            Some(json!({"enabled": false})),
        )
        .await;

    // Server views, dashboard, TSIG.
    let srv = admin.ok("GET", "/v1/server", None).await;
    assert_eq!(srv["daemon_type"], "authoritative");
    assert!(
        admin
            .ok("GET", "/v1/server/statistics", None)
            .await
            .is_array()
    );
    assert!(admin.ok("GET", "/v1/server/config", None).await.is_array());
    let dash = admin.ok("GET", "/v1/dashboard", None).await;
    assert!(dash["server"]["version"].is_string());
    assert!(dash["recent_history"].as_array().expect("h").len() > 3);
    let t = admin
        .ok(
            "POST",
            "/v1/tsigkeys",
            Some(json!({"name": "ui-xfr", "algorithm": "hmac-sha256"})),
        )
        .await;
    let tsig_id = t["id"].as_str().expect("id").to_owned();
    let listed = admin.ok("GET", "/v1/tsigkeys", None).await;
    assert!(
        listed
            .as_array()
            .expect("l")
            .iter()
            .all(|k| k.get("key").is_none())
    );
    assert!(
        admin
            .ok("GET", &format!("/v1/tsigkeys/{tsig_id}"), None)
            .await["key"]
            .is_string()
    );
    admin
        .ok(
            "PUT",
            &format!("/v1/tsigkeys/{tsig_id}"),
            Some(json!({"algorithm": "hmac-sha512"})),
        )
        .await;
    admin
        .ok("DELETE", &format!("/v1/tsigkeys/{tsig_id}"), None)
        .await;
    let tsig_hist = admin
        .ok("GET", "/v1/history?operation=tsigkey.create", None)
        .await;
    assert!(
        tsig_hist["items"][0]["detail"]["key"].is_null()
            || tsig_hist["items"][0]["detail"]["key"] == "[redacted]"
    );

    // A User in the account sees the zone, not the reverse zone.
    let user = sign_in(&h, "member", UserRole::User).await;
    let uid = h
        .state
        .store
        .find_user_by_username("member")
        .await
        .expect("q")
        .expect("u")
        .id;
    admin
        .ok(
            "PUT",
            &format!("/v1/accounts/{acct_id}"),
            Some(json!({"name": "team-ui", "description": "UI", "user_ids": [uid]})),
        )
        .await;
    let mine = user.ok("GET", "/v1/zones", None).await;
    assert_eq!(mine["total"], 1);
    assert_eq!(mine["items"][0]["name"], zone);
    let (s, _) = user.call("GET", &format!("/v1/zones/{rev}"), None).await;
    assert_eq!(s, 404);
    let (s, _) = user
        .call(
            "POST",
            &format!("/v1/zones/{zone}/dnssec"),
            Some(json!({"enabled": true})),
        )
        .await;
    assert_eq!(s, 403);
    let zh = user
        .ok("GET", &format!("/v1/zones/{zone}/history"), None)
        .await;
    assert!(!zh["items"].as_array().expect("items").is_empty());
    let accts = user.ok("GET", "/v1/accounts", None).await;
    assert_eq!(accts.as_array().expect("a").len(), 1);
    let dash = user.ok("GET", "/v1/dashboard", None).await;
    assert_eq!(dash["zone_count"], 1);
    assert!(dash["server"].is_null());

    // Admin key management through the UI.
    let created = admin
        .ok(
            "POST",
            "/v1/api-keys",
            Some(json!({"role": "operator", "description": "ops", "networks": ["10.0.0.0/8"]})),
        )
        .await;
    let kid = created["key"]["id"].as_i64().expect("id");
    admin
        .ok(
            "PUT",
            &format!("/v1/api-keys/{kid}"),
            Some(json!({"role": "scoped", "zones": [zone], "description": "ops"})),
        )
        .await;
    admin
        .ok("POST", &format!("/v1/api-keys/{kid}/enable"), None)
        .await;
    let (s, _) = user
        .call("DELETE", &format!("/v1/api-keys/{kid}"), None)
        .await;
    assert_eq!(s, 403);
    admin
        .ok("DELETE", &format!("/v1/api-keys/{kid}"), None)
        .await;
    let (s, _) = admin
        .call(
            "POST",
            "/v1/api-keys",
            Some(json!({"role": "scoped", "expires_at": 1})),
        )
        .await;
    assert_eq!(s, 422);

    // User administration extras.
    admin
        .ok("POST", &format!("/v1/users/{uid}/revoke-sessions"), None)
        .await;
    let (s, _) = user.call("GET", "/v1/me", None).await;
    assert_eq!(s, 401);
    admin
        .ok("DELETE", &format!("/v1/users/{uid}/totp"), None)
        .await;

    // Clean up.
    admin
        .ok("DELETE", &format!("/v1/accounts/{acct_id}"), None)
        .await;
    admin
        .ok("DELETE", &format!("/v1/templates/{tid}"), None)
        .await;
    for z in [zone, rev] {
        admin.ok("DELETE", &format!("/v1/zones/{z}"), None).await;
    }
    let (s, _) = admin.call("GET", &format!("/v1/zones/{zone}"), None).await;
    assert_eq!(s, 404);
}
