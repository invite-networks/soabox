//! Command line interface.

use std::net::SocketAddr;

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::keys;
use crate::state::AppState;
use crate::store::{HistoryFilter, NewApiKey, Role, Store};

/// SOAbox: web interface and PowerDNS-compatible API gateway.
#[derive(Debug, Parser)]
#[command(name = "soabox", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the web server.
    Serve,
    /// Check configuration, database, and PowerDNS connectivity.
    Check,
    /// Write a consistent copy of the SQLite database.
    Backup {
        /// Destination file.
        dest: String,
    },
    /// Manage API keys.
    #[command(subcommand)]
    Apikey(ApikeyCommand),
    /// Manage accounts.
    #[command(subcommand)]
    Account(AccountCommand),
    /// Create a local Admin and print a one-time password.
    CreateAdmin {
        #[arg(long)]
        username: String,
    },
    /// Re-encrypt stored secrets with the current SOABOX_SECRET_KEY.
    RotateSecrets,
    /// Print the OpenAPI document for /auth and /v1.
    Openapi,
    /// Show change history.
    History {
        /// Only this zone.
        #[arg(long)]
        zone: Option<String>,
        /// Maximum rows.
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
}

#[derive(Debug, Subcommand)]
pub enum ApikeyCommand {
    /// Create a key and print its secret once.
    Create {
        /// admin, operator, or scoped.
        #[arg(long, default_value = "scoped")]
        role: String,
        #[arg(long, default_value = "")]
        description: String,
        /// Zone in scope (repeatable).
        #[arg(long = "zone")]
        zones: Vec<String>,
        /// Account name in scope (repeatable).
        #[arg(long = "account")]
        accounts: Vec<String>,
        /// Account that zones created by this key are assigned to.
        #[arg(long)]
        create_in: Option<String>,
        /// Allowed client network (repeatable).
        #[arg(long = "network")]
        networks: Vec<String>,
        /// Expiry as Unix seconds.
        #[arg(long)]
        expires_at: Option<i64>,
    },
    /// List keys.
    List,
    /// Disable a key.
    Disable { id: i64 },
    /// Enable a key.
    Enable { id: i64 },
    /// Delete a key.
    Delete { id: i64 },
}

#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    /// Create an account.
    Create {
        name: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    /// List accounts and their zones.
    List,
    /// Assign a zone to an account.
    AddZone { account: String, zone: String },
    /// Remove a zone from its account.
    RemoveZone { zone: String },
    /// Delete an account. Its zones stay in PowerDNS.
    Delete { name: String },
}

/// Runs the parsed command.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    if matches!(cli.command, Command::Openapi) {
        println!("{}", crate::openapi::json());
        return Ok(());
    }
    let config = Config::from_env().context("configuration")?;
    crate::telemetry::init(&config, !matches!(cli.command, Command::Serve));
    match cli.command {
        Command::Serve => serve(config).await,
        Command::Check => check(config).await,
        Command::Backup { dest } => {
            let store = Store::connect(config.database_url.expose()).await?;
            store.backup_sqlite(&dest).await?;
            println!("backup written to {dest}");
            Ok(())
        }
        Command::Openapi => Ok(()),
        Command::CreateAdmin { username } => {
            let store = Store::connect(config.database_url.expose()).await?;
            let password = create_admin(&store, &username).await?;
            println!("username: {username}");
            println!("password: {password}");
            println!("Sign in and change this password now.");
            Ok(())
        }
        Command::RotateSecrets => {
            let state = AppState::new(config).await?;
            let n = rotate_secrets(&state).await?;
            println!("re-encrypted {n} secrets");
            Ok(())
        }
        Command::Apikey(c) => {
            let store = Store::connect(config.database_url.expose()).await?;
            apikey(&store, c).await
        }
        Command::Account(c) => {
            let store = Store::connect(config.database_url.expose()).await?;
            account(&store, c).await
        }
        Command::History { zone, limit } => {
            let store = Store::connect(config.database_url.expose()).await?;
            let rows = store
                .list_history(&HistoryFilter {
                    zone: zone
                        .and_then(|z| crate::pdns::ZoneName::from_name(&z))
                        .map(|z| z.to_string()),
                    limit,
                    ..Default::default()
                })
                .await?;
            for r in rows {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    r.id,
                    r.at,
                    r.actor_name,
                    r.operation,
                    r.zone_name.unwrap_or_default(),
                    r.outcome
                );
            }
            Ok(())
        }
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let bind = config.bind;
    let state = AppState::new(config).await?;
    crate::auth::bootstrap_admin(&state).await?;
    if state.config.mode == crate::config::Mode::Prod && state.config.trusted_proxies.is_empty() {
        tracing::warn!(
            "SOABOX_TRUSTED_PROXIES is empty: behind a reverse proxy every client shares the \
             proxy address, so rate limits and audit client addresses apply to all clients together"
        );
    }
    spawn_session_cleanup(state.clone());
    let app = crate::app::router(state);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!(%bind, version = env!("CARGO_PKG_VERSION"), "listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

fn spawn_session_cleanup(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(600));
        loop {
            tick.tick().await;
            let idle = i64::try_from(state.config.session_idle.as_secs()).unwrap_or(i64::MAX);
            let cutoff = crate::store::now().saturating_sub(idle);
            match state.store.purge_sessions(cutoff).await {
                Ok(n) if n > 0 => tracing::info!(removed = n, "expired sessions removed"),
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "session cleanup failed"),
            }
        }
    });
}

