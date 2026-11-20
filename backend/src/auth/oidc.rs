//! OpenID Connect sign-in (authorization code flow with PKCE).
//!
//! The backend runs the whole flow. Tokens never reach the browser. The
//! state, nonce, and PKCE verifier travel in a short-lived encrypted cookie.

use axum::Extension;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};

use super::{crypto, provider};
use crate::config::OidcConfig;
use crate::errors::ApiError;
use crate::http::ClientInfo;
use crate::settings::{NewOidcUser, Settings};
use crate::state::AppState;
use crate::store::{NewUser, User, UserRole, UserStatus, now};

const FLOW_COOKIE: &str = "soabox_oidc";
const FLOW_TTL_SECS: i64 = 600;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct FlowState {
    state: String,
    nonce: String,
    verifier: String,
    expires_at: i64,
}

fn encode_flow(key: &[u8; 32], f: &FlowState) -> anyhow::Result<String> {
    let json = serde_json::to_vec(f)?;
    Ok(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(crypto::encrypt(
            crypto::PURPOSE_OIDC_FLOW,
            key,
            &json,
        )?),
    )
}

fn decode_flow(keys: &[&[u8; 32]], raw: &str) -> Option<FlowState> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .ok()?;
    let json = crypto::decrypt(crypto::PURPOSE_OIDC_FLOW, keys, &bytes)?;
    serde_json::from_slice(&json).ok()
}

pub(super) fn http_client() -> Result<openidconnect::reqwest::Client, ApiError> {
    openidconnect::reqwest::ClientBuilder::new()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(ApiError::internal)
}

/// The active provider settings and a client built from its discovery
/// document. Answers `404` when single sign-on is off.
async fn client(state: &AppState) -> Result<(OidcConfig, Client), ApiError> {
    let cfg = provider::active(state)
        .await?
        .ok_or_else(|| ApiError::NotFound("single sign-on".into()))?;
    let meta = state.oidc_metadata.get(&cfg.issuer).await?;
    let client = CoreClient::from_provider_metadata(
        meta,
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(cfg.client_secret.expose().to_owned())),
    )
    .set_redirect_uri(
        RedirectUrl::new(provider::redirect_uri(&state.config.public_url))
            .map_err(ApiError::internal)?,
    );
    Ok((cfg, client))
}

type Client = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

fn login_error(code: &str) -> Response {
    Redirect::to(&format!("/login?error={code}")).into_response()
}

/// `GET /auth/oidc/start`
pub async fn start(State(state): State<AppState>, jar: CookieJar) -> Result<Response, ApiError> {
    let (_cfg, client) = client(&state).await?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(challenge)
        .url();
    let flow = FlowState {
        state: csrf.secret().clone(),
        nonce: nonce.secret().clone(),
        verifier: verifier.secret().clone(),
        expires_at: now() + FLOW_TTL_SECS,
    };
    let cookie = Cookie::build((
        FLOW_COOKIE,
        encode_flow(&state.config.secret_key, &flow).map_err(ApiError::Internal)?,
    ))
    .path("/auth/oidc")
    .http_only(true)
    .secure(super::secure_cookies(&state))
    .same_site(SameSite::Lax)
    .max_age(time::Duration::seconds(FLOW_TTL_SECS))
    .build();
    Ok((jar.add(cookie), Redirect::to(url.as_str())).into_response())
}

