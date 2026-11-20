//! Runtime configuration.
//!
//! This is the only module that reads environment variables. Every variable
//! uses the `SOABOX_` prefix. Secret values may instead be supplied as a file
//! path through the same name with a `_FILE` suffix.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use ipnet::IpNet;
use url::Url;

/// Runtime mode. Code branches on this value only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Developer workstation: readable logs, debug level, API docs exposed.
    Dev,
    /// Every deployed environment: JSON logs, info level, no debug endpoints.
    Prod,
}

/// A value that must never be printed.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a secret value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the secret. Call only where the value is actually used.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// OpenID Connect provider settings.
#[derive(Debug, Clone)]
pub struct OidcConfig {
    /// Issuer URL, used for discovery and token validation.
    pub issuer: Url,
    /// Client id registered with the provider.
    pub client_id: String,
    /// Client secret registered with the provider.
    pub client_secret: Secret,
    /// Text shown on the sign-in button.
    pub button_label: String,
}

/// Bootstrap Admin account created at startup when no Admin exists.
#[derive(Debug, Clone)]
pub struct BootstrapAdmin {
    /// Username of the Admin to create.
    pub username: String,
    /// Initial password.
    pub password: Secret,
}

/// Complete service configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    pub environment: Option<String>,
    pub bind: SocketAddr,
    pub public_url: Url,
    pub database_url: Secret,
    pub pdns_url: Url,
    pub pdns_api_key: Secret,
    pub pdns_server_id: String,
    pub pdns_timeout: Duration,
    pub trusted_proxies: Vec<IpNet>,
    pub secret_key: [u8; 32],
    pub secret_key_previous: Option<[u8; 32]>,
    pub oidc: Option<OidcConfig>,
    pub session_idle: Duration,
    pub session_max: Duration,
    pub bootstrap_admin: Option<BootstrapAdmin>,
    pub static_dir: PathBuf,
    pub max_body_bytes: usize,
    pub log_level: String,
    pub hsts: bool,
}

/// A configuration problem, naming the variable at fault.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} is required")]
    Missing(&'static str),
    #[error("{name} is invalid: {reason}")]
    Invalid { name: &'static str, reason: String },
    #[error("{name}_FILE could not be read: {source}")]
    File {
        name: &'static str,
        source: std::io::Error,
    },
}

impl Config {
    /// Loads configuration from the process environment. In `dev` setups a
    /// `.env` file in the working directory or its parent is loaded first.
    pub fn from_env() -> Result<Self, ConfigError> {
        if std::env::var("SOABOX_MODE").as_deref() != Ok("prod") {
            let _ = dotenvy::dotenv();
        }
        let vars: HashMap<String, String> = std::env::vars().collect();
        Self::from_map(&vars)
    }

    /// Builds configuration from a map of variables. Used by tests.
    pub fn from_map(vars: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let r = Reader { vars };

        let mode = match r.required("SOABOX_MODE")?.as_str() {
            "dev" => Mode::Dev,
            "prod" => Mode::Prod,
            other => {
                return Err(ConfigError::Invalid {
                    name: "SOABOX_MODE",
                    reason: format!("expected dev or prod, got {other:?}"),
                });
            }
        };

        let oidc = match r.optional("SOABOX_OIDC_ISSUER") {
            None => None,
            Some(issuer) => Some(OidcConfig {
                issuer: parse_url("SOABOX_OIDC_ISSUER", &issuer)?,
                client_id: r.required("SOABOX_OIDC_CLIENT_ID")?,
                client_secret: Secret::new(r.secret_required("SOABOX_OIDC_CLIENT_SECRET")?),
                button_label: r
                    .optional("SOABOX_OIDC_BUTTON_LABEL")
                    .unwrap_or_else(|| crate::auth::provider::DEFAULT_BUTTON_LABEL.to_owned()),
            }),
        };

        let bootstrap_admin = match r.optional("SOABOX_BOOTSTRAP_ADMIN_USERNAME") {
            None => None,
            Some(username) => Some(BootstrapAdmin {
                username,
                password: Secret::new(r.secret_required("SOABOX_BOOTSTRAP_ADMIN_PASSWORD")?),
            }),
        };

        let trusted_proxies = match r.optional("SOABOX_TRUSTED_PROXIES") {
            None => Vec::new(),
            Some(list) => list
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(parse_net)
                .collect::<Result<_, _>>()?,
        };

        let pdns_server_id = r
            .optional("SOABOX_PDNS_SERVER_ID")
            .unwrap_or_else(|| "localhost".to_owned());

        let public_url = parse_url("SOABOX_PUBLIC_URL", &r.required("SOABOX_PUBLIC_URL")?)?;
        if mode == Mode::Prod && public_url.scheme() != "https" {
            return Err(ConfigError::Invalid {
                name: "SOABOX_PUBLIC_URL",
                reason: "must use https in prod mode so cookies are Secure".to_owned(),
            });
        }

        let default_level = match mode {
            Mode::Dev => "debug",
            Mode::Prod => "info",
        };

        Ok(Self {
            mode,
            environment: r.optional("SOABOX_ENVIRONMENT"),
            bind: r.parsed("SOABOX_BIND", "0.0.0.0:8080")?,
            public_url,
            database_url: Secret::new(
                r.secret_optional("SOABOX_DATABASE_URL")?
                    .unwrap_or_else(|| "sqlite://data/soabox.db".to_owned()),
            ),
            pdns_url: parse_url("SOABOX_PDNS_URL", &r.required("SOABOX_PDNS_URL")?)?,
            pdns_api_key: Secret::new(r.secret_required("SOABOX_PDNS_API_KEY")?),
            pdns_server_id,
            pdns_timeout: Duration::from_secs(r.parsed("SOABOX_PDNS_TIMEOUT_SECS", "30")?),
            trusted_proxies,
            secret_key: parse_key(
                "SOABOX_SECRET_KEY",
                &r.secret_required("SOABOX_SECRET_KEY")?,
            )?,
            secret_key_previous: r
                .secret_optional("SOABOX_SECRET_KEY_PREVIOUS")?
                .map(|v| parse_key("SOABOX_SECRET_KEY_PREVIOUS", &v))
                .transpose()?,
            oidc,
            session_idle: Duration::from_secs(
                60 * r.parsed::<u64>("SOABOX_SESSION_IDLE_MINUTES", "60")?,
            ),
            session_max: Duration::from_secs(
                3600 * r.parsed::<u64>("SOABOX_SESSION_MAX_HOURS", "12")?,
            ),
            bootstrap_admin,
            static_dir: PathBuf::from(
                r.optional("SOABOX_STATIC_DIR")
                    .unwrap_or_else(|| "frontend/dist".to_owned()),
            ),
            max_body_bytes: r.parsed("SOABOX_MAX_BODY_BYTES", "10485760")?,
            log_level: r
                .optional("SOABOX_LOG_LEVEL")
                .unwrap_or_else(|| default_level.to_owned()),
            hsts: r.parsed("SOABOX_HSTS", "false")?,
        })
    }

    /// True when OIDC is set by `SOABOX_OIDC_*` variables. These take
    /// precedence over settings saved in the UI.
    pub fn oidc_from_env(&self) -> bool {
        self.oidc.is_some()
    }
}

struct Reader<'a> {
    vars: &'a HashMap<String, String>,
}

