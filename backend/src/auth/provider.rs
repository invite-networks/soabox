//! Where the single sign-on provider settings come from, and the cached
//! provider discovery document.
//!
//! Precedence: when `SOABOX_OIDC_ISSUER` is set, the environment wins and
//! the stored settings are ignored and cannot be edited. Otherwise the
//! settings an Admin saved in the UI apply. They are read from the database
//! on each use, so a change takes effect at once on every instance without a
//! restart.

use std::time::{Duration, Instant};

use openidconnect::IssuerUrl;
use openidconnect::core::CoreProviderMetadata;
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

use super::crypto;
use crate::config::{Mode, OidcConfig, Secret};
use crate::errors::ApiError;
use crate::state::AppState;
use crate::store::OidcProviderRecord;

/// Button text when none is set.
pub const DEFAULT_BUTTON_LABEL: &str = "Sign in with SSO";
/// Longest accepted button label.
pub const MAX_BUTTON_LABEL: usize = 64;
/// How long a discovery document is reused before it is fetched again, so
/// provider signing key rotation is picked up.
pub const METADATA_TTL: Duration = Duration::from_secs(3600);

/// Shortest time between forced refreshes for an unknown signing key, so a
/// stream of bad tokens cannot make SOAbox hammer the provider.
pub const METADATA_MIN_REFRESH: Duration = Duration::from_secs(60);

/// Where the active provider settings come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum OidcSource {
    /// `SOABOX_OIDC_*` environment variables. Read-only in the UI.
    Environment,
    /// Settings saved by an Admin.
    Database,
}

struct CachedMetadata {
    issuer: String,
    fetched: Instant,
    metadata: CoreProviderMetadata,
}

/// Holds the discovery document for the current issuer. At most one entry
/// is kept. It is replaced when the issuer changes, when it is older than
/// [`METADATA_TTL`], or when the settings are saved.
#[derive(Default)]
pub struct MetadataCache {
    inner: tokio::sync::Mutex<Option<CachedMetadata>>,
}

impl std::fmt::Debug for MetadataCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MetadataCache")
    }
}

impl MetadataCache {
    /// Drops the cached document so the next sign-in discovers again.
    pub async fn clear(&self) {
        *self.inner.lock().await = None;
    }

    /// The discovery document for `issuer`, fetched when missing or stale.
    pub async fn get(&self, issuer: &Url) -> Result<CoreProviderMetadata, ApiError> {
        self.load(issuer, METADATA_TTL).await
    }

    /// Fetches the discovery document and signing keys again, for an ID token
    /// signed with a key the cached set does not contain. Refreshes at most
    /// once per [`METADATA_MIN_REFRESH`]. A failed fetch keeps the cached
    /// document and returns the error, so sign-in fails closed.
    pub async fn refresh(&self, issuer: &Url) -> Result<CoreProviderMetadata, ApiError> {
        self.load(issuer, METADATA_MIN_REFRESH).await
    }

    async fn load(
        &self,
        issuer: &Url,
        max_age: Duration,
    ) -> Result<CoreProviderMetadata, ApiError> {
        let issuer = issuer.as_str().trim_end_matches('/').to_owned();
        let mut slot = self.inner.lock().await;
        if let Some(c) = slot.as_ref()
            && c.issuer == issuer
            && c.fetched.elapsed() < max_age
        {
            return Ok(c.metadata.clone());
        }
        let issuer_url = IssuerUrl::new(issuer.clone()).map_err(ApiError::internal)?;
        let metadata =
            CoreProviderMetadata::discover_async(issuer_url, &super::oidc::http_client()?)
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "OIDC discovery failed");
                    ApiError::UpstreamUnavailable
                })?;
        *slot = Some(CachedMetadata {
            issuer,
            fetched: Instant::now(),
            metadata: metadata.clone(),
        });
        Ok(metadata)
    }
}

/// The redirect URI to register with the provider.
pub fn redirect_uri(public_url: &Url) -> String {
    format!(
        "{}/auth/oidc/callback",
        public_url.as_str().trim_end_matches('/')
    )
}

