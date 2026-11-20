//! Logging setup.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::config::{Config, Mode};

/// Installs the global subscriber: JSON in `prod`, readable text in `dev`.
/// The server logs to stdout. CLI commands log to stderr so their output
/// stays clean.
pub fn init(config: &Config, to_stderr: bool) {
    let filter = EnvFilter::try_new(format!(
        "{level},sqlx=warn,hyper=warn,reqwest=warn,tower_http=warn",
        level = config.log_level
    ))
    .unwrap_or_else(|_| EnvFilter::new("info"));
    let registry = tracing_subscriber::registry().with(filter);
    let result = match config.mode {
        Mode::Prod if to_stderr => registry
            .with(
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_writer(std::io::stderr),
            )
            .try_init(),
        Mode::Prod => registry
            .with(
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(false)
                    .with_span_list(false),
            )
            .try_init(),
        Mode::Dev if to_stderr => registry
            .with(fmt::layer().compact().with_writer(std::io::stderr))
            .try_init(),
        Mode::Dev => registry.with(fmt::layer().compact()).try_init(),
    };
    if result.is_ok()
        && let Some(env) = &config.environment
    {
        tracing::info!(environment = %env, "logging initialized");
    }
}
