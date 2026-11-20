//! API key records.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::any::AnyRow;

use super::{Store, now};

/// What an API key may do. See the design's route classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Everything, including server administration.
    Admin,
    /// All zones and read-only server views.
    Operator,
    /// Only the zones and accounts it is given.
    Scoped,
}

impl Role {
    /// Database spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Scoped => "scoped",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "operator" => Ok(Self::Operator),
            "scoped" => Ok(Self::Scoped),
            other => Err(format!("unknown role {other:?}")),
        }
    }
}

/// A stored API key with its scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiKeyRecord {
    pub id: i64,
    pub hint: String,
    pub description: String,
    pub role: Role,
    pub create_account_id: Option<i64>,
    pub owner_user_id: Option<i64>,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
    pub disabled: bool,
    pub zones: Vec<String>,
    pub account_ids: Vec<i64>,
    pub networks: Vec<String>,
}

impl ApiKeyRecord {
    /// True if the key is past its expiry time at `at`.
    pub fn is_expired(&self, at: i64) -> bool {
        self.expires_at.is_some_and(|e| e <= at)
    }
}

/// Fields for a new key. The digest is computed by the caller.
#[derive(Debug, Clone, Default)]
pub struct NewApiKey {
    pub key_hash: String,
    pub hint: String,
    pub description: String,
    pub role: Option<Role>,
    pub create_account_id: Option<i64>,
    pub owner_user_id: Option<i64>,
    pub expires_at: Option<i64>,
    pub zones: Vec<String>,
    pub account_ids: Vec<i64>,
    pub networks: Vec<String>,
}

const KEY_COLUMNS: &str = "id, hint, description, role, create_account_id, owner_user_id, \
     expires_at, last_used_at, created_at, disabled";