/// Turns a stored row into usable settings. Returns `None` when it is turned
/// off, incomplete, or the secret cannot be decrypted with either key.
pub fn from_record(rec: &OidcProviderRecord, keys: &[&[u8; 32]]) -> Option<OidcConfig> {
    if !rec.enabled || rec.client_id.is_empty() {
        return None;
    }
    let issuer = Url::parse(&rec.issuer).ok()?;
    let secret = rec
        .client_secret_enc
        .as_deref()
        .and_then(|enc| crypto::decrypt(crypto::PURPOSE_OIDC_CLIENT_SECRET, keys, enc));
    let Some(secret) = secret.and_then(|s| String::from_utf8(s).ok()) else {
        tracing::warn!("stored OIDC client secret cannot be decrypted; single sign-on is off");
        return None;
    };
    Some(OidcConfig {
        issuer,
        client_id: rec.client_id.clone(),
        client_secret: Secret::new(secret),
        button_label: label_or_default(&rec.button_label),
    })
}

fn label_or_default(label: &str) -> String {
    let l = label.trim();
    if l.is_empty() {
        DEFAULT_BUTTON_LABEL.to_owned()
    } else {
        l.to_owned()
    }
}

/// The provider settings in force right now, if single sign-on is on.
pub async fn active(state: &AppState) -> Result<Option<OidcConfig>, ApiError> {
    if let Some(env) = &state.config.oidc {
        return Ok(Some(env.clone()));
    }
    Ok(state
        .store
        .oidc_provider()
        .await?
        .and_then(|rec| from_record(&rec, &state.secret_keys())))
}

/// The provider settings as an Admin sees them. The secret is never included.
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
pub struct OidcSettings {
    /// Where the settings in force come from.
    pub source: OidcSource,
    /// True when single sign-on is offered on the sign-in page.
    pub enabled: bool,
    pub issuer: String,
    pub client_id: String,
    /// True when a client secret is stored. The value is never returned.
    pub client_secret_set: bool,
    pub button_label: String,
    /// The callback URL to register with the provider.
    pub redirect_uri: String,
}

/// Changes to the provider settings.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct OidcSettingsUpdate {
    pub enabled: bool,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub client_id: String,
    /// A new client secret. Omit or leave empty to keep the stored one.
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub button_label: String,
}

/// Reads the settings for the Admin page.
pub async fn view(state: &AppState) -> Result<OidcSettings, ApiError> {
    let redirect = redirect_uri(&state.config.public_url);
    if let Some(env) = &state.config.oidc {
        return Ok(OidcSettings {
            source: OidcSource::Environment,
            enabled: true,
            issuer: env.issuer.to_string(),
            client_id: env.client_id.clone(),
            client_secret_set: true,
            button_label: env.button_label.clone(),
            redirect_uri: redirect,
        });
    }
    let rec = state.store.oidc_provider().await?.unwrap_or_default();
    Ok(OidcSettings {
        source: OidcSource::Database,
        enabled: rec.enabled,
        issuer: rec.issuer,
        client_id: rec.client_id,
        client_secret_set: rec.client_secret_enc.is_some(),
        button_label: label_or_default(&rec.button_label),
        redirect_uri: redirect,
    })
}

/// Checks an issuer URL. It must be `https`, except in `dev` mode where
/// `http` is allowed for a local test provider.
pub fn validate_issuer(raw: &str, mode: Mode) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|_| "enter the issuer as a full URL".to_owned())?;
    let ok = match url.scheme() {
        "https" => true,
        "http" => mode == Mode::Dev,
        _ => false,
    };
    if !ok {
        return Err("the issuer must use https".to_owned());
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("the issuer must include a host".to_owned());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("the issuer cannot have a query or fragment".to_owned());
    }
    Ok(url)
}

/// Validates an update against the stored row and returns the row to save.
/// `secret_enc` is the encrypted new secret, if one was given.
pub fn merge(
    current: Option<OidcProviderRecord>,
    req: &OidcSettingsUpdate,
    secret_enc: Option<Vec<u8>>,
    mode: Mode,
) -> Result<OidcProviderRecord, String> {
    let issuer = req.issuer.trim();
    let client_id = req.client_id.trim();
    let label = req.button_label.trim();
    if !issuer.is_empty() {
        validate_issuer(issuer, mode)?;
    }
    if client_id.len() > 255 {
        return Err("the client ID is too long".to_owned());
    }
    if label.chars().count() > MAX_BUTTON_LABEL {
        return Err(format!(
            "the button label may be up to {MAX_BUTTON_LABEL} characters"
        ));
    }
    let client_secret_enc = secret_enc.or_else(|| current.and_then(|c| c.client_secret_enc));
    if req.enabled {
        if issuer.is_empty() {
            return Err("enter the issuer URL".to_owned());
        }
        if client_id.is_empty() {
            return Err("enter the client ID".to_owned());
        }
        if client_secret_enc.is_none() {
            return Err("enter the client secret".to_owned());
        }
    }
    Ok(OidcProviderRecord {
        enabled: req.enabled,
        issuer: issuer.to_owned(),
        client_id: client_id.to_owned(),
        client_secret_enc,
        button_label: label.to_owned(),
        updated_at: 0,
    })
}

