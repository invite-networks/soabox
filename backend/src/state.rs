//! Shared application state.

use std::sync::Arc;

use std::time::Duration;

use crate::config::Config;
use crate::keys::KeyService;
use crate::pdns::PdnsClient;
use crate::ratelimit::FailureLimiter;
use crate::store::Store;

/// Everything handlers need. Cheap to clone.
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Store,
    pub pdns: PdnsClient,
    pub keys: Arc<KeyService>,
    /// Failed sign-in attempts per client address and per address and
    /// username pair. Also limits password and code checks after sign-in.
    pub logins: Arc<FailureLimiter>,
    /// A higher limit per username across all addresses, so one attacker
    /// cannot lock an account out quickly but a distributed guess is slowed.
    pub login_users: Arc<FailureLimiter>,
    /// OIDC provider metadata, discovered on first use.
    pub oidc_metadata: Arc<tokio::sync::OnceCell<openidconnect::core::CoreProviderMetadata>>,
}

impl AppState {
    /// Connects to the database and builds the PowerDNS client.
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let store = Store::connect(config.database_url.expose()).await?;
        Self::with_store(config, store)
    }

    /// Builds state around an existing store. Used by tests.
    pub fn with_store(config: Config, store: Store) -> anyhow::Result<Self> {
        let pdns = PdnsClient::new(
            config.pdns_url.clone(),
            config.pdns_api_key.clone(),
            config.pdns_server_id.clone(),
            config.pdns_timeout,
        )?;
        Ok(Self {
            config: Arc::new(config),
            store,
            pdns,
            keys: Arc::new(KeyService::default()),
            logins: Arc::new(FailureLimiter::new(10, Duration::from_secs(300))),
            login_users: Arc::new(FailureLimiter::new(100, Duration::from_secs(300))),
            oidc_metadata: Arc::new(tokio::sync::OnceCell::new()),
        })
    }

    /// Keys accepted for decrypting data at rest, current first.
    pub fn secret_keys(&self) -> Vec<&[u8; 32]> {
        let mut keys = vec![&self.config.secret_key];
        if let Some(prev) = &self.config.secret_key_previous {
            keys.push(prev);
        }
        keys
    }
}
