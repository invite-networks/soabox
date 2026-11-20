//! The connection to PowerDNS. Only this module talks to PowerDNS.

pub mod json;
pub mod model;
pub mod options;
pub mod zone_id;

use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, HeaderName, Method, StatusCode, header};
use serde::de::DeserializeOwned;
use url::Url;

use crate::config::Secret;
pub use zone_id::ZoneName;

/// Errors from typed PowerDNS calls.
#[derive(Debug, thiserror::Error)]
pub enum PdnsClientError {
    #[error("PowerDNS unreachable: {0}")]
    Unavailable(#[source] reqwest::Error),
    #[error("PowerDNS returned {status}: {message}")]
    Status { status: u16, message: String },
    #[error("could not decode PowerDNS response: {0}")]
    Decode(#[source] serde_json::Error),
}

/// Why a forwarded request could not be completed.
#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    /// The path would be rewritten by URL normalization.
    #[error("unsafe path")]
    UnsafePath,
    /// PowerDNS could not be reached or did not answer in time.
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
}

/// A raw upstream response, forwarded to the client as is.
#[derive(Debug, Clone)]
pub struct UpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// Request headers never forwarded upstream. The key is always replaced.
const DROP_REQUEST_HEADERS: &[HeaderName] = &[
    header::HOST,
    header::CONNECTION,
    header::CONTENT_LENGTH,
    header::TRANSFER_ENCODING,
    header::TE,
    header::UPGRADE,
    header::PROXY_AUTHORIZATION,
    header::AUTHORIZATION,
    header::COOKIE,
    header::ACCEPT_ENCODING,
];

/// Response headers never copied back to the client.
pub const DROP_RESPONSE_HEADERS: &[HeaderName] = &[
    header::CONNECTION,
    header::CONTENT_LENGTH,
    header::TRANSFER_ENCODING,
    header::TE,
    header::UPGRADE,
    header::TRAILER,
];

fn is_dropped_request_header(name: &HeaderName) -> bool {
    DROP_REQUEST_HEADERS.contains(name)
        || name.as_str().eq_ignore_ascii_case("x-api-key")
        || name.as_str().starts_with("x-forwarded-")
        || name.as_str() == "forwarded"
        || name.as_str() == "keep-alive"
        || name.as_str() == "proxy-connection"
        || name.as_str() == "x-real-ip"
        || name.as_str() == "x-request-id"
}

/// Client for the PowerDNS HTTP API using SOAbox's own key.
#[derive(Debug, Clone)]
pub struct PdnsClient {
    http: reqwest::Client,
    base: Url,
    key: Secret,
    server_id: String,
}

impl PdnsClient {
    /// Builds a client. `base` is the PowerDNS web server URL without `/api`.
    pub fn new(
        base: Url,
        key: Secret,
        server_id: String,
        timeout: Duration,
    ) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            base,
            key,
            server_id,
        })
    }

    /// The PowerDNS server id SOAbox serves.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Builds the upstream URL. Returns `None` when URL normalization would
    /// change the path (dot segments, backslashes, encoded dots), so a request
    /// can never reach a different PowerDNS resource than the one authorized.
    fn url(&self, path_and_query: &str) -> Option<Url> {
        let mut u = self.base.clone();
        let (path, query) = match path_and_query.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (path_and_query, None),
        };
        let base_path = u.path().trim_end_matches('/').to_owned();
        let wanted = format!("{base_path}{path}");
        u.set_path(&wanted);
        if u.path() != wanted {
            tracing::warn!(path = %path, "refusing a PowerDNS path that normalizes differently");
            return None;
        }
        u.set_query(query);
        Some(u)
    }

    /// Forwards a request unchanged except for the key and hop-by-hop headers.
    /// `path_and_query` starts with `/api`.
    pub async fn forward(
        &self,
        method: Method,
        path_and_query: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<UpstreamResponse, ForwardError> {
        let url = self.url(path_and_query).ok_or(ForwardError::UnsafePath)?;
        let mut out = HeaderMap::new();
        for (name, value) in headers {
            if !is_dropped_request_header(name) {
                out.append(name.clone(), value.clone());
            }
        }
        let resp = self
            .http
            .request(method, url)
            .headers(out)
            .header("X-API-Key", self.key.expose())
            .body(body)
            .send()
            .await?;
        let status = resp.status();
        let mut headers = resp.headers().clone();
        for h in DROP_RESPONSE_HEADERS {
            headers.remove(h);
        }
        let body = resp.bytes().await?;
        Ok(UpstreamResponse {
            status,
            headers,
            body,
        })
    }

    async fn call(
        &self,
        method: Method,
        path_and_query: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Bytes, PdnsClientError> {
        let Some(url) = self.url(path_and_query) else {
            return Err(PdnsClientError::Status {
                status: 404,
                message: "Not Found".into(),
            });
        };
        let mut req = self
            .http
            .request(method, url)
            .header("X-API-Key", self.key.expose());
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await.map_err(PdnsClientError::Unavailable)?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(PdnsClientError::Unavailable)?;
        if !status.is_success() {
            let message = serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .unwrap_or_else(|| status.to_string());
            return Err(PdnsClientError::Status {
                status: status.as_u16(),
                message,
            });
        }
        Ok(bytes)
    }

    /// GET and decode JSON. `path` is relative to the server, for example `/zones`.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, PdnsClientError> {
        let bytes = self
            .call(Method::GET, &self.server_path(path), None)
            .await?;
        serde_json::from_slice(&bytes).map_err(PdnsClientError::Decode)
    }

    /// Sends a write with a JSON body and decodes any JSON response.
    pub async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, PdnsClientError> {
        let bytes = self.call(method, &self.server_path(path), body).await?;
        if bytes.is_empty() {
            return Ok(None);
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(PdnsClientError::Decode)
    }

    /// `/api/v1/servers/{id}{path}`.
    pub fn server_path(&self, path: &str) -> String {
        format!("/api/v1/servers/{}{}", self.server_id, path)
    }

    /// Lists zones without records.
    pub async fn list_zones(&self) -> Result<Vec<model::ZoneSummary>, PdnsClientError> {
        self.get("/zones").await
    }

    /// Gets a zone. With `rrsets` false only zone-level fields are returned.
    pub async fn get_zone(
        &self,
        zone: &ZoneName,
        rrsets: bool,
    ) -> Result<model::Zone, PdnsClientError> {
        let q = if rrsets { "" } else { "?rrsets=false" };
        self.get(&format!("/zones/{}{q}", zone.to_url_id())).await
    }

    /// Gets one RRset of a zone. Returns an empty list if it does not exist.
    pub async fn get_rrset(
        &self,
        zone: &ZoneName,
        name: &str,
        rtype: &str,
    ) -> Result<Vec<model::RrSet>, PdnsClientError> {
        let qs = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("rrset_name", name)
            .append_pair("rrset_type", rtype)
            .finish();
        let z: model::Zone = self
            .get(&format!("/zones/{}?{qs}", zone.to_url_id()))
            .await?;
        Ok(z.rrsets)
    }

    /// Readiness probe: the server object answers with our key.
    pub async fn ping(&self) -> Result<(), PdnsClientError> {
        self.call(Method::GET, &self.server_path(""), None)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> PdnsClient {
        PdnsClient::new(
            Url::parse(&server.uri()).expect("url"),
            Secret::new("real-key"),
            "localhost".into(),
            Duration::from_millis(500),
        )
        .expect("client")
    }

    #[tokio::test]
    async fn forward_replaces_key_and_strips_hop_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/servers"))
            .and(header("x-api-key", "real-key"))
            .and(query_param("x", "1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .insert_header("x-frame-options", "deny")
                    .set_body_string("[]"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mut h = HeaderMap::new();
        h.insert("x-api-key", "client-key".parse().expect("hv"));
        h.insert("x-forwarded-for", "203.0.113.9".parse().expect("hv"));
        h.insert("accept", "application/json".parse().expect("hv"));
        let resp = client(&server)
            .forward(Method::GET, "/api/v1/servers?x=1", &h, Bytes::new())
            .await
            .expect("forward");
        assert_eq!(resp.status, 200);
        assert_eq!(resp.headers.get("x-frame-options").expect("h"), "deny");
        assert!(resp.headers.get("content-length").is_none());
        let reqs = server.received_requests().await.expect("reqs");
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].headers.get_all("x-api-key").iter().count(), 1);
        assert!(reqs[0].headers.get("x-forwarded-for").is_none());
    }

    #[tokio::test]
    async fn typed_call_maps_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/servers/localhost/zones/nope.test."))
            .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"error": "Not Found"}"#))
            .mount(&server)
            .await;
        let z = ZoneName::from_name("nope.test").expect("zone");
        match client(&server).get_zone(&z, true).await {
            Err(PdnsClientError::Status { status, message }) => {
                assert_eq!(status, 404);
                assert_eq!(message, "Not Found");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_is_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).ping().await,
            Err(PdnsClientError::Unavailable(_))
        ));
    }

    #[test]
    fn url_keeps_base_path() {
        let c = PdnsClient::new(
            Url::parse("http://pdns:8081/prefix/").expect("url"),
            Secret::new("k"),
            "localhost".into(),
            Duration::from_secs(1),
        )
        .expect("client");
        assert_eq!(
            c.url("/api/v1/servers?a=b").expect("url").as_str(),
            "http://pdns:8081/prefix/api/v1/servers?a=b"
        );
        for bad in [
            "/api/v1/servers/localhost/zones/a.test./../../tsigkeys",
            "/api/v1/servers/localhost/zones/a.test./%2e%2e/%2E%2E/tsigkeys",
            "/api/v1/servers/localhost/zones/a.test.\\..\\..\\tsigkeys",
            "/api/v1/servers/localhost/zones/.",
        ] {
            assert!(c.url(bad).is_none(), "{bad}");
        }
    }
}
