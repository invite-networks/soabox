//! Local answers to `OPTIONS` requests under `/api`.
//!
//! `OPTIONS` carries no API key, so it is never forwarded with the PowerDNS
//! key. SOAbox answers it itself with the same status and headers PowerDNS 5
//! returns for each route, so clients cannot tell the difference.

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;

/// The methods PowerDNS allows on a route, or `None` when it has no such
/// route. `segments` are the path segments after `/api`.
pub fn allowed_methods(segments: &[&str], server_id: &str) -> Option<&'static str> {
    const GET: &str = "GET, OPTIONS";
    let rest = match segments {
        [] | ["v1"] | ["v1", "servers"] | ["docs"] => return Some(GET),
        ["v1", "servers", id, rest @ ..] if *id == server_id => rest,
        _ => return None,
    };
    Some(match rest {
        [] | ["config"] | ["search-data"] | ["statistics"] | ["views"] | ["networks"] => GET,
        ["cache", "flush"] => "PUT, OPTIONS",
        ["zones"] => "GET, POST, OPTIONS",
        ["zones", _] => "GET, PATCH, PUT, DELETE, OPTIONS",
        ["zones", _, "axfr-retrieve" | "notify" | "rectify"] => "PUT, OPTIONS",
        ["zones", _, "export"] => GET,
        ["zones", _, "cryptokeys" | "metadata"] => "GET, POST, OPTIONS",
        ["zones", _, "cryptokeys", _] => "GET, POST, PUT, DELETE, OPTIONS",
        ["zones", _, "metadata", _] | ["tsigkeys", _] => "GET, PUT, DELETE, OPTIONS",
        ["tsigkeys"] | ["autoprimaries"] => "GET, POST, OPTIONS",
        ["autoprimaries", _, _] | ["views", _, _] => "DELETE, OPTIONS",
        ["views", _] => "GET, POST, OPTIONS",
        ["networks", _, _] => "GET, PUT, OPTIONS",
        _ => return None,
    })
}

/// The `OPTIONS` response PowerDNS would send for this path.
pub fn response(segments: &[&str], server_id: &str) -> Response {
    let mut res = Response::new(Body::empty());
    let Some(methods) = allowed_methods(segments, server_id) else {
        *res.status_mut() = StatusCode::NOT_FOUND;
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        return res;
    };
    let headers = res.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, X-API-Key"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(methods),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("3600"),
    );
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn methods(path: &str) -> Option<&'static str> {
        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        allowed_methods(&segs, "localhost")
    }

    #[test]
    fn matches_powerdns_routes() {
        assert_eq!(methods(""), Some("GET, OPTIONS"));
        assert_eq!(
            methods("v1/servers/localhost/zones"),
            Some("GET, POST, OPTIONS")
        );
        assert_eq!(
            methods("v1/servers/localhost/zones/a.test."),
            Some("GET, PATCH, PUT, DELETE, OPTIONS")
        );
        assert_eq!(
            methods("v1/servers/localhost/zones/a.test./cryptokeys/3"),
            Some("GET, POST, PUT, DELETE, OPTIONS")
        );
        assert_eq!(
            methods("v1/servers/localhost/networks/192.0.2.0/24"),
            Some("GET, PUT, OPTIONS")
        );
        assert_eq!(
            methods("v1/servers/localhost/cache/flush"),
            Some("PUT, OPTIONS")
        );
    }

    #[test]
    fn unknown_routes_and_servers_are_not_found() {
        assert_eq!(methods("v1/servers/other"), None);
        assert_eq!(methods("v1/servers/localhost/config/x"), None);
        assert_eq!(methods("v1/servers/localhost/zones/a.test./rrsets"), None);
        assert_eq!(methods("v2"), None);
        let res = response(&["v2"], "localhost");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
