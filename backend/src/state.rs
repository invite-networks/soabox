//! Shared application state.

use std::sync::Arc;

use crate::config::Config;
use crate::keys::KeyService;
use crate::pdns::PdnsClient;
use crate::store::Store;

/// Everything handlers need. Cheap to clone.
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Store,
    pub pdns: PdnsClient,
    pub keys: Arc<KeyService>,
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
        })
    }
}
