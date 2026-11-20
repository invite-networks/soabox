//! SOAbox: a web interface and PowerDNS-compatible API gateway for
//! PowerDNS Authoritative Server 5.x.
//!
//! See `docs/spec/design.md` in the repository for the design.

pub mod api;
pub mod app;
pub mod audit;
pub mod auth;
pub mod authz;
pub mod cli;
pub mod config;
pub mod errors;
pub mod http;
pub mod keys;
pub mod openapi;
pub mod pdns;
pub mod ratelimit;
pub mod settings;
pub mod state;
pub mod store;
pub mod telemetry;
pub mod v1;