impl Reader<'_> {
    fn optional(&self, name: &str) -> Option<String> {
        self.vars
            .get(name)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }

    fn required(&self, name: &'static str) -> Result<String, ConfigError> {
        self.optional(name).ok_or(ConfigError::Missing(name))
    }

    fn secret_optional(&self, name: &'static str) -> Result<Option<String>, ConfigError> {
        if let Some(v) = self.optional(name) {
            return Ok(Some(v));
        }
        let file_var = format!("{name}_FILE");
        match self.optional(&file_var) {
            None => Ok(None),
            Some(path) => std::fs::read_to_string(&path)
                .map(|s| Some(s.trim().to_owned()))
                .map_err(|source| ConfigError::File { name, source }),
        }
    }

    fn secret_required(&self, name: &'static str) -> Result<String, ConfigError> {
        self.secret_optional(name)?
            .filter(|v| !v.is_empty())
            .ok_or(ConfigError::Missing(name))
    }

    fn parsed<T>(&self, name: &'static str, default: &str) -> Result<T, ConfigError>
    where
        T: std::str::FromStr,
        T::Err: fmt::Display,
    {
        let raw = self.optional(name).unwrap_or_else(|| default.to_owned());
        raw.parse().map_err(|e: T::Err| ConfigError::Invalid {
            name,
            reason: e.to_string(),
        })
    }
}

fn parse_url(name: &'static str, raw: &str) -> Result<Url, ConfigError> {
    Url::parse(raw).map_err(|e| ConfigError::Invalid {
        name,
        reason: e.to_string(),
    })
}

fn parse_net(raw: &str) -> Result<IpNet, ConfigError> {
    raw.parse::<IpNet>()
        .or_else(|_| raw.parse::<std::net::IpAddr>().map(IpNet::from))
        .map_err(|e| ConfigError::Invalid {
            name: "SOABOX_TRUSTED_PROXIES",
            reason: format!("{raw:?}: {e}"),
        })
}