/// Creates a local Admin with a random password, which is returned.
pub async fn create_admin(store: &Store, username: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        store.find_user_by_username(username).await?.is_none(),
        "user {username} already exists"
    );
    let password = crate::auth::crypto::random_token()?;
    store
        .create_user(&crate::store::NewUser {
            username: username.to_owned(),
            display_name: username.to_owned(),
            email: String::new(),
            role: crate::store::UserRole::Admin,
            auth_source: "local",
            oidc_issuer: None,
            oidc_subject: None,
            password_hash: Some(crate::auth::crypto::hash_password(&password)?),
            status: crate::store::UserStatus::Active,
        })
        .await?;
    Ok(password)
}

/// Re-encrypts TOTP secrets with the current key. Returns how many.
pub async fn rotate_secrets(state: &AppState) -> anyhow::Result<usize> {
    let keys = state.secret_keys();
    let mut n = 0;
    for (id, enc) in state.store.users_with_totp().await? {
        let plain = crate::auth::crypto::decrypt(crate::auth::crypto::PURPOSE_TOTP, &keys, &enc)
            .with_context(|| format!("user {id}: secret cannot be decrypted with either key"))?;
        let fresh = crate::auth::crypto::encrypt(
            crate::auth::crypto::PURPOSE_TOTP,
            &state.config.secret_key,
            &plain,
        )?;
        let enabled = state
            .store
            .get_user(id)
            .await?
            .is_some_and(|u| u.totp_enabled);
        state.store.set_totp(id, Some(&fresh), enabled).await?;
        n += 1;
    }
    Ok(n)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    tracing::info!("shutting down");
}

async fn check(config: Config) -> anyhow::Result<()> {
    let state = AppState::new(config).await?;
    state.store.ping().await.context("database")?;
    println!("database: ok");
    state.pdns.ping().await.context("PowerDNS")?;
    println!("pdns: ok");
    Ok(())
}

