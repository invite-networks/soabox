//! Admin settings stored as key and value pairs.

use std::collections::BTreeMap;

use sqlx::Row;

use super::{Backend, Store};

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

#[cfg(test)]
mod tests {
    use crate::store::testing;

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
