//! Store behavior on every supported database. SQLite always runs. Postgres
//! runs when `SOABOX_TEST_POSTGRES_URL` points at an empty database.

use soabox::store::{HistoryFilter, NewApiKey, NewHistory, Role, Store};

async fn exercise(s: &Store) {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let acct_name = format!("acme-{suffix}");
    let zone = format!("z-{suffix}.test.");
    let acct = s.create_account(&acct_name, "d").await.expect("account");
    s.set_zone_account(&zone, acct).await.expect("map");
    assert_eq!(s.zone_account(&zone).await.expect("q"), Some(acct));

    let hash = format!("hash-{suffix}");
    let id = s
        .insert_api_key(&NewApiKey {
            key_hash: hash.clone(),
            hint: "abcd".into(),
            role: Some(Role::Scoped),
            create_account_id: Some(acct),
            zones: vec![zone.clone()],
            account_ids: vec![acct],
            networks: vec!["10.0.0.0/8".into()],
            expires_at: Some(4_000_000_000),
            ..Default::default()
        })
        .await
        .expect("key");
    let k = s
        .find_api_key_by_hash(&hash)
        .await
        .expect("q")
        .expect("key");
    assert_eq!(k.id, id);
    assert_eq!(k.zones, vec![zone.clone()]);
    assert_eq!(k.expires_at, Some(4_000_000_000));
    assert!(!k.disabled);
    assert!(s.set_api_key_disabled(id, true).await.expect("disable"));
    assert!(s.get_api_key(id).await.expect("q").expect("k").disabled);
    s.add_api_key_zone(id, "extra.test.").await.expect("add");
    s.add_api_key_zone(id, "extra.test.")
        .await
        .expect("add again");

    s.put_setting(&format!("k-{suffix}"), "1")
        .await
        .expect("put");
    s.put_setting(&format!("k-{suffix}"), "2")
        .await
        .expect("put");
    assert_eq!(
        s.all_settings()
            .await
            .expect("all")
            .get(&format!("k-{suffix}"))
            .map(String::as_str),
        Some("2")
    );

    let provider = soabox::store::OidcProviderRecord {
        enabled: true,
        issuer: "https://login.example.test/v2.0".into(),
        client_id: format!("client-{suffix}"),
        client_secret_enc: Some(vec![0, 1, 2, 255]),
        button_label: "Sign in".into(),
        updated_at: 0,
    };
    s.put_oidc_provider(&provider).await.expect("put provider");
    s.put_oidc_provider(&provider)
        .await
        .expect("upsert provider");
    let got = s.oidc_provider().await.expect("get").expect("row");
    assert_eq!(got.client_id, provider.client_id);
    assert_eq!(got.client_secret_enc, provider.client_secret_enc);
    assert!(got.enabled);

    let hid = s
        .insert_history(&NewHistory {
            actor_type: "api_key".into(),
            actor_id: Some(id),
            actor_name: "k".into(),
            operation: "rrsets.patch".into(),
            zone_name: Some(zone.clone()),
            outcome: "success".into(),
            upstream_status: Some(204),
            diff: Some(serde_json::json!([{"name": "www"}])),
            ..Default::default()
        })
        .await
        .expect("history");
    let rows = s
        .list_history(&HistoryFilter {
            zones: Some(vec![zone.clone()]),
            ..Default::default()
        })
        .await
        .expect("list");
    assert_eq!(rows[0].id, hid);
    assert_eq!(rows[0].upstream_status, Some(204));

    s.forget_zone(&zone).await.expect("forget");
    assert_eq!(s.zone_account(&zone).await.expect("q"), None);
    assert!(s.delete_api_key(id).await.expect("delete"));
    assert!(s.delete_account(acct).await.expect("delete account"));

    let uid = s
        .create_user(&soabox::store::NewUser {
            username: format!("totp-{suffix}"),
            display_name: String::new(),
            email: String::new(),
            role: soabox::store::UserRole::User,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: None,
            status: soabox::store::UserStatus::Active,
        })
        .await
        .expect("user");
    let (a, b) = tokio::join!(s.consume_totp_step(uid, 7), s.consume_totp_step(uid, 7));
    assert_eq!(
        [a.expect("a"), b.expect("b")]
            .iter()
            .filter(|w| **w)
            .count(),
        1,
        "a TOTP step is consumed once"
    );
    assert!(!s.consume_totp_step(uid, 6).await.expect("older"));
}

#[tokio::test]
async fn sqlite_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let s = Store::connect(&format!("sqlite://{}/s.db", dir.path().display()))
        .await
        .expect("store");
    exercise(&s).await;
}

#[tokio::test]
async fn postgres_store() {
    let Ok(url) = std::env::var("SOABOX_TEST_POSTGRES_URL") else {
        eprintln!("SOABOX_TEST_POSTGRES_URL not set, skipping");
        return;
    };
    let s = Store::connect(&url).await.expect("postgres store");
    s.ping().await.expect("ping");
    exercise(&s).await;
}