/// Query parameters on the provider's redirect back.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /auth/oidc/callback`
pub async fn callback(
    State(state): State<AppState>,
    Extension(client_info): Extension<ClientInfo>,
    headers: HeaderMap,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> Result<Response, ApiError> {
    if provider::active(&state).await?.is_none() {
        return Err(ApiError::NotFound("single sign-on".into()));
    }
    let flow = jar
        .get(FLOW_COOKIE)
        .and_then(|c| decode_flow(&state.secret_keys(), c.value()));
    let clear = Cookie::build((FLOW_COOKIE, ""))
        .path("/auth/oidc")
        .max_age(time::Duration::seconds(0))
        .build();
    let jar = jar.add(clear);

    if let Some(err) = q.error {
        tracing::info!(error = %err, "OIDC provider returned an error");
        return Ok((jar, login_error("oidc_failed")).into_response());
    }
    let (Some(flow), Some(code), Some(returned_state)) = (flow, q.code, q.state) else {
        return Ok((jar, login_error("oidc_failed")).into_response());
    };
    if flow.expires_at < now() || !crypto::constant_eq_str(&flow.state, &returned_state) {
        return Ok((jar, login_error("oidc_failed")).into_response());
    }

    let (_cfg, client) = client(&state).await?;
    let http = http_client()?;
    let token = match client.exchange_code(AuthorizationCode::new(code)) {
        Ok(req) => {
            req.set_pkce_verifier(PkceCodeVerifier::new(flow.verifier))
                .request_async(&http)
                .await
        }
        Err(e) => {
            tracing::warn!(error = %e, "OIDC client has no token endpoint");
            return Ok((jar, login_error("oidc_failed")).into_response());
        }
    };
    let token = match token {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "OIDC code exchange failed");
            return Ok((jar, login_error("oidc_failed")).into_response());
        }
    };
    let Some(id_token) = token.id_token() else {
        return Ok((jar, login_error("oidc_failed")).into_response());
    };
    let claims = match id_token.claims(&client.id_token_verifier(), &Nonce::new(flow.nonce)) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "ID token rejected");
            return Ok((jar, login_error("oidc_failed")).into_response());
        }
    };

    let identity = OidcIdentity {
        issuer: claims.issuer().as_str().to_owned(),
        subject: claims.subject().as_str().to_owned(),
        preferred_username: claims.preferred_username().map(|u| u.as_str().to_owned()),
        email: claims.email().map(|e| e.as_str().to_owned()),
        name: claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_owned()),
    };
    let settings = Settings::load(&state.store).await?;
    let user = match provision(&state, &identity, &settings).await? {
        Provisioned::Active(u) => u,
        Provisioned::Pending => return Ok((jar, login_error("pending_approval")).into_response()),
        Provisioned::Disabled => return Ok((jar, login_error("disabled")).into_response()),
    };

    super::end_session(&state, &jar).await?;
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let (cookie, _csrf) = super::start_session(&state, &user, &client_info, ua).await?;
    tracing::info!(user = %user.username, client_ip = %client_info.ip, "signed in with OIDC");
    Ok((jar.add(cookie), Redirect::to("/")).into_response())
}

/// Identity claims taken from a validated ID token.
#[derive(Debug, Clone)]
pub struct OidcIdentity {
    pub issuer: String,
    pub subject: String,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// Result of matching or creating an OIDC user.
#[derive(Debug)]
pub enum Provisioned {
    Active(Box<User>),
    Pending,
    Disabled,
}

/// Finds the user for an identity by issuer and subject, creating one on
/// first sign-in according to the `oidc_new_user` setting.
pub async fn provision(
    state: &AppState,
    id: &OidcIdentity,
    settings: &Settings,
) -> Result<Provisioned, ApiError> {
    if let Some(u) = state
        .store
        .find_user_by_oidc(&id.issuer, &id.subject)
        .await?
    {
        return Ok(match u.status {
            UserStatus::Active => Provisioned::Active(Box::new(u)),
            UserStatus::Pending => Provisioned::Pending,
            UserStatus::Disabled => Provisioned::Disabled,
        });
    }
    let base = id
        .preferred_username
        .clone()
        .or_else(|| id.email.clone())
        .unwrap_or_else(|| format!("user-{}", id.subject.chars().take(12).collect::<String>()))
        .to_ascii_lowercase();
    let mut username = base.clone();
    let mut n = 2;
    while state
        .store
        .find_user_by_username(&username)
        .await?
        .is_some()
    {
        username = format!("{base}-{n}");
        n += 1;
    }
    let status = match settings.oidc_new_user {
        NewOidcUser::Create => UserStatus::Active,
        NewOidcUser::Approve => UserStatus::Pending,
    };
    let uid = state
        .store
        .create_user(&NewUser {
            username: username.clone(),
            display_name: id.name.clone().unwrap_or_else(|| username.clone()),
            email: id.email.clone().unwrap_or_default(),
            role: UserRole::User,
            auth_source: "oidc",
            oidc_issuer: Some(id.issuer.clone()),
            oidc_subject: Some(id.subject.clone()),
            password_hash: None,
            status,
        })
        .await?;
    tracing::info!(username = %username, status = %status, "created OIDC user");
    match status {
        UserStatus::Active => {
            let u = state
                .store
                .get_user(uid)
                .await?
                .ok_or_else(|| ApiError::internal(anyhow::anyhow!("user vanished")))?;
            Ok(Provisioned::Active(Box::new(u)))
        }
        _ => Ok(Provisioned::Pending),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_cookie_round_trip_and_tamper() {
        let k = [7u8; 32];
        let f = FlowState {
            state: "s".into(),
            nonce: "n".into(),
            verifier: "v".into(),
            expires_at: 10,
        };
        let enc = encode_flow(&k, &f).expect("enc");
        assert_eq!(decode_flow(&[&k], &enc), Some(f));
        assert_eq!(decode_flow(&[&[8u8; 32]], &enc), None);
        assert_eq!(decode_flow(&[&k], "!!!"), None);
    }
}