async fn apikey(store: &Store, c: ApikeyCommand) -> anyhow::Result<()> {
    match c {
        ApikeyCommand::Create {
            role,
            description,
            zones,
            accounts,
            create_in,
            networks,
            expires_at,
        } => {
            let role: Role = role.parse().map_err(anyhow::Error::msg)?;
            let mut account_ids = Vec::new();
            for name in &accounts {
                let a = store
                    .find_account_by_name(name)
                    .await?
                    .with_context(|| format!("account {name} not found"))?;
                account_ids.push(a.id);
            }
            let create_account_id = match create_in {
                None => None,
                Some(name) => Some(
                    store
                        .find_account_by_name(&name)
                        .await?
                        .with_context(|| format!("account {name} not found"))?
                        .id,
                ),
            };
            let zones = zones
                .iter()
                .map(|z| {
                    crate::pdns::ZoneName::from_name(z)
                        .map(|z| z.to_string())
                        .with_context(|| format!("invalid zone {z}"))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            for n in &networks {
                anyhow::ensure!(
                    n.parse::<ipnet::IpNet>().is_ok() || n.parse::<std::net::IpAddr>().is_ok(),
                    "invalid network {n}"
                );
            }
            let g = keys::generate()?;
            let id = store
                .insert_api_key(&NewApiKey {
                    key_hash: g.hash,
                    hint: g.hint,
                    description,
                    role: Some(role),
                    create_account_id,
                    owner_user_id: None,
                    expires_at,
                    zones,
                    account_ids,
                    networks,
                })
                .await?;
            println!("id: {id}");
            println!("key: {}", g.secret);
            println!("Store this key now. It cannot be shown again.");
        }
        ApikeyCommand::List => {
            for k in store.list_api_keys(None).await? {
                println!(
                    "{}\t{}\t...{}\t{}\t{}\tzones={}\taccounts={:?}",
                    k.id,
                    k.role,
                    k.hint,
                    if k.disabled { "disabled" } else { "active" },
                    k.description,
                    k.zones.join(","),
                    k.account_ids
                );
            }
        }
        ApikeyCommand::Disable { id } => {
            anyhow::ensure!(
                store.set_api_key_disabled(id, true).await?,
                "key {id} not found"
            );
        }
        ApikeyCommand::Enable { id } => {
            anyhow::ensure!(
                store.set_api_key_disabled(id, false).await?,
                "key {id} not found"
            );
        }
        ApikeyCommand::Delete { id } => {
            anyhow::ensure!(store.delete_api_key(id).await?, "key {id} not found");
        }
    }
    Ok(())
}

async fn account(store: &Store, c: AccountCommand) -> anyhow::Result<()> {
    match c {
        AccountCommand::Create { name, description } => {
            let id = store.create_account(&name, &description).await?;
            println!("id: {id}");
        }
        AccountCommand::List => {
            for a in store.list_accounts().await? {
                println!("{}\t{}\t{}", a.id, a.name, a.zones.join(","));
            }
        }
        AccountCommand::AddZone { account, zone } => {
            let a = store
                .find_account_by_name(&account)
                .await?
                .with_context(|| format!("account {account} not found"))?;
            let z = crate::pdns::ZoneName::from_name(&zone).context("invalid zone")?;
            store.set_zone_account(z.as_str(), a.id).await?;
        }
        AccountCommand::RemoveZone { zone } => {
            let z = crate::pdns::ZoneName::from_name(&zone).context("invalid zone")?;
            store.clear_zone_account(z.as_str()).await?;
        }
        AccountCommand::Delete { name } => {
            let a = store
                .find_account_by_name(&name)
                .await?
                .with_context(|| format!("account {name} not found"))?;
            store.delete_account(a.id).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing;

    #[tokio::test]
    async fn account_and_key_commands() {
        let (s, _d) = testing::sqlite().await;
        account(
            &s,
            AccountCommand::Create {
                name: "acme".into(),
                description: String::new(),
            },
        )
        .await
        .expect("create account");
        account(
            &s,
            AccountCommand::AddZone {
                account: "acme".into(),
                zone: "Example.Test".into(),
            },
        )
        .await
        .expect("add zone");
        assert_eq!(
            s.find_account_by_name("acme")
                .await
                .expect("q")
                .expect("a")
                .zones,
            vec!["example.test."]
        );
        account(&s, AccountCommand::List).await.expect("list");

        apikey(
            &s,
            ApikeyCommand::Create {
                role: "scoped".into(),
                description: "ci".into(),
                zones: vec!["a.test".into()],
                accounts: vec!["acme".into()],
                create_in: Some("acme".into()),
                networks: vec!["10.0.0.0/8".into()],
                expires_at: None,
            },
        )
        .await
        .expect("create key");
        let keys = s.list_api_keys(None).await.expect("keys");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].zones, vec!["a.test."]);
        assert_eq!(keys[0].role, Role::Scoped);
        let id = keys[0].id;
        apikey(&s, ApikeyCommand::List).await.expect("list");
        apikey(&s, ApikeyCommand::Disable { id })
            .await
            .expect("disable");
        assert!(s.get_api_key(id).await.expect("q").expect("k").disabled);
        apikey(&s, ApikeyCommand::Enable { id })
            .await
            .expect("enable");
        apikey(&s, ApikeyCommand::Delete { id })
            .await
            .expect("delete");
        assert!(apikey(&s, ApikeyCommand::Delete { id }).await.is_err());

        account(
            &s,
            AccountCommand::RemoveZone {
                zone: "example.test.".into(),
            },
        )
        .await
        .expect("remove zone");
        account(
            &s,
            AccountCommand::Delete {
                name: "acme".into(),
            },
        )
        .await
        .expect("delete account");
        assert!(
            account(
                &s,
                AccountCommand::Delete {
                    name: "acme".into()
                }
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn invalid_key_options_are_rejected() {
        let (s, _d) = testing::sqlite().await;
        let base = || ApikeyCommand::Create {
            role: "scoped".into(),
            description: String::new(),
            zones: vec![],
            accounts: vec![],
            create_in: None,
            networks: vec![],
            expires_at: None,
        };
        let mut c = base();
        if let ApikeyCommand::Create { role, .. } = &mut c {
            *role = "root".into();
        }
        assert!(apikey(&s, c).await.is_err());
        let mut c = base();
        if let ApikeyCommand::Create { accounts, .. } = &mut c {
            accounts.push("missing".into());
        }
        assert!(apikey(&s, c).await.is_err());
        let mut c = base();
        if let ApikeyCommand::Create { networks, .. } = &mut c {
            networks.push("not-a-net".into());
        }
        assert!(apikey(&s, c).await.is_err());
    }

    #[test]
    fn parses_commands() {
        let cli =
            Cli::try_parse_from(["soabox", "apikey", "create", "--role", "admin"]).expect("parse");
        assert!(matches!(
            cli.command,
            Command::Apikey(ApikeyCommand::Create { .. })
        ));
        assert!(Cli::try_parse_from(["soabox", "nope"]).is_err());
    }
}
