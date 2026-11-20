//! Error types and their HTTP representations.
//!
//! Two response shapes exist. The `/api` passthrough answers in the PowerDNS
//! shape (`{"error": "..."}`) through [`PdnsError`]. Everything else answers
//! in the standard shape (`{"detail", "code", "status"}`) through [`ApiError`].

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::pdns::json::to_pdns_string;

/// An error returned on the `/api` passthrough, shaped exactly like PowerDNS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdnsError {
    /// Missing, unknown, expired, disabled, or out-of-network key, or an
    /// endpoint the key's role may not use.
    Unauthorized,
    /// A zone outside the key's scope or a server id that is not served.
    NotFound,
    /// A request body SOAbox needed to read was not valid JSON.
    BadRequest,
    /// Too many failed key attempts from this client.
    TooManyRequests,
    /// PowerDNS could not be reached or did not answer in time.
    BadGateway,
    /// SOAbox could not complete the request because of its own fault.
    Internal,
    /// A request SOAbox refuses on policy, such as a record type that is not
    /// allowed, answered like a PowerDNS validation error.
    Rejected(String),
}

impl PdnsError {
    fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::BadGateway => StatusCode::BAD_GATEWAY,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Rejected(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Rejected(m) => m,
            Self::Unauthorized => "Unauthorized",
            Self::NotFound => "Not Found",
            Self::BadRequest => "Bad Request",
            Self::TooManyRequests => "Too Many Requests",
            Self::BadGateway => "Bad Gateway",
            Self::Internal => "Internal Server Error",
        }
    }
}

impl IntoResponse for PdnsError {
    fn into_response(self) -> Response {
        let body = to_pdns_string(&serde_json::json!({ "error": self.message() }));
        let unauthorized = self == Self::Unauthorized;
        let mut resp = (self.status(), body).into_response();
        let headers = resp.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        if unauthorized {
            headers.insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("X-API-Key realm=\"PowerDNS\""),
            );
        }
        resp
    }
}

/// The standard error body for `/v1` and `/auth`.
#[derive(Debug, Serialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
pub struct ErrorBody {
    /// Human-readable message, safe to show to a user.
    pub detail: String,
    /// Stable machine-readable code.
    pub code: String,
    /// HTTP status code.
    pub status: u16,
}

/// An error returned by `/v1` and `/auth` handlers.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Validation(String),
    #[error("sign in required")]
    Unauthenticated,
    #[error("a one-time code is required")]
    TotpRequired,
    #[error("missing or invalid CSRF token")]
    CsrfFailed,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("too many attempts, try again later")]
    RateLimited,
    #[error("PowerDNS returned an error: {0}")]
    Upstream(String),
    #[error("PowerDNS is unavailable")]
    UpstreamUnavailable,
    #[error("internal error")]
    Internal(#[source] anyhow::Error),
}

impl ApiError {
    /// Wraps any error as an internal failure.
    pub fn internal(e: impl Into<anyhow::Error>) -> Self {
        Self::Internal(e.into())
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"),
            Self::Unauthenticated => (StatusCode::UNAUTHORIZED, "UNAUTHENTICATED"),
            Self::TotpRequired => (StatusCode::UNAUTHORIZED, "TOTP_REQUIRED"),
            Self::CsrfFailed => (StatusCode::FORBIDDEN, "CSRF_FAILED"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            Self::Upstream(_) => (StatusCode::UNPROCESSABLE_ENTITY, "UPSTREAM_ERROR"),
            Self::UpstreamUnavailable => (StatusCode::BAD_GATEWAY, "UPSTREAM_UNAVAILABLE"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if let Self::Internal(e) = &self {
            tracing::error!(error = ?e, "internal error");
        }
        let body = ErrorBody {
            detail: self.to_string(),
            code: code.to_owned(),
            status: status.as_u16(),
        };
        (status, axum::Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::internal(e)
    }
}

impl From<crate::pdns::PdnsClientError> for ApiError {
    fn from(e: crate::pdns::PdnsClientError) -> Self {
        match e {
            crate::pdns::PdnsClientError::Unavailable(_) => Self::UpstreamUnavailable,
            crate::pdns::PdnsClientError::Status { status, message } => match status {
                404 => Self::NotFound("zone".to_owned()),
                // Validation messages explain what to fix and are safe to show.
                400 | 409 | 422 => Self::Upstream(message.chars().take(300).collect()),
                _ => {
                    tracing::warn!(status, error = %message, "PowerDNS request failed");
                    Self::Upstream("PowerDNS could not complete the request".to_owned())
                }
            },
            crate::pdns::PdnsClientError::Decode(_) => Self::internal(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn body_of(resp: Response) -> String {
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        String::from_utf8(bytes.to_vec()).expect("utf8")
    }

    #[tokio::test]
    async fn pdns_errors_match_powerdns_bytes() {
        let resp = PdnsError::NotFound.into_response();
        assert_eq!(resp.status(), 404);
        assert_eq!(body_of(resp).await, r#"{"error": "Not Found"}"#);

        let resp = PdnsError::Unauthorized.into_response();
        assert_eq!(
            resp.headers().get("www-authenticate").expect("header"),
            "X-API-Key realm=\"PowerDNS\""
        );
        assert_eq!(body_of(resp).await, r#"{"error": "Unauthorized"}"#);
    }

    #[tokio::test]
    async fn api_errors_use_standard_shape() {
        let resp = ApiError::NotFound("zone".into()).into_response();
        assert_eq!(resp.status(), 404);
        let v: serde_json::Value = serde_json::from_str(&body_of(resp).await).expect("json");
        assert_eq!(v["code"], "NOT_FOUND");
        assert_eq!(v["status"], 404);
        assert_eq!(v["detail"], "zone not found");
    }

    #[tokio::test]
    async fn internal_errors_hide_detail() {
        let resp = ApiError::internal(anyhow::anyhow!("secret path /etc/x")).into_response();
        let body = body_of(resp).await;
        assert!(!body.contains("/etc/x"));
        assert!(body.contains("INTERNAL_ERROR"));
    }
}
