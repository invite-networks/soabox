//! Admin settings with defaults and validation.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::store::Store;

/// Whether local accounts may sign in while OIDC is configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LocalLogin {
    Allow,
    Deny,
}

/// What happens to a new OIDC user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NewOidcUser {
    /// Created active with the User role and no zones.
    Create,
    /// Created pending until an Admin approves.
    Approve,
}

/// All settings. See the design's settings table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Settings {
    pub local_login_with_oidc: LocalLogin,
    pub oidc_new_user: NewOidcUser,
    pub user_can_create_zones: bool,
    pub user_can_delete_zones: bool,
    pub user_can_view_history: bool,
    pub user_can_manage_dnssec: bool,
    pub user_can_create_api_keys: bool,
    pub record_types_forward: Vec<String>,
    pub record_types_reverse: Vec<String>,
    pub auto_ptr: bool,
}

/// A partial update. Missing fields are unchanged.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SettingsPatch {
    pub local_login_with_oidc: Option<LocalLogin>,
    pub oidc_new_user: Option<NewOidcUser>,
    pub user_can_create_zones: Option<bool>,
    pub user_can_delete_zones: Option<bool>,
    pub user_can_view_history: Option<bool>,
    pub user_can_manage_dnssec: Option<bool>,
    pub user_can_create_api_keys: Option<bool>,
    pub record_types_forward: Option<Vec<String>>,
    pub record_types_reverse: Option<Vec<String>>,
    pub auto_ptr: Option<bool>,
}

/// Record types PowerDNS supports that may be offered in the editor.
pub const KNOWN_TYPES: &[&str] = &[
    "A",
    "AAAA",
    "AFSDB",
    "ALIAS",
    "APL",
    "CAA",
    "CERT",
    "CDNSKEY",
    "CDS",
    "CNAME",
    "CSYNC",
    "DNSKEY",
    "DNAME",
    "DS",
    "HINFO",
    "HTTPS",
    "KEY",
    "LOC",
    "LUA",
    "MX",
    "NAPTR",
    "NS",
    "NSEC",
    "NSEC3",
    "NSEC3PARAM",
    "OPENPGPKEY",
    "PTR",
    "RP",
    "RRSIG",
    "SOA",
    "SPF",
    "SSHFP",
    "SRV",
    "SVCB",
    "TLSA",
    "SMIMEA",
    "TXT",
    "URI",
];

impl Default for Settings {
    fn default() -> Self {
        let list = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        Self {
            local_login_with_oidc: LocalLogin::Allow,
            oidc_new_user: NewOidcUser::Create,
            user_can_create_zones: false,
            user_can_delete_zones: false,
            user_can_view_history: true,
            user_can_manage_dnssec: false,
            user_can_create_api_keys: false,
            record_types_forward: list(&[
                "A", "AAAA", "ALIAS", "CAA", "CNAME", "DS", "HTTPS", "LOC", "MX", "NAPTR", "NS",
                "PTR", "SOA", "SPF", "SRV", "SSHFP", "SVCB", "TLSA", "TXT",
            ]),
            record_types_reverse: list(&["CNAME", "LOC", "NS", "PTR", "SOA", "TXT"]),
            auto_ptr: false,
        }
    }
}

impl Settings {
    /// Loads settings, filling gaps with defaults. Unparseable stored values
    /// fall back to the default and are logged.
    pub async fn load(store: &Store) -> Result<Self, sqlx::Error> {
        let stored = store.all_settings().await?;
        let mut value = serde_json::to_value(Self::default()).unwrap_or_default();
        if let Some(obj) = value.as_object_mut() {
            for (k, v) in stored {
                if obj.contains_key(&k) {
                    match serde_json::from_str(&v) {
                        Ok(parsed) => {
                            obj.insert(k, parsed);
                        }
                        Err(e) => tracing::warn!(key = %k, error = %e, "ignoring bad setting"),
                    }
                }
            }
        }
        Ok(serde_json::from_value(value).unwrap_or_default())
    }

    /// Applies a patch, validates, and stores the changed keys.
    pub async fn apply(
        store: &Store,
        patch: SettingsPatch,
    ) -> Result<Result<Self, String>, sqlx::Error> {
        let mut s = Self::load(store).await?;
        macro_rules! set {
            ($($f:ident),+) => { $( if let Some(v) = patch.$f { s.$f = v; } )+ };
        }
        set!(
            local_login_with_oidc,
            oidc_new_user,
            user_can_create_zones,
            user_can_delete_zones,
            user_can_view_history,
            user_can_manage_dnssec,
            user_can_create_api_keys,
            record_types_forward,
            record_types_reverse,
            auto_ptr
        );
        for list in [&mut s.record_types_forward, &mut s.record_types_reverse] {
            for t in list.iter_mut() {
                *t = t.trim().to_ascii_uppercase();
            }
            list.sort();
            list.dedup();
            if let Some(bad) = list.iter().find(|t| !KNOWN_TYPES.contains(&t.as_str())) {
                return Ok(Err(format!("unknown record type {bad}")));
            }
            if list.is_empty() {
                return Ok(Err("record type lists cannot be empty".to_owned()));
            }
        }
        let value = serde_json::to_value(&s).unwrap_or_default();
        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                store.put_setting(k, &v.to_string()).await?;
            }
        }
        Ok(Ok(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing;

    #[tokio::test]
    async fn defaults_and_patch() {
        let (s, _d) = testing::sqlite().await;
        let d = Settings::load(&s).await.expect("load");
        assert_eq!(d, Settings::default());
        assert_eq!(d.local_login_with_oidc, LocalLogin::Allow);

        let r = Settings::apply(
            &s,
            SettingsPatch {
                local_login_with_oidc: Some(LocalLogin::Deny),
                record_types_reverse: Some(vec!["ptr".into(), "PTR".into(), "ns".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("db")
        .expect("valid");
        assert_eq!(r.local_login_with_oidc, LocalLogin::Deny);
        assert_eq!(r.record_types_reverse, vec!["NS", "PTR"]);
        assert_eq!(Settings::load(&s).await.expect("load"), r);

        let bad = Settings::apply(
            &s,
            SettingsPatch {
                record_types_forward: Some(vec!["BOGUS".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("db");
        assert!(bad.is_err());
        let empty = Settings::apply(
            &s,
            SettingsPatch {
                record_types_forward: Some(vec![]),
                ..Default::default()
            },
        )
        .await
        .expect("db");
        assert!(empty.is_err());
    }

    #[tokio::test]
    async fn corrupt_values_fall_back() {
        let (s, _d) = testing::sqlite().await;
        s.put_setting("auto_ptr", "not json").await.expect("put");
        s.put_setting("unknown_key", "1").await.expect("put");
        assert_eq!(Settings::load(&s).await.expect("load"), Settings::default());
    }
}