fn parse_key(name: &'static str, raw: &str) -> Result<[u8; 32], ConfigError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|e| ConfigError::Invalid {
            name,
            reason: format!("not base64: {e}"),
        })?;
    let key: [u8; 32] = bytes.try_into().map_err(|_| ConfigError::Invalid {
        name,
        reason: "must decode to exactly 32 bytes (openssl rand -base64 32)".to_owned(),
    })?;
    let distinct: std::collections::BTreeSet<u8> = key.iter().copied().collect();
    if distinct.len() < 8 {
        return Err(ConfigError::Invalid {
            name,
            reason: "is not random enough, generate one with openssl rand -base64 32".to_owned(),
        });
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn base_vars() -> HashMap<String, String> {
        [
            ("SOABOX_MODE", "prod"),
            ("SOABOX_PUBLIC_URL", "https://dns.example.test"),
            ("SOABOX_PDNS_URL", "http://pdns:8081"),
            ("SOABOX_PDNS_API_KEY", "upstream"),
            (
                "SOABOX_SECRET_KEY",
                "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    #[test]
    fn loads_minimal_config_with_defaults() {
        let c = Config::from_map(&base_vars()).expect("valid");
        assert_eq!(c.mode, Mode::Prod);
        assert_eq!(c.pdns_server_id, "localhost");
        assert_eq!(c.database_url.expose(), "sqlite://data/soabox.db");
        assert_eq!(c.log_level, "info");
        assert!(c.oidc.is_none());
        assert_eq!(c.session_idle, Duration::from_secs(3600));
    }

    #[test]
    fn mode_is_required_and_validated() {
        let mut v = base_vars();
        v.remove("SOABOX_MODE");
        assert!(matches!(
            Config::from_map(&v),
            Err(ConfigError::Missing("SOABOX_MODE"))
        ));
        v.insert("SOABOX_MODE".into(), "staging".into());
        assert!(matches!(
            Config::from_map(&v),
            Err(ConfigError::Invalid {
                name: "SOABOX_MODE",
                ..
            })
        ));
    }

    #[test]
    fn secret_can_come_from_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("key");
        std::fs::write(&path, "from-file\n").expect("write");
        let mut v = base_vars();
        v.remove("SOABOX_PDNS_API_KEY");
        v.insert(
            "SOABOX_PDNS_API_KEY_FILE".into(),
            path.to_string_lossy().into_owned(),
        );
        let c = Config::from_map(&v).expect("valid");
        assert_eq!(c.pdns_api_key.expose(), "from-file");
    }

    #[test]
    fn secret_key_must_be_32_bytes() {
        let mut v = base_vars();
        v.insert("SOABOX_SECRET_KEY".into(), "c2hvcnQ=".into());
        assert!(matches!(
            Config::from_map(&v),
            Err(ConfigError::Invalid {
                name: "SOABOX_SECRET_KEY",
                ..
            })
        ));
    }

    #[test]
    fn trusted_proxies_accept_cidrs_and_addresses() {
        let mut v = base_vars();
        v.insert(
            "SOABOX_TRUSTED_PROXIES".into(),
            "10.0.0.0/8, 192.0.2.7".into(),
        );
        let c = Config::from_map(&v).expect("valid");
        assert_eq!(c.trusted_proxies.len(), 2);
        v.insert("SOABOX_TRUSTED_PROXIES".into(), "nonsense".into());
        assert!(Config::from_map(&v).is_err());
    }

    #[test]
    fn oidc_requires_client_settings() {
        let mut v = base_vars();
        v.insert(
            "SOABOX_OIDC_ISSUER".into(),
            "https://login.example.test/v2.0".into(),
        );
        assert!(matches!(
            Config::from_map(&v),
            Err(ConfigError::Missing("SOABOX_OIDC_CLIENT_ID"))
        ));
        v.insert("SOABOX_OIDC_CLIENT_ID".into(), "client".into());
        v.insert("SOABOX_OIDC_CLIENT_SECRET".into(), "secret".into());
        let c = Config::from_map(&v).expect("valid");
        assert!(c.oidc_from_env());
        assert_eq!(
            format!("{:?}", c.oidc.expect("oidc").client_secret),
            "[redacted]"
        );
    }

    #[test]
    fn prod_requires_https_and_random_key() {
        let mut v = base_vars();
        v.insert("SOABOX_PUBLIC_URL".into(), "http://dns.example.test".into());
        assert!(Config::from_map(&v).is_err());
        let mut v = base_vars();
        v.insert(
            "SOABOX_SECRET_KEY".into(),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
        );
        assert!(matches!(
            Config::from_map(&v),
            Err(ConfigError::Invalid {
                name: "SOABOX_SECRET_KEY",
                ..
            })
        ));
    }

    #[test]
    fn dev_mode_defaults_to_debug() {
        let mut v = base_vars();
        v.insert("SOABOX_MODE".into(), "dev".into());
        assert_eq!(Config::from_map(&v).expect("valid").log_level, "debug");
    }
}