impl Store {
    /// Inserts a key and its scope. Returns the new id.
    pub async fn insert_api_key(&self, k: &NewApiKey) -> Result<i64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let role = k.role.unwrap_or(Role::Scoped);
        let id: i64 = sqlx::query(
            "INSERT INTO api_keys (key_hash, hint, description, role, create_account_id, \
             owner_user_id, expires_at, created_at, disabled) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0) RETURNING id",
        )
        .bind(&k.key_hash)
        .bind(&k.hint)
        .bind(&k.description)
        .bind(role.as_str())
        .bind(k.create_account_id)
        .bind(k.owner_user_id)
        .bind(k.expires_at)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?
        .try_get(0)?;
        replace_scope(&mut tx, id, &k.zones, &k.account_ids, &k.networks).await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Replaces a key's description, role, expiry, and scope.
    pub async fn update_api_key(&self, id: i64, k: &NewApiKey) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let n = sqlx::query(
            "UPDATE api_keys SET description = $1, role = $2, create_account_id = $3, \
             expires_at = $4 WHERE id = $5",
        )
        .bind(&k.description)
        .bind(k.role.unwrap_or(Role::Scoped).as_str())
        .bind(k.create_account_id)
        .bind(k.expires_at)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n == 0 {
            return Ok(false);
        }
        replace_scope(&mut tx, id, &k.zones, &k.account_ids, &k.networks).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Finds a key by the digest of its secret.
    pub async fn find_api_key_by_hash(
        &self,
        hash: &str,
    ) -> Result<Option<ApiKeyRecord>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {KEY_COLUMNS} FROM api_keys WHERE key_hash = $1"
        )))
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            None => Ok(None),
            Some(r) => Ok(Some(self.hydrate_key(&r).await?)),
        }
    }

    /// Gets a key by id.
    pub async fn get_api_key(&self, id: i64) -> Result<Option<ApiKeyRecord>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {KEY_COLUMNS} FROM api_keys WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            None => Ok(None),
            Some(r) => Ok(Some(self.hydrate_key(&r).await?)),
        }
    }

    /// Lists keys, optionally only those owned by a user.
    pub async fn list_api_keys(
        &self,
        owner_user_id: Option<i64>,
    ) -> Result<Vec<ApiKeyRecord>, sqlx::Error> {
        let rows = match owner_user_id {
            Some(uid) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "SELECT {KEY_COLUMNS} FROM api_keys WHERE owner_user_id = $1 ORDER BY id"
                )))
                .bind(uid)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "SELECT {KEY_COLUMNS} FROM api_keys ORDER BY id"
                )))
                .fetch_all(&self.pool)
                .await?
            }
        };
        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            out.push(self.hydrate_key(r).await?);
        }
        Ok(out)
    }

    /// Enables or disables a key. Returns false if it does not exist.
    pub async fn set_api_key_disabled(&self, id: i64, disabled: bool) -> Result<bool, sqlx::Error> {
        let n = sqlx::query("UPDATE api_keys SET disabled = $1 WHERE id = $2")
            .bind(i64::from(disabled))
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n > 0)
    }

    /// Disables every key owned by a user.
    pub async fn disable_user_api_keys(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        Ok(
            sqlx::query("UPDATE api_keys SET disabled = 1 WHERE owner_user_id = $1")
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    /// Deletes a key. Returns false if it does not exist.
    pub async fn delete_api_key(&self, id: i64) -> Result<bool, sqlx::Error> {
        let n = sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n > 0)
    }

    /// Records the last time a key was used.
    pub async fn touch_api_key(&self, id: i64, at: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE api_keys SET last_used_at = $1 WHERE id = $2")
            .bind(at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Adds a zone directly to a key's scope.
    pub async fn add_api_key_zone(&self, id: i64, zone: &str) -> Result<(), sqlx::Error> {
        let sql = match self.backend {
            super::Backend::Sqlite => {
                "INSERT OR IGNORE INTO api_key_zones (api_key_id, zone_name) VALUES ($1, $2)"
            }
            super::Backend::Postgres => {
                "INSERT INTO api_key_zones (api_key_id, zone_name) VALUES ($1, $2) \
                 ON CONFLICT DO NOTHING"
            }
        };
        sqlx::query(sql)
            .bind(id)
            .bind(zone)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn hydrate_key(&self, r: &AnyRow) -> Result<ApiKeyRecord, sqlx::Error> {
        let id: i64 = r.try_get("id")?;
        let role_s: String = r.try_get("role")?;
        let role = role_s
            .parse()
            .map_err(|e: String| sqlx::Error::Decode(e.into()))?;
        let zones = sqlx::query(
            "SELECT zone_name FROM api_key_zones WHERE api_key_id = $1 ORDER BY zone_name",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| r.try_get::<String, _>(0))
        .collect::<Result<_, _>>()?;
        let account_ids = sqlx::query(
            "SELECT account_id FROM api_key_accounts WHERE api_key_id = $1 ORDER BY account_id",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| r.try_get::<i64, _>(0))
        .collect::<Result<_, _>>()?;
        let networks =
            sqlx::query("SELECT cidr FROM api_key_networks WHERE api_key_id = $1 ORDER BY cidr")
                .bind(id)
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| r.try_get::<String, _>(0))
                .collect::<Result<_, _>>()?;
        Ok(ApiKeyRecord {
            id,
            hint: r.try_get("hint")?,
            description: r.try_get("description")?,
            role,
            create_account_id: r.try_get("create_account_id")?,
            owner_user_id: r.try_get("owner_user_id")?,
            expires_at: r.try_get("expires_at")?,
            last_used_at: r.try_get("last_used_at")?,
            created_at: r.try_get("created_at")?,
            disabled: r.try_get::<i64, _>("disabled")? != 0,
            zones,
            account_ids,
            networks,
        })
    }
}

async fn replace_scope(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    id: i64,
    zones: &[String],
    account_ids: &[i64],
    networks: &[String],
) -> Result<(), sqlx::Error> {
    for table in ["api_key_zones", "api_key_accounts", "api_key_networks"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE api_key_id = $1"
        )))
        .bind(id)
        .execute(&mut **tx)
        .await?;
    }
    let mut zones: Vec<&String> = zones.iter().collect();
    zones.sort();
    zones.dedup();
    for z in zones {
        sqlx::query("INSERT INTO api_key_zones (api_key_id, zone_name) VALUES ($1, $2)")
            .bind(id)
            .bind(z)
            .execute(&mut **tx)
            .await?;
    }
    let mut accounts = account_ids.to_vec();
    accounts.sort_unstable();
    accounts.dedup();
    for a in accounts {
        sqlx::query("INSERT INTO api_key_accounts (api_key_id, account_id) VALUES ($1, $2)")
            .bind(id)
            .bind(a)
            .execute(&mut **tx)
            .await?;
    }
    let mut nets: Vec<&String> = networks.iter().collect();
    nets.sort();
    nets.dedup();
    for n in nets {
        sqlx::query("INSERT INTO api_key_networks (api_key_id, cidr) VALUES ($1, $2)")
            .bind(id)
            .bind(n)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing;

    #[tokio::test]
    async fn insert_find_update_delete() {
        let (s, _d) = testing::sqlite().await;
        let acct = s.create_account("acme", "").await.expect("acct");
        let id = s
            .insert_api_key(&NewApiKey {
                key_hash: "h1".into(),
                hint: "abcd".into(),
                description: "ci".into(),
                role: Some(Role::Scoped),
                create_account_id: Some(acct),
                zones: vec!["b.test.".into(), "a.test.".into(), "a.test.".into()],
                account_ids: vec![acct],
                networks: vec!["10.0.0.0/8".into()],
                ..Default::default()
            })
            .await
            .expect("insert");
        let k = s
            .find_api_key_by_hash("h1")
            .await
            .expect("find")
            .expect("some");
        assert_eq!(k.id, id);
        assert_eq!(k.zones, vec!["a.test.", "b.test."]);
        assert_eq!(k.account_ids, vec![acct]);
        assert_eq!(k.networks, vec!["10.0.0.0/8"]);
        assert!(!k.disabled);

        assert!(s.set_api_key_disabled(id, true).await.expect("dis"));
        assert!(
            s.get_api_key(id)
                .await
                .expect("get")
                .expect("some")
                .disabled
        );

        s.touch_api_key(id, 42).await.expect("touch");
        assert_eq!(
            s.get_api_key(id)
                .await
                .expect("get")
                .expect("some")
                .last_used_at,
            Some(42)
        );

        assert!(
            s.update_api_key(
                id,
                &NewApiKey {
                    description: "new".into(),
                    role: Some(Role::Operator),
                    ..Default::default()
                }
            )
            .await
            .expect("update")
        );
        let k = s.get_api_key(id).await.expect("get").expect("some");
        assert_eq!(k.role, Role::Operator);
        assert!(k.zones.is_empty());

        s.add_api_key_zone(id, "c.test.").await.expect("add");
        s.add_api_key_zone(id, "c.test.").await.expect("add twice");
        assert_eq!(
            s.get_api_key(id)
                .await
                .expect("get")
                .expect("k")
                .zones
                .len(),
            1
        );

        assert_eq!(s.list_api_keys(None).await.expect("list").len(), 1);
        assert!(s.delete_api_key(id).await.expect("del"));
        assert!(s.find_api_key_by_hash("h1").await.expect("find").is_none());
        assert!(!s.delete_api_key(id).await.expect("del again"));
    }

    #[test]
    fn role_round_trip_and_expiry() {
        for r in [Role::Admin, Role::Operator, Role::Scoped] {
            assert_eq!(r.as_str().parse::<Role>().expect("parse"), r);
        }
        assert!("root".parse::<Role>().is_err());
        let k = ApiKeyRecord {
            id: 1,
            hint: String::new(),
            description: String::new(),
            role: Role::Admin,
            create_account_id: None,
            owner_user_id: None,
            expires_at: Some(100),
            last_used_at: None,
            created_at: 0,
            disabled: false,
            zones: vec![],
            account_ids: vec![],
            networks: vec![],
        };
        assert!(!k.is_expired(99));
        assert!(k.is_expired(100));
    }
}
