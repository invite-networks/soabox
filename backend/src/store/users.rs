//! Users, sessions, and user zone access.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::any::AnyRow;

use super::{Backend, Store, now};

/// A user's role. Managed only in SOAbox (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Operator,
    User,
}

/// Whether a user may sign in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Pending,
    Disabled,
}

macro_rules! str_enum {
    ($t:ty, $($v:ident => $s:literal),+) => {
        impl $t {
            /// Database spelling.
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s),+ }
            }
        }
        impl FromStr for $t {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s { $($s => Ok(Self::$v),)+ other => Err(format!("unknown value {other:?}")) }
            }
        }
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
        }
    };
}

str_enum!(UserRole, Admin => "admin", Operator => "operator", User => "user");
str_enum!(UserStatus, Active => "active", Pending => "pending", Disabled => "disabled");

/// A stored user. Secrets are never serialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub role: UserRole,
    /// `local` or `oidc`.
    pub auth_source: String,
    pub status: UserStatus,
    pub totp_enabled: bool,
    #[serde(skip)]
    pub totp_last_step: i64,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
    #[serde(skip)]
    pub oidc_issuer: Option<String>,
    #[serde(skip)]
    pub oidc_subject: Option<String>,
    #[serde(skip)]
    pub password_hash: Option<String>,
    #[serde(skip)]
    pub totp_secret_enc: Option<Vec<u8>>,
}

impl User {
    /// True for users who sign in with a password.
    pub fn is_local(&self) -> bool {
        self.auth_source == "local"
    }
}

/// Fields for a new user.
#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub role: UserRole,
    pub auth_source: &'static str,
    pub oidc_issuer: Option<String>,
    pub oidc_subject: Option<String>,
    pub password_hash: Option<String>,
    pub status: UserStatus,
}

/// A session joined with its user.
#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub id_hash: String,
    pub csrf_token: String,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub expires_at: i64,
    pub user: User,
}

const USER_COLUMNS: &str = "u.id, u.username, u.display_name, u.email, u.role, u.auth_source, \
     u.oidc_issuer, u.oidc_subject, u.password_hash, u.totp_secret_enc, u.totp_enabled, \
     u.totp_last_step, u.status, u.created_at, u.last_login_at";

fn user_from_row(r: &AnyRow) -> Result<User, sqlx::Error> {
    let decode = |e: String| sqlx::Error::Decode(e.into());
    Ok(User {
        id: r.try_get("id")?,
        username: r.try_get("username")?,
        display_name: r.try_get("display_name")?,
        email: r.try_get("email")?,
        role: r.try_get::<String, _>("role")?.parse().map_err(decode)?,
        auth_source: r.try_get("auth_source")?,
        oidc_issuer: r.try_get("oidc_issuer")?,
        oidc_subject: r.try_get("oidc_subject")?,
        password_hash: r.try_get("password_hash")?,
        totp_secret_enc: r.try_get("totp_secret_enc")?,
        totp_enabled: r.try_get::<i64, _>("totp_enabled")? != 0,
        totp_last_step: r.try_get("totp_last_step")?,
        status: r.try_get::<String, _>("status")?.parse().map_err(decode)?,
        created_at: r.try_get("created_at")?,
        last_login_at: r.try_get("last_login_at")?,
    })
}

impl Store {
    /// Creates a user. Returns its id.
    pub async fn create_user(&self, u: &NewUser) -> Result<i64, sqlx::Error> {
        sqlx::query(
            "INSERT INTO users (username, display_name, email, role, auth_source, oidc_issuer, \
             oidc_subject, password_hash, status, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
        )
        .bind(&u.username)
        .bind(&u.display_name)
        .bind(&u.email)
        .bind(u.role.as_str())
        .bind(u.auth_source)
        .bind(&u.oidc_issuer)
        .bind(&u.oidc_subject)
        .bind(&u.password_hash)
        .bind(u.status.as_str())
        .bind(now())
        .fetch_one(&self.pool)
        .await?
        .try_get(0)
    }

    async fn find_user_where(
        &self,
        clause: &str,
        bind: Bind<'_>,
    ) -> Result<Option<User>, sqlx::Error> {
        let sql = format!("SELECT {USER_COLUMNS} FROM users u WHERE {clause}");
        let q = sqlx::query(sqlx::AssertSqlSafe(sql));
        let q = match bind {
            Bind::I(i) => q.bind(i),
            Bind::S(s) => q.bind(s.to_owned()),
        };
        q.fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(user_from_row)
            .transpose()
    }

