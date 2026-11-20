//! HTTP plumbing: client address resolution, request ids, access logging,
//! security headers, and health endpoints.

use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;
use serde_json::json;

use crate::state::AppState;

/// Who sent the request, after applying trusted proxy rules.
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub ip: IpAddr,
    pub request_id: String,
    /// True when the original request used HTTPS according to a trusted proxy.
    pub https: bool,
}

/// Resolves the client address. `X-Forwarded-For` is honored only when the
/// direct peer is a trusted proxy, and is walked right to left skipping
/// further trusted proxies.
pub fn resolve_client_ip(peer: IpAddr, headers: &HeaderMap, trusted: &[IpNet]) -> IpAddr {
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|n| n.contains(ip));
    if !is_trusted(&peer) {
        return peer;
    }
    let chain: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .collect();
    let mut client = peer;
    for raw in chain.iter().rev() {
        let Ok(ip) = raw.parse::<IpAddr>() else {
            break;
        };
        client = ip;
        if !is_trusted(&ip) {
            break;
        }
    }
    client
}

fn new_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn valid_request_id(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Middleware: attaches [`ClientInfo`], echoes `X-Request-Id`, and logs one
/// line per request.
pub async fn client_info(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let started = Instant::now();
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));
    let trusted_peer = state
        .config
        .trusted_proxies
        .iter()
        .any(|n| n.contains(&peer));
    let ip = resolve_client_ip(peer, req.headers(), &state.config.trusted_proxies);
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| trusted_peer && valid_request_id(v))
        .map_or_else(new_request_id, str::to_owned);
    let https = trusted_peer
        && req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.eq_ignore_ascii_case("https"));
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    req.extensions_mut().insert(ClientInfo {
        ip,
        request_id: request_id.clone(),
        https,
    });

    let mut resp = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(&request_id) {
        resp.headers_mut().insert("x-request-id", v);
    }
    tracing::info!(
        request_id = %request_id,
        client_ip = %ip,
        method = %method,
        path = %path,
        status = resp.status().as_u16(),
        latency_ms = started.elapsed().as_millis() as u64,
        "request"
    );
    resp
}

/// Middleware: security headers for the UI and `/v1`. The `/api`
/// passthrough mirrors PowerDNS headers instead.
pub async fn security_headers(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let is_api = req.uri().path() == "/api" || req.uri().path().starts_with("/api/");
    let mut resp = next.run(req).await;
    if is_api {
        return resp;
    }
    let h = resp.headers_mut();
    let set = |h: &mut HeaderMap, name: &'static str, value: &'static str| {
        h.entry(name).or_insert(HeaderValue::from_static(value));
    };
    set(h, "x-content-type-options", "nosniff");
    set(h, "x-frame-options", "DENY");
    set(h, "referrer-policy", "no-referrer");
    set(h, "cross-origin-opener-policy", "same-origin");
    set(
        h,
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()",
    );
    set(
        h,
        "content-security-policy",
        "default-src 'self'; img-src 'self' data:; style-src 'self'; \
         style-src-attr 'unsafe-inline'; script-src 'self'; connect-src 'self'; font-src 'self'; \
         frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
    );
    if state.config.hsts {
        set(
            h,
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        );
    }
    resp
}

/// `GET /healthz`: the process is running.
pub async fn healthz() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

/// `GET /readyz`: the database and PowerDNS both answer.
pub async fn readyz(State(state): State<AppState>) -> Response {
    let db = state.store.ping().await;
    let pdns = state.pdns.ping().await;
    if let Err(e) = &db {
        tracing::warn!(error = %e, "readiness: database failed");
    }
    if let Err(e) = &pdns {
        tracing::warn!(error = %e, "readiness: PowerDNS failed");
    }
    let body = json!({
        "status": if db.is_ok() && pdns.is_ok() { "ok" } else { "unavailable" },
        "database": if db.is_ok() { "ok" } else { "failed" },
        "pdns": if pdns.is_ok() { "ok" } else { "failed" },
    });
    let status = if db.is_ok() && pdns.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body)).into_response()
}

/// `GET /version`.
pub async fn version() -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "revision": option_env!("SOABOX_GIT_SHA").unwrap_or("unknown"),
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(list: &[&str]) -> Vec<IpNet> {
        list.iter().map(|s| s.parse().expect("net")).collect()
    }

    fn xff(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", v.parse().expect("hv"));
        h
    }

    #[test]
    fn untrusted_peer_cannot_spoof() {
        let peer: IpAddr = "203.0.113.5".parse().expect("ip");
        let ip = resolve_client_ip(peer, &xff("1.2.3.4"), &nets(&["10.0.0.0/8"]));
        assert_eq!(ip, peer);
    }

    #[test]
    fn trusted_peer_uses_rightmost_untrusted() {
        let peer: IpAddr = "10.0.0.2".parse().expect("ip");
        let ip = resolve_client_ip(
            peer,
            &xff("6.6.6.6, 198.51.100.7, 10.0.0.9"),
            &nets(&["10.0.0.0/8"]),
        );
        assert_eq!(ip, "198.51.100.7".parse::<IpAddr>().expect("ip"));
    }

    #[test]
    fn trusted_peer_without_header_is_client() {
        let peer: IpAddr = "10.0.0.2".parse().expect("ip");
        assert_eq!(
            resolve_client_ip(peer, &HeaderMap::new(), &nets(&["10.0.0.0/8"])),
            peer
        );
    }

    #[test]
    fn garbage_in_chain_stops_parsing() {
        let peer: IpAddr = "10.0.0.2".parse().expect("ip");
        let ip = resolve_client_ip(peer, &xff("not-an-ip"), &nets(&["10.0.0.0/8"]));
        assert_eq!(ip, peer);
    }

    #[test]
    fn request_id_validation() {
        assert!(valid_request_id("abc-123_x.y"));
        assert!(!valid_request_id(""));
        assert!(!valid_request_id("a b"));
        assert!(!valid_request_id(&"a".repeat(200)));
    }
}
