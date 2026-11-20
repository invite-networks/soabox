//! Database access. Only this module runs SQL.
//!
//! Queries are portable between SQLite and Postgres and use `$N` placeholders,
//! which both accept. Times are Unix seconds and flags are integers so both
//! databases decode the same way through the `Any` driver.

mod accounts;
mod api_keys;
mod history;
mod settings;

pub use accounts::Account;
pub use api_keys::{ApiKeyRecord, NewApiKey, Role};
pub use history::{HistoryEntry, HistoryFilter, NewHistory};

use std::str::FromStr;
use std::time::Duration;

use sqlx::any::{AnyConnectOptions, AnyPoolOptions};
use sqlx::{AnyPool, Executor};

/// Which database is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
}

/// Handle to the database.
#[derive(Debug, Clone)]
pub struct Store {
    pool: AnyPool,
    backend: Backend,
}

/// Current Unix time in seconds.
pub fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

impl Store {
    /// Connects, creating the SQLite file and its directory if needed, and
    /// runs pending migrations.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        sqlx::any::install_default_drivers();
        let backend = if url.starts_with("postgres") {
            Backend::Postgres
        } else if url.starts_with("sqlite") {
            Backend::Sqlite
        } else {
            anyhow::bail!("SOABOX_DATABASE_URL must start with sqlite:// or postgres://");
        };

        let url = match backend {
            Backend::Sqlite => prepare_sqlite_url(url)?,
            Backend::Postgres => url.to_owned(),
        };
        let opts = AnyConnectOptions::from_str(&url)?;
        let pool = AnyPoolOptions::new()
            .max_connections(match backend {
                Backend::Sqlite => 4,
                Backend::Postgres => 10,
            })
            .acquire_timeout(Duration::from_secs(10))
            .after_connect(move |conn, _| {
                Box::pin(async move {
                    if backend == Backend::Sqlite {
                        conn.execute("PRAGMA foreign_keys = ON").await?;
                        conn.execute("PRAGMA journal_mode = WAL").await?;
                        conn.execute("PRAGMA busy_timeout = 5000").await?;
                    }
                    Ok(())
                })
            })
            .connect_with(opts)
            .await?;

        let store = Self { pool, backend };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        match self.backend {
            Backend::Sqlite => {
                sqlx::migrate!("./migrations/sqlite")
                    .run(&self.pool)
                    .await?
            }
            Backend::Postgres => {
                sqlx::migrate!("./migrations/postgres")
                    .run(&self.pool)
                    .await?
            }
        }
        tracing::info!(backend = ?self.backend, "migrations complete");
        Ok(())
    }

    /// The database in use.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// The connection pool.
    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    /// Readiness probe.
    pub async fn ping(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
    }

    /// Writes a consistent copy of a SQLite database to `dest`.
    pub async fn backup_sqlite(&self, dest: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.backend == Backend::Sqlite,
            "backup is for SQLite; use pg_dump for Postgres"
        );
        anyhow::ensure!(
            !dest.contains('\''),
            "backup path must not contain a single quote"
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("VACUUM INTO '{dest}'")))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn prepare_sqlite_url(url: &str) -> anyhow::Result<String> {
    let path = url
        .trim_start_matches("sqlite://")
        .trim_start_matches("sqlite:")
        .split('?')
        .next()
        .unwrap_or_default();
    if path != ":memory:"
        && !path.is_empty()
        && let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(if url.contains('?') {
        url.to_owned()
    } else {
        format!("{url}?mode=rwc")
    })
}

#[cfg(test)]
pub(crate) mod testing {
    use super::Store;

    /// A fresh SQLite store in a temporary directory, or Postgres when
    /// `SOABOX_TEST_POSTGRES_URL` is set and `postgres` is true.
    pub async fn sqlite() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let url = format!("sqlite://{}/t.db", dir.path().display());
        (Store::connect(&url).await.expect("store"), dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connects_and_migrates_sqlite() {
        let (store, _dir) = testing::sqlite().await;
        store.ping().await.expect("ping");
        assert_eq!(store.backend(), Backend::Sqlite);
    }

    #[tokio::test]
    async fn rejects_unknown_scheme() {
        assert!(Store::connect("mysql://x").await.is_err());
    }

    #[tokio::test]
    async fn sqlite_backup_writes_file() {
        let (store, dir) = testing::sqlite().await;
        let dest = dir.path().join("b.db");
        store
            .backup_sqlite(dest.to_str().expect("path"))
            .await
            .expect("backup");
        assert!(dest.exists());
    }
}
