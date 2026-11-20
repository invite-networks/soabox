//! Accounts: named groups of zones (and, from milestone 2, users).

use serde::Serialize;
use sqlx::Row;

use super::{Store, now};

/// An account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: i64,
    pub zones: Vec<String>,
}

impl Store {
    /// Creates an account. Returns its id.
    pub async fn create_account(&self, name: &str, description: &str) -> Result<i64, sqlx::Error> {
        sqlx::query(
            "INSERT INTO accounts (name, description, created_at) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(name)
        .bind(description)
        .bind(now())
        .fetch_one(&self.pool)
        .await?
        .try_get(0)
    }

    /// Updates an account's name and description.
    pub async fn update_account(
        &self,
        id: i64,
        name: &str,
        description: &str,
    ) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("UPDATE accounts SET name = $1, description = $2 WHERE id = $3")
                .bind(name)
                .bind(description)
                .bind(id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                > 0,
        )
    }

    /// Deletes an account. Its zones are not deleted from PowerDNS.
    pub async fn delete_account(&self, id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("DELETE FROM accounts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    /// Lists all accounts with their zones.
    pub async fn list_accounts(&self) -> Result<Vec<Account>, sqlx::Error> {
        let rows =
            sqlx::query("SELECT id, name, description, created_at FROM accounts ORDER BY name")
                .fetch_all(&self.pool)
                .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: i64 = r.try_get(0)?;
            out.push(Account {
                id,
                name: r.try_get(1)?,
                description: r.try_get(2)?,
                created_at: r.try_get(3)?,
                zones: self.account_zones(id).await?,
            });
        }
        Ok(out)
    }

    /// Gets an account by id.
    pub async fn get_account(&self, id: i64) -> Result<Option<Account>, sqlx::Error> {
        Ok(self.list_accounts().await?.into_iter().find(|a| a.id == id))
    }

    /// Finds an account by name.
    pub async fn find_account_by_name(&self, name: &str) -> Result<Option<Account>, sqlx::Error> {
        let row = sqlx::query("SELECT id FROM accounts WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            None => Ok(None),
            Some(r) => self.get_account(r.try_get(0)?).await,
        }
    }

    /// Every zone to account mapping, as zone name to (id, name).
    pub async fn all_zone_accounts(
        &self,
    ) -> Result<std::collections::HashMap<String, (i64, String)>, sqlx::Error> {
        sqlx::query(
            "SELECT az.zone_name, a.id, a.name FROM account_zones az JOIN accounts a ON a.id = az.account_id",
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| Ok((r.try_get(0)?, (r.try_get(1)?, r.try_get(2)?))))
        .collect()
    }

    /// Replaces the zones owned by an account.
    pub async fn set_account_zones(&self, id: i64, zones: &[String]) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM account_zones WHERE account_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let mut zones: Vec<&String> = zones.iter().collect();
        zones.sort();
        zones.dedup();
        for z in zones {
            sqlx::query("DELETE FROM account_zones WHERE zone_name = $1")
                .bind(z)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO account_zones (account_id, zone_name) VALUES ($1, $2)")
                .bind(id)
                .bind(z)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }

    /// Zones owned by an account.
    pub async fn account_zones(&self, id: i64) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query("SELECT zone_name FROM account_zones WHERE account_id = $1 ORDER BY zone_name")
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| r.try_get(0))
            .collect()
    }

    /// Zones owned by any of the given accounts.
    pub async fn zones_for_accounts(&self, ids: &[i64]) -> Result<Vec<String>, sqlx::Error> {
        let mut out = Vec::new();
        for id in ids {
            out.extend(self.account_zones(*id).await?);
        }
        Ok(out)
    }

    /// The account that owns a zone, if any.
    pub async fn zone_account(&self, zone: &str) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query("SELECT account_id FROM account_zones WHERE zone_name = $1")
            .bind(zone)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.try_get(0))
            .transpose()
    }

    /// Assigns a zone to an account, moving it from any other account.
    pub async fn set_zone_account(&self, zone: &str, account_id: i64) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM account_zones WHERE zone_name = $1")
            .bind(zone)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO account_zones (account_id, zone_name) VALUES ($1, $2)")
            .bind(account_id)
            .bind(zone)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    }

    /// Removes a zone from whichever account owns it.
    pub async fn clear_zone_account(&self, zone: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM account_zones WHERE zone_name = $1")
            .bind(zone)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes every SOAbox reference to a deleted zone. History is kept.
    pub async fn forget_zone(&self, zone: &str) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        for sql in [
            "DELETE FROM account_zones WHERE zone_name = $1",
            "DELETE FROM api_key_zones WHERE zone_name = $1",
            "DELETE FROM user_zones WHERE zone_name = $1",
        ] {
            sqlx::query(sql).bind(zone).execute(&mut *tx).await?;
        }
        tx.commit().await
    }
}

#[cfg(test)]
mod tests {
    use crate::store::testing;

    #[tokio::test]
    async fn account_zone_mapping() {
        let (s, _d) = testing::sqlite().await;
        let a = s.create_account("acme", "Acme").await.expect("a");
        let b = s.create_account("beta", "").await.expect("b");
        assert!(s.create_account("acme", "").await.is_err());

        s.set_zone_account("x.test.", a).await.expect("set");
        assert_eq!(s.zone_account("x.test.").await.expect("q"), Some(a));
        s.set_zone_account("x.test.", b).await.expect("move");
        assert_eq!(s.account_zones(a).await.expect("q"), Vec::<String>::new());
        assert_eq!(
            s.zones_for_accounts(&[a, b]).await.expect("q"),
            vec!["x.test."]
        );

        let found = s
            .find_account_by_name("beta")
            .await
            .expect("q")
            .expect("some");
        assert_eq!(found.zones, vec!["x.test."]);

        s.forget_zone("x.test.").await.expect("forget");
        assert_eq!(s.zone_account("x.test.").await.expect("q"), None);

        assert!(s.update_account(a, "acme2", "d").await.expect("upd"));
        assert_eq!(s.get_account(a).await.expect("q").expect("a").name, "acme2");
        assert!(s.delete_account(a).await.expect("del"));
        assert_eq!(s.list_accounts().await.expect("l").len(), 1);
    }
}