    /// Gets a user by id.
    pub async fn get_user(&self, id: i64) -> Result<Option<User>, sqlx::Error> {
        self.find_user_where("u.id = $1", Bind::I(id)).await
    }

    /// Finds a user by username.
    pub async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, sqlx::Error> {
        self.find_user_where("u.username = $1", Bind::S(username))
            .await
    }

    /// Finds an OIDC user by issuer and subject.
    pub async fn find_user_by_oidc(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {USER_COLUMNS} FROM users u WHERE u.oidc_issuer = $1 AND u.oidc_subject = $2"
        )))
        .bind(issuer)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await?
        .as_ref()
        .map(user_from_row)
        .transpose()
    }

    /// Lists all users.
    pub async fn list_users(&self) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {USER_COLUMNS} FROM users u ORDER BY u.username"
        )))
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(user_from_row)
        .collect()
    }

    /// Number of active Admins.
    pub async fn count_active_admins(&self) -> Result<i64, sqlx::Error> {
        sqlx::query("SELECT COUNT(*) FROM users WHERE role = 'admin' AND status = 'active'")
            .fetch_one(&self.pool)
            .await?
            .try_get(0)
    }

    /// Updates profile, role, and status.
    pub async fn update_user(
        &self,
        id: i64,
        display_name: &str,
        email: &str,
        role: UserRole,
        status: UserStatus,
    ) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query(
            "UPDATE users SET display_name = $1, email = $2, role = $3, status = $4 WHERE id = $5",
        )
        .bind(display_name)
        .bind(email)
        .bind(role.as_str())
        .bind(status.as_str())
        .bind(id)
        .execute(&self.pool)
        .await?
        .rows_affected()
            > 0)
    }

    /// Sets or clears a password hash.
    pub async fn set_password_hash(&self, id: i64, hash: &str) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
                .bind(hash)
                .bind(id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                > 0,
        )
    }

    /// Stores an encrypted TOTP secret and whether it is enabled.
    pub async fn set_totp(
        &self,
        id: i64,
        secret_enc: Option<&[u8]>,
        enabled: bool,
    ) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("UPDATE users SET totp_secret_enc = $1, totp_enabled = $2 WHERE id = $3")
                .bind(secret_enc.map(<[u8]>::to_vec))
                .bind(i64::from(enabled))
                .bind(id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                > 0,
        )
    }

    /// Consumes a TOTP time step for a user. Returns true only for the one
    /// request that moves the last accepted step forward, so a code cannot be
    /// used twice even by concurrent requests.
    pub async fn consume_totp_step(&self, id: i64, step: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE users SET totp_last_step = $1 WHERE id = $2 AND totp_last_step < $1",
        )
        .bind(step)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Ends every session of a user except one.
    pub async fn delete_other_sessions(
        &self,
        user_id: i64,
        keep: &str,
    ) -> Result<u64, sqlx::Error> {
        Ok(
            sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id_hash <> $2")
                .bind(user_id)
                .bind(keep)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    /// Users with an encrypted TOTP secret, for key rotation.
    pub async fn users_with_totp(&self) -> Result<Vec<(i64, Vec<u8>)>, sqlx::Error> {
        sqlx::query("SELECT id, totp_secret_enc FROM users WHERE totp_secret_enc IS NOT NULL")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| Ok((r.try_get(0)?, r.try_get(1)?)))
            .collect()
    }

    /// Records a successful sign-in.
    pub async fn touch_user_login(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET last_login_at = $1 WHERE id = $2")
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes a user. Sessions and access rows cascade.
    pub async fn delete_user(&self, id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    /// Zones assigned directly to a user.
    pub async fn user_zones(&self, id: i64) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query("SELECT zone_name FROM user_zones WHERE user_id = $1 ORDER BY zone_name")
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| r.try_get(0))
            .collect()
    }

    /// Accounts a user belongs to.
    pub async fn user_account_ids(&self, id: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query("SELECT account_id FROM account_users WHERE user_id = $1 ORDER BY account_id")
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| r.try_get(0))
            .collect()
    }

    /// Members of an account.
    pub async fn account_user_ids(&self, account_id: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query("SELECT user_id FROM account_users WHERE account_id = $1 ORDER BY user_id")
            .bind(account_id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| r.try_get(0))
            .collect()
    }

    /// Every zone a user can reach: direct zones plus their accounts' zones.
    pub async fn user_access_zones(&self, id: i64) -> Result<Vec<String>, sqlx::Error> {
        let mut zones = self.user_zones(id).await?;
        zones.extend(
            self.zones_for_accounts(&self.user_account_ids(id).await?)
                .await?,
        );
        zones.sort();
        zones.dedup();
        Ok(zones)
    }

    /// Replaces a user's direct zones and account memberships.
    pub async fn set_user_access(
        &self,
        id: i64,
        zones: &[String],
        account_ids: &[i64],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM user_zones WHERE user_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM account_users WHERE user_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let mut zones: Vec<&String> = zones.iter().collect();
        zones.sort();
        zones.dedup();
        for z in zones {
            sqlx::query("INSERT INTO user_zones (user_id, zone_name) VALUES ($1, $2)")
                .bind(id)
                .bind(z)
                .execute(&mut *tx)
                .await?;
        }
        let mut accts = account_ids.to_vec();
        accts.sort_unstable();
        accts.dedup();
        for a in accts {
            sqlx::query("INSERT INTO account_users (account_id, user_id) VALUES ($1, $2)")
                .bind(a)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }

    /// Replaces an account's members.
    pub async fn set_account_users(
        &self,
        account_id: i64,
        user_ids: &[i64],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM account_users WHERE account_id = $1")
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
        let mut ids = user_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        for u in ids {
            sqlx::query("INSERT INTO account_users (account_id, user_id) VALUES ($1, $2)")
                .bind(account_id)
                .bind(u)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }

    /// Adds one zone to a user's direct zones.
    pub async fn add_user_zone(&self, id: i64, zone: &str) -> Result<(), sqlx::Error> {
        let sql = match self.backend {
            Backend::Sqlite => {
                "INSERT OR IGNORE INTO user_zones (user_id, zone_name) VALUES ($1, $2)"
            }
            Backend::Postgres => {
                "INSERT INTO user_zones (user_id, zone_name) VALUES ($1, $2) ON CONFLICT DO NOTHING"
            }
        };
        sqlx::query(sql)
            .bind(id)
            .bind(zone)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Creates a session.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_session(
        &self,
        id_hash: &str,
        user_id: i64,
        csrf_token: &str,
        expires_at: i64,
        client_ip: &str,
        user_agent: &str,
    ) -> Result<(), sqlx::Error> {
        let at = now();
        sqlx::query(
            "INSERT INTO sessions (id_hash, user_id, csrf_token, created_at, last_seen_at, \
             expires_at, client_ip, user_agent) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(id_hash)
        .bind(user_id)
        .bind(csrf_token)
        .bind(at)
        .bind(at)
        .bind(expires_at)
        .bind(client_ip)
        .bind(user_agent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Gets a session with its user.
    pub async fn get_session(&self, id_hash: &str) -> Result<Option<SessionRecord>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT s.id_hash, s.csrf_token, s.created_at AS s_created, s.last_seen_at, \
             s.expires_at, {USER_COLUMNS} FROM sessions s JOIN users u ON u.id = s.user_id \
             WHERE s.id_hash = $1"
        )))
        .bind(id_hash)
        .fetch_optional(&self.pool)
        .await?;
        let Some(r) = row else { return Ok(None) };
        Ok(Some(SessionRecord {
            id_hash: r.try_get("id_hash")?,
            csrf_token: r.try_get("csrf_token")?,
            created_at: r.try_get("s_created")?,
            last_seen_at: r.try_get("last_seen_at")?,
            expires_at: r.try_get("expires_at")?,
            user: user_from_row(&r)?,
        }))
    }

    /// Updates a session's last-seen time.
    pub async fn touch_session(&self, id_hash: &str, at: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE sessions SET last_seen_at = $1 WHERE id_hash = $2")
            .bind(at)
            .bind(id_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes one session.
    pub async fn delete_session(&self, id_hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE id_hash = $1")
            .bind(id_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Ends every session of a user.
    pub async fn delete_user_sessions(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    /// Removes expired sessions.
    pub async fn purge_sessions(&self, idle_cutoff: i64) -> Result<u64, sqlx::Error> {
        Ok(
            sqlx::query("DELETE FROM sessions WHERE expires_at <= $1 OR last_seen_at <= $2")
                .bind(now())
                .bind(idle_cutoff)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
}

enum Bind<'a> {
    I(i64),
    S(&'a str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing;

    fn local(name: &str, role: UserRole) -> NewUser {
        NewUser {
            username: name.into(),
            display_name: name.into(),
            email: String::new(),
            role,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: Some("h".into()),
            status: UserStatus::Active,
        }
    }

    #[tokio::test]
    async fn user_lifecycle_and_access() {
        let (s, _d) = testing::sqlite().await;
        let id = s
            .create_user(&local("alice", UserRole::User))
            .await
            .expect("create");
        assert!(
            s.create_user(&local("alice", UserRole::User))
                .await
                .is_err()
        );
        let u = s
            .find_user_by_username("alice")
            .await
            .expect("q")
            .expect("u");
        assert_eq!(u.id, id);
        assert!(u.is_local());
        assert_eq!(s.count_active_admins().await.expect("count"), 0);

        let acct = s.create_account("acme", "").await.expect("acct");
        s.set_zone_account("b.test.", acct).await.expect("map");
        s.set_user_access(id, &["a.test.".into()], &[acct])
            .await
            .expect("access");
        assert_eq!(
            s.user_access_zones(id).await.expect("z"),
            vec!["a.test.", "b.test."]
        );
        assert_eq!(s.account_user_ids(acct).await.expect("m"), vec![id]);
        s.add_user_zone(id, "c.test.").await.expect("add");
        s.add_user_zone(id, "c.test.").await.expect("add twice");
        s.set_account_users(acct, &[]).await.expect("members");
        assert_eq!(
            s.user_access_zones(id).await.expect("z"),
            vec!["a.test.", "c.test."]
        );

        assert!(
            s.update_user(
                id,
                "Alice",
                "a@example.test",
                UserRole::Operator,
                UserStatus::Disabled
            )
            .await
            .expect("upd")
        );
        let u = s.get_user(id).await.expect("q").expect("u");
        assert_eq!(u.role, UserRole::Operator);
        assert_eq!(u.status, UserStatus::Disabled);

        s.set_totp(id, Some(b"enc"), true).await.expect("totp");
        assert_eq!(s.users_with_totp().await.expect("t").len(), 1);
        assert!(s.get_user(id).await.expect("q").expect("u").totp_enabled);
        s.set_password_hash(id, "h2").await.expect("pw");
        s.touch_user_login(id).await.expect("login");
        assert!(
            s.get_user(id)
                .await
                .expect("q")
                .expect("u")
                .last_login_at
                .is_some()
        );

        let json =
            serde_json::to_value(s.get_user(id).await.expect("q").expect("u")).expect("json");
        assert!(json.get("password_hash").is_none());
        assert!(json.get("totp_secret_enc").is_none());

        assert_eq!(s.list_users().await.expect("list").len(), 1);
        assert!(s.delete_user(id).await.expect("del"));
    }

    #[tokio::test]
    async fn oidc_users_and_sessions() {
        let (s, _d) = testing::sqlite().await;
        let mut nu = local("bob", UserRole::Admin);
        nu.auth_source = "oidc";
        nu.oidc_issuer = Some("https://idp.test".into());
        nu.oidc_subject = Some("sub-1".into());
        nu.password_hash = None;
        let id = s.create_user(&nu).await.expect("create");
        assert_eq!(
            s.find_user_by_oidc("https://idp.test", "sub-1")
                .await
                .expect("q")
                .expect("u")
                .id,
            id
        );
        assert!(
            s.find_user_by_oidc("https://idp.test", "sub-2")
                .await
                .expect("q")
                .is_none()
        );

        s.create_session("h1", id, "csrf", now() + 100, "192.0.2.1", "ua")
            .await
            .expect("sess");
        let sess = s.get_session("h1").await.expect("q").expect("s");
        assert_eq!(sess.user.id, id);
        assert_eq!(sess.csrf_token, "csrf");
        s.touch_session("h1", 5).await.expect("touch");
        assert_eq!(s.purge_sessions(10).await.expect("purge"), 1);
        s.create_session("h2", id, "c", now() + 100, "", "")
            .await
            .expect("sess");
        assert_eq!(s.delete_user_sessions(id).await.expect("del"), 1);
        s.create_session("h3", id, "c", now() + 100, "", "")
            .await
            .expect("sess");
        s.delete_session("h3").await.expect("del");
        assert!(s.get_session("h3").await.expect("q").is_none());
    }
}
