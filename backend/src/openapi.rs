//! The OpenAPI document for `/auth` and `/v1`. The PowerDNS-compatible
//! `/api` is documented by PowerDNS itself.

use utoipa::OpenApi;

use crate::{auth, v1};

/// OpenAPI document.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "SOAbox UI API",
        description = "API used by the SOAbox web interface. See docs/spec/v1-api.md.",
        license(name = "Apache-2.0")
    ),
    paths(
        auth::local::config,
        auth::local::login,
        auth::local::logout,
        v1::me::get_me,
        v1::me::update_me,
        v1::me::change_password,
        v1::me::totp_setup,
        v1::me::totp_enable,
        v1::me::totp_disable,
        v1::server::dashboard,
        v1::server::server,
        v1::server::statistics,
        v1::server::config,
        v1::server::history,
        v1::zones::list,
        v1::zones::create,
        v1::zones::get,
        v1::zones::update,
        v1::zones::remove,
        v1::zones::patch_rrsets,
        v1::zones::notify,
        v1::zones::rectify,
        v1::zones::axfr_retrieve,
        v1::zones::export,
        v1::zones::history,
        v1::zones::set_dnssec,
        v1::zones::list_cryptokeys,
        v1::zones::create_cryptokey,
        v1::zones::update_cryptokey,
        v1::zones::delete_cryptokey,
        v1::zones::list_metadata,
        v1::zones::put_metadata,
        v1::zones::delete_metadata,
        v1::templates::list,
        v1::templates::create,
        v1::templates::update,
        v1::templates::remove,
        v1::admin::list_users,
        v1::admin::create_user,
        v1::admin::update_user,
        v1::admin::set_user_password,
        v1::admin::revoke_sessions,
        v1::admin::reset_totp,
        v1::admin::delete_user,
        v1::admin::list_accounts,
        v1::admin::create_account,
        v1::admin::update_account,
        v1::admin::delete_account,
        v1::admin::get_settings,
        v1::admin::put_settings,
        v1::admin::get_oidc_settings,
        v1::admin::put_oidc_settings,
        v1::admin::list_tsig,
        v1::admin::create_tsig,
        v1::admin::get_tsig,
        v1::admin::update_tsig,
        v1::admin::delete_tsig,
        v1::keys::list,
        v1::keys::create,
        v1::keys::update,
        v1::keys::disable,
        v1::keys::enable,
        v1::keys::remove,
    ),
    components(schemas(crate::errors::ErrorBody, crate::store::HistoryEntry))
)]
pub struct ApiDoc;

/// The document as pretty JSON.
pub fn json() -> String {
    ApiDoc::openapi().to_pretty_json().unwrap_or_default()
}