/// Validates and stores an update, then drops the cached discovery document.
pub async fn save(state: &AppState, req: &OidcSettingsUpdate) -> Result<OidcSettings, ApiError> {
    if state.config.oidc.is_some() {
        return Err(ApiError::Conflict(
            "single sign-on is set by environment variables and cannot be changed here".into(),
        ));
    }
    let new_secret = req
        .client_secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if new_secret.is_some_and(|s| s.len() > 1024) {
        return Err(ApiError::Validation("the client secret is too long".into()));
    }
    let secret_enc = new_secret
        .map(|s| {
            crypto::encrypt(
                crypto::PURPOSE_OIDC_CLIENT_SECRET,
                &state.config.secret_key,
                s.as_bytes(),
            )
        })
        .transpose()
        .map_err(ApiError::Internal)?;
    let current = state.store.oidc_provider().await?;
    let rec = merge(current, req, secret_enc, state.config.mode).map_err(ApiError::Validation)?;
    state.store.put_oidc_provider(&rec).await?;
    state.oidc_metadata.clear().await;
    view(state).await
}

/// Re-encrypts the stored client secret with the current key. Returns
/// whether there was one.
pub async fn rotate_secret(state: &AppState) -> anyhow::Result<bool> {
    let Some(mut rec) = state.store.oidc_provider().await? else {
        return Ok(false);
    };
    let Some(enc) = rec.client_secret_enc.take() else {
        return Ok(false);
    };
    let plain = crypto::decrypt(
        crypto::PURPOSE_OIDC_CLIENT_SECRET,
        &state.secret_keys(),
        &enc,
    )
    .ok_or_else(|| anyhow::anyhow!("OIDC client secret cannot be decrypted with either key"))?;
    rec.client_secret_enc = Some(crypto::encrypt(
        crypto::PURPOSE_OIDC_CLIENT_SECRET,
        &state.config.secret_key,
        &plain,
    )?);
    state.store.put_oidc_provider(&rec).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn mount_discovery(idp: &MockServer) {
        let issuer = format!("{}/tenant", idp.uri());
        Mock::given(method("GET"))
            .and(path("/tenant/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "issuer": issuer,
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint": format!("{issuer}/token"),
                "jwks_uri": format!("{issuer}/keys"),
                "response_types_supported": ["code"],
                "subject_types_supported": ["public"],
                "id_token_signing_alg_values_supported": ["RS256"],
            })))
            .mount(idp)
            .await;
        Mock::given(method("GET"))
            .and(path("/tenant/keys"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keys": []})))
            .mount(idp)
            .await;
    }

    async fn discoveries(idp: &MockServer) -> usize {
        idp.received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .filter(|r| r.url.path().ends_with("openid-configuration"))
            .count()
    }

    async fn age(cache: &MetadataCache, by: Duration) {
        let mut slot = cache.inner.lock().await;
        let entry = slot.as_mut().expect("cached");
        entry.fetched = Instant::now().checked_sub(by).expect("instant");
    }

    #[tokio::test]
    async fn unknown_key_refresh_is_bounded_and_fails_closed() {
        let idp = MockServer::start().await;
        mount_discovery(&idp).await;
        let issuer = Url::parse(&format!("{}/tenant", idp.uri())).expect("url");
        let cache = MetadataCache::default();

        cache.get(&issuer).await.expect("first discovery");
        assert_eq!(discoveries(&idp).await, 1);
        cache.refresh(&issuer).await.expect("too soon");
        assert_eq!(discoveries(&idp).await, 1, "refresh is rate limited");

        age(&cache, METADATA_MIN_REFRESH + Duration::from_secs(1)).await;
        cache.refresh(&issuer).await.expect("refresh");
        assert_eq!(
            discoveries(&idp).await,
            2,
            "a stale enough entry is refetched"
        );

        idp.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&idp)
            .await;
        age(&cache, METADATA_MIN_REFRESH + Duration::from_secs(1)).await;
        assert!(
            cache.refresh(&issuer).await.is_err(),
            "a failed refresh is an error"
        );
        assert!(
            cache.get(&issuer).await.is_ok(),
            "the known-good document is kept after a failed refresh"
        );
    }

    fn req(enabled: bool, secret: Option<&str>) -> OidcSettingsUpdate {
        OidcSettingsUpdate {
            enabled,
            issuer: " https://login.example.test/tenant/v2.0 ".into(),
            client_id: " client ".into(),
            client_secret: secret.map(Into::into),
            button_label: String::new(),
        }
    }

    #[test]
    fn issuer_rules() {
        assert!(validate_issuer("https://login.example.test/x/v2.0", Mode::Prod).is_ok());
        assert!(validate_issuer("http://localhost:28080/default", Mode::Prod).is_err());
        assert!(validate_issuer("http://localhost:28080/default", Mode::Dev).is_ok());
        assert!(validate_issuer("ftp://example.test", Mode::Dev).is_err());
        assert!(validate_issuer("not a url", Mode::Dev).is_err());
        assert!(validate_issuer("https://example.test/?a=b", Mode::Prod).is_err());
    }

    #[test]
    fn merge_requires_complete_settings_when_enabled() {
        assert_eq!(
            merge(None, &req(true, None), None, Mode::Prod),
            Err("enter the client secret".to_owned())
        );
        let mut r = req(true, None);
        r.client_id = " ".into();
        assert_eq!(
            merge(None, &r, Some(vec![1]), Mode::Prod),
            Err("enter the client ID".to_owned())
        );
        let mut r = req(true, None);
        r.issuer = String::new();
        assert!(merge(None, &r, Some(vec![1]), Mode::Prod).is_err());
        let mut r = req(false, None);
        r.button_label = "x".repeat(65);
        assert!(merge(None, &r, None, Mode::Prod).is_err());
        let off = merge(None, &req(false, None), None, Mode::Prod).expect("off is fine");
        assert!(!off.enabled);
        assert_eq!(off.client_id, "client");
        assert_eq!(off.issuer, "https://login.example.test/tenant/v2.0");
    }

    #[test]
    fn merge_keeps_stored_secret_unless_replaced() {
        let stored = OidcProviderRecord {
            client_secret_enc: Some(vec![9]),
            ..Default::default()
        };
        let kept = merge(Some(stored.clone()), &req(true, None), None, Mode::Prod).expect("ok");
        assert_eq!(kept.client_secret_enc, Some(vec![9]));
        let replaced =
            merge(Some(stored), &req(true, None), Some(vec![7]), Mode::Prod).expect("ok");
        assert_eq!(replaced.client_secret_enc, Some(vec![7]));
    }

    #[test]
    fn record_needs_enabled_and_decryptable_secret() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let enc = crypto::encrypt(crypto::PURPOSE_OIDC_CLIENT_SECRET, &k1, b"s3cret").expect("enc");
        let mut rec = OidcProviderRecord {
            enabled: true,
            issuer: "https://login.example.test/v2.0".into(),
            client_id: "client".into(),
            client_secret_enc: Some(enc),
            button_label: " ".into(),
            updated_at: 1,
        };
        let cfg = from_record(&rec, &[&k2, &k1]).expect("active");
        assert_eq!(cfg.client_secret.expose(), "s3cret");
        assert_eq!(cfg.button_label, DEFAULT_BUTTON_LABEL);
        assert!(from_record(&rec, &[&k2]).is_none(), "wrong key");
        let totp = crypto::encrypt(crypto::PURPOSE_TOTP, &k1, b"s3cret").expect("enc");
        rec.client_secret_enc = Some(totp);
        assert!(from_record(&rec, &[&k1]).is_none(), "purpose is bound");
        rec.enabled = false;
        assert!(from_record(&rec, &[&k1]).is_none());
    }

    #[test]
    fn redirect_uri_from_public_url() {
        let u = Url::parse("https://dns.example.test/").expect("url");
        assert_eq!(
            redirect_uri(&u),
            "https://dns.example.test/auth/oidc/callback"
        );
    }
}
