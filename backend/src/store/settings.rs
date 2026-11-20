//! Admin settings stored as key and value pairs, and the single sign-on
//! provider row.

use std::collections::BTreeMap;

use sqlx::Row;

use super::{Backend, Store, now};

/// The stored single sign-on provider. The secret stays encrypted here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OidcProviderRecord {
    pub enabled: bool,
    pub issuer: String,
    pub client_id: String,
    pub client_secret_enc: Option<Vec<u8>>,
    pub button_label: String,
    pub updated_at: i64,
}

impl Store {
    /// All stored settings. Missing keys fall back to defaults elsewhere.
    pub async fn all_settings(&self) -> Result<BTreeMap<String, String>, sqlx::Error> {
        sqlx::query("SELECT key, value FROM settings")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| Ok((r.try_get(0)?, r.try_get(1)?)))
            .collect()
    }

    /// Stores one setting.
    pub async fn put_setting(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        let sql = match self.backend {
            Backend::Sqlite => {
                "INSERT INTO settings (key, value) VALUES ($1, $2) \
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value"
            }
            Backend::Postgres => {
                "INSERT INTO settings (key, value) VALUES ($1, $2) \
                 ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value"
            }
        };
        sqlx::query(sql)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl Store {
    /// The stored single sign-on provider, if one was ever saved.
    pub async fn oidc_provider(&self) -> Result<Option<OidcProviderRecord>, sqlx::Error> {
        sqlx::query(
            "SELECT enabled, issuer, client_id, client_secret_enc, button_label, updated_at \
             FROM oidc_provider WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| {
            Ok(OidcProviderRecord {
                enabled: r.try_get::<i64, _>("enabled")? != 0,
                issuer: r.try_get("issuer")?,
                client_id: r.try_get("client_id")?,
                client_secret_enc: r.try_get("client_secret_enc")?,
                button_label: r.try_get("button_label")?,
                updated_at: r.try_get("updated_at")?,
            })
        })
        .transpose()
    }

    /// Stores the single sign-on provider, replacing any earlier row.
    pub async fn put_oidc_provider(&self, p: &OidcProviderRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO oidc_provider \
             (id, enabled, issuer, client_id, client_secret_enc, button_label, updated_at) \
             VALUES (1, $1, $2, $3, $4, $5, $6) \
             ON CONFLICT (id) DO UPDATE SET enabled = excluded.enabled, \
             issuer = excluded.issuer, client_id = excluded.client_id, \
             client_secret_enc = excluded.client_secret_enc, \
             button_label = excluded.button_label, updated_at = excluded.updated_at",
        )
        .bind(i64::from(p.enabled))
        .bind(&p.issuer)
        .bind(&p.client_id)
        .bind(p.client_secret_enc.clone())
        .bind(&p.button_label)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::OidcProviderRecord;
    use crate::store::testing;

    #[tokio::test]
    async fn oidc_provider_round_trip() {
        let (s, _d) = testing::sqlite().await;
        assert_eq!(s.oidc_provider().await.expect("get"), None);
        let mut p = OidcProviderRecord {
            enabled: true,
            issuer: "https://login.example.test/v2.0".into(),
            client_id: "client".into(),
            client_secret_enc: Some(vec![1, 2, 3]),
            button_label: "Sign in".into(),
            updated_at: 0,
        };
        s.put_oidc_provider(&p).await.expect("put");
        p.enabled = false;
        p.client_secret_enc = None;
        s.put_oidc_provider(&p).await.expect("put again");
        let got = s.oidc_provider().await.expect("get").expect("row");
        assert!(got.updated_at > 0);
        assert_eq!(
            got,
            OidcProviderRecord {
                updated_at: got.updated_at,
                ..p
            }
        );
    }

    #[tokio::test]
    async fn put_and_read() {
        let (s, _d) = testing::sqlite().await;
        s.put_setting("a", "1").await.expect("put");
        s.put_setting("a", "2").await.expect("put");
        assert_eq!(
            s.all_settings()
                .await
                .expect("all")
                .get("a")
                .map(String::as_str),
            Some("2")
        );
    }
}
