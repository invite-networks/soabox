//! Route classification and permission decisions.
//!
//! Every request to PowerDNS, from the passthrough or the UI, is decided here.

use std::collections::HashSet;

use http::Method;

use crate::pdns::ZoneName;
use crate::settings::Settings;
use crate::store::{ApiKeyRecord, Role, Store, User, UserRole};

/// How much of PowerDNS a principal may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Everything, including server administration.
    Admin,
    /// All zones and read-only server views.
    Operator,
    /// Only the zones in [`Principal::zones`].
    Limited,
}

/// Who is acting, and what they may reach.
#[derive(Debug, Clone)]
pub struct Principal {
    /// `api_key` or `user`.
    pub actor_type: &'static str,
    pub actor_id: i64,
    pub actor_name: String,
    pub level: Level,
    /// Canonical zone names in scope. Used only for [`Level::Limited`].
    pub zones: HashSet<String>,
    /// Account new zones are assigned to, if any.
    pub create_account_id: Option<i64>,
    /// Whether a limited principal may create zones.
    pub can_create: bool,
    /// Whether a limited principal may delete zones in scope.
    pub can_delete_zones: bool,
    /// Whether DNSSEC keys, DNSSEC zone flags, and metadata may be changed.
    pub manage_dnssec: bool,
    /// Record types allowed in forward and reverse zones. `None` means no
    /// list applies (Admins, Operators, and keys issued by them).
    pub allowed_types: Option<(Vec<String>, Vec<String>)>,
    /// Whether secondary zones and their primaries may be set. Primaries make
    /// PowerDNS connect to other hosts, so this is staff only.
    pub manage_secondaries: bool,
}

fn level_of_role(role: UserRole) -> Level {
    match role {
        UserRole::Admin => Level::Admin,
        UserRole::Operator => Level::Operator,
        UserRole::User => Level::Limited,
    }
}

fn rank(l: Level) -> u8 {
    match l {
        Level::Admin => 2,
        Level::Operator => 1,
        Level::Limited => 0,
    }
}

impl Principal {
    /// Builds the principal for an API key. A key never exceeds its owner:
    /// its level is capped at the owner's current role, and a key owned by a
    /// User is limited to the owner's current zones and follows the User
    /// policy settings.
    pub async fn for_api_key(
        store: &Store,
        key: &ApiKeyRecord,
        settings: &Settings,
    ) -> Result<Self, sqlx::Error> {
        let key_level = match key.role {
            Role::Admin => Level::Admin,
            Role::Operator => Level::Operator,
            Role::Scoped => Level::Limited,
        };
        let owner = match key.owner_user_id {
            Some(uid) => store.get_user(uid).await?,
            None => None,
        };
        let owner_level = owner.as_ref().map(|u| level_of_role(u.role));
        let level = match owner_level {
            Some(o) if rank(o) < rank(key_level) => o,
            _ => key_level,
        };
        let mut zones: HashSet<String> = HashSet::new();
        if level == Level::Limited {
            zones.extend(key.zones.iter().cloned());
            zones.extend(store.zones_for_accounts(&key.account_ids).await?);
            if let Some(o) = owner.as_ref().filter(|u| u.role == UserRole::User) {
                let allowed: HashSet<String> =
                    store.user_access_zones(o.id).await?.into_iter().collect();
                zones.retain(|z| allowed.contains(z));
            }
        }
        let user_owned = owner.as_ref().is_some_and(|u| u.role == UserRole::User);
        let name = if key.description.is_empty() {
            format!("api key {}", key.id)
        } else {
            format!("api key {} ({})", key.id, key.description)
        };
        Ok(Self {
            actor_type: "api_key",
            actor_id: key.id,
            actor_name: name,
            level,
            zones,
            create_account_id: key.create_account_id,
            can_create: key.create_account_id.is_some()
                && (!user_owned || settings.user_can_create_zones),
            can_delete_zones: !user_owned || settings.user_can_delete_zones,
            manage_dnssec: !user_owned || settings.user_can_manage_dnssec,
            allowed_types: user_owned.then(|| {
                (
                    settings.record_types_forward.clone(),
                    settings.record_types_reverse.clone(),
                )
            }),
            manage_secondaries: level != Level::Limited,
        })
    }

    /// Builds the principal for a signed-in user.
    pub async fn for_user(
        store: &Store,
        user: &User,
        settings: &Settings,
    ) -> Result<Self, sqlx::Error> {
        let level = level_of_role(user.role);
        let zones = if level == Level::Limited {
            store
                .user_access_zones(user.id)
                .await?
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        };
        let staff = level != Level::Limited;
        Ok(Self {
            actor_type: "user",
            actor_id: user.id,
            actor_name: user.username.clone(),
            level,
            zones,
            create_account_id: None,
            can_create: settings.user_can_create_zones,
            can_delete_zones: settings.user_can_delete_zones,
            manage_dnssec: staff || settings.user_can_manage_dnssec,
            allowed_types: Some((
                settings.record_types_forward.clone(),
                settings.record_types_reverse.clone(),
            )),
            manage_secondaries: staff,
        })
    }

    /// True if the principal can reach every zone.
    pub fn all_zones(&self) -> bool {
        self.level != Level::Limited
    }

    /// True if `zone` is in scope.
    pub fn can_access_zone(&self, zone: &ZoneName) -> bool {
        self.all_zones() || self.zones.contains(zone.as_str())
    }

    /// True if the principal may create zones.
    pub fn can_create_zones(&self) -> bool {
        self.all_zones() || self.can_create
    }

    /// Checks a record type for a zone. `Err` carries the reason.
    pub fn check_record_type(&self, zone: &ZoneName, rtype: &str) -> Result<(), String> {
        let t = rtype.to_ascii_uppercase();
        match &self.allowed_types {
            Some((forward, reverse)) => {
                let list = if is_reverse_zone(zone) {
                    reverse
                } else {
                    forward
                };
                if list.contains(&t) {
                    Ok(())
                } else {
                    Err(format!("record type {t} is not allowed in this zone"))
                }
            }
            None if t == "LUA" && self.level != Level::Admin => {
                Err("LUA records are reserved for Admins".to_owned())
            }
            None => Ok(()),
        }
    }

    /// Checks whether this principal may write a zone metadata kind.
    pub fn check_metadata_kind(&self, kind: &str) -> Result<(), String> {
        if !self.manage_dnssec {
            return Err("you may not change zone metadata".to_owned());
        }
        if self.level == Level::Admin || SAFE_METADATA.contains(&kind.to_ascii_uppercase().as_str())
        {
            Ok(())
        } else {
            Err(format!("metadata kind {kind} is reserved for Admins"))
        }
    }

    /// For limited principals, refuses a new zone that is equal to, above,
    /// or below a zone outside their scope.
    pub fn check_new_zone(&self, new: &ZoneName, existing: &[ZoneName]) -> Result<(), String> {
        if self.all_zones() {
            return Ok(());
        }
        let clash = existing.iter().find(|z| {
            !self.can_access_zone(z)
                && (z.contains_name(new.as_str()) || new.contains_name(z.as_str()))
        });
        match clash {
            Some(z) => Err(format!("{new} overlaps {z}, which you do not manage")),
            None => Ok(()),
        }
    }
}

/// Metadata kinds non-Admins may set. Kinds that run code, make PowerDNS
/// contact other hosts, or forward updates are Admin only.
pub const SAFE_METADATA: &[&str] = &[
    "ALLOW-AXFR-FROM",
    "ALLOW-DNSUPDATE-FROM",
    "API-RECTIFY",
    "AXFR-MASTER-TSIG",
    "IXFR",
    "NOTIFY-DNSUPDATE",
    "NSEC3NARROW",
    "NSEC3PARAM",
    "PUBLISH-CDNSKEY",
    "PUBLISH-CDS",
    "SLAVE-RENOTIFY",
    "SOA-EDIT",
    "SOA-EDIT-API",
    "SOA-EDIT-DNSUPDATE",
    "TSIG-ALLOW-AXFR",
    "TSIG-ALLOW-DNSUPDATE",
];

/// True for in-addr.arpa and ip6.arpa zones.
pub fn is_reverse_zone(zone: &ZoneName) -> bool {
    zone.as_str().ends_with(".in-addr.arpa.") || zone.as_str().ends_with(".ip6.arpa.")
}

/// True when a zone kind or primaries list makes it a secondary zone.
pub fn is_secondary_request(body: &serde_json::Value) -> bool {
    let kind = body
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let has_primaries = body
        .get("masters")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|m| !m.is_empty());
    kind == "slave" || kind == "secondary" || kind == "consumer" || has_primaries
}

/// What a `/api` request is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteClass {
    /// `/api`, `/api/v1`, `/api/v1/servers`, `/api/v1/servers/{id}`.
    Discovery,
    /// Any path naming a server id SOAbox does not serve.
    OtherServer,
    /// `GET .../zones`.
    ZoneList,
    /// `POST .../zones`.
    ZoneCreate,
    /// Other methods on `.../zones`. PowerDNS answers `405`.
    ZoneCollectionOther,
    /// Anything under `.../zones/{zone_id}`.
    Zone {
        zone: ZoneName,
        variant: Option<String>,
        /// Path segments after the zone id.
        rest: Vec<String>,
    },
    /// A zone id that cannot be parsed.
    BadZoneId,
    /// `GET .../search-data`.
    Search,
    /// `GET .../statistics`, `GET .../config`.
    ServerRead,
    /// tsigkeys, autoprimaries, cache, views, networks, config writes.
    ServerAdmin,
    /// A path SOAbox does not recognize.
    Unknown,
}

/// The decision for a classified request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Forward as is.
    Allow,
    /// Forward and filter the list to zones in scope.
    AllowFiltered,
    /// Answer exactly as PowerDNS answers a missing zone or server.
    NotFound,
    /// Answer exactly as PowerDNS answers a rejected key.
    Unauthorized,
}

/// Classifies a path below `/api`. `segments` excludes the leading `api`.
pub fn classify(method: &Method, segments: &[&str], server_id: &str) -> RouteClass {
    match segments {
        [] | ["v1"] | ["v1", "servers"] => RouteClass::Discovery,
        ["v1", "servers", sid, rest @ ..] => {
            if *sid != server_id {
                return RouteClass::OtherServer;
            }
            classify_server(method, rest)
        }
        _ => RouteClass::Unknown,
    }
}

fn classify_server(method: &Method, rest: &[&str]) -> RouteClass {
    match rest {
        [] => RouteClass::Discovery,
        ["zones"] => match *method {
            Method::GET | Method::HEAD => RouteClass::ZoneList,
            Method::POST => RouteClass::ZoneCreate,
            _ => RouteClass::ZoneCollectionOther,
        },
        ["zones", id, tail @ ..] => match ZoneName::from_url_id(id) {
            Some((zone, variant)) => RouteClass::Zone {
                zone,
                variant,
                rest: tail.iter().map(|s| (*s).to_owned()).collect(),
            },
            None => RouteClass::BadZoneId,
        },
        ["search-data"] => RouteClass::Search,
        ["statistics"] | ["config"] | ["config", _]
            if matches!(*method, Method::GET | Method::HEAD) =>
        {
            RouteClass::ServerRead
        }
        ["statistics", ..] | ["config", ..] => RouteClass::ServerAdmin,
        ["tsigkeys", ..]
        | ["autoprimaries", ..]
        | ["cache", ..]
        | ["views", ..]
        | ["networks", ..] => RouteClass::ServerAdmin,
        _ => RouteClass::Unknown,
    }
}

/// Decides whether `p` may make the classified request.
pub fn decide(p: &Principal, method: &Method, class: &RouteClass) -> Decision {
    use Decision::{Allow, AllowFiltered, NotFound, Unauthorized};
    match (p.level, class) {
        (_, RouteClass::OtherServer) => NotFound,
        (_, RouteClass::BadZoneId) => NotFound,
        (Level::Admin, _) => Allow,
        (_, RouteClass::Discovery) => Allow,
        (Level::Operator, RouteClass::ZoneList | RouteClass::Search) => Allow,
        (Level::Limited, RouteClass::ZoneList | RouteClass::Search) => AllowFiltered,
        (Level::Operator, RouteClass::ZoneCreate | RouteClass::ZoneCollectionOther) => Allow,
        (Level::Limited, RouteClass::ZoneCreate) => {
            if p.can_create_zones() {
                Allow
            } else {
                Unauthorized
            }
        }
        (Level::Limited, RouteClass::ZoneCollectionOther) => Allow,
        (Level::Operator, RouteClass::Zone { .. }) => Allow,
        (Level::Limited, RouteClass::Zone { zone, rest, .. }) => {
            if !p.can_access_zone(zone) {
                NotFound
            } else if *method == Method::DELETE && rest.is_empty() && !p.can_delete_zones {
                Unauthorized
            } else {
                Allow
            }
        }
        (Level::Operator, RouteClass::ServerRead) => Allow,
        (_, RouteClass::ServerRead | RouteClass::ServerAdmin | RouteClass::Unknown) => Unauthorized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn principal(level: Level, zones: &[&str], create: bool) -> Principal {
        Principal {
            actor_type: "api_key",
            actor_id: 1,
            actor_name: "k".into(),
            level,
            zones: zones.iter().map(|z| (*z).to_owned()).collect(),
            create_account_id: create.then_some(1),
            can_create: create,
            can_delete_zones: true,
            manage_dnssec: true,
            allowed_types: None,
            manage_secondaries: level != Level::Limited,
        }
    }

    fn segs(path: &str) -> Vec<&str> {
        path.split('/').filter(|s| !s.is_empty()).collect()
    }

    fn class(method: &Method, path: &str) -> RouteClass {
        classify(method, &segs(path), "localhost")
    }

    #[rstest]
    #[case("GET", "", RouteClass::Discovery)]
    #[case("GET", "v1", RouteClass::Discovery)]
    #[case("GET", "v1/servers", RouteClass::Discovery)]
    #[case("GET", "v1/servers/localhost", RouteClass::Discovery)]
    #[case("GET", "v1/servers/other/zones", RouteClass::OtherServer)]
    #[case("GET", "v1/servers/localhost/zones", RouteClass::ZoneList)]
    #[case("POST", "v1/servers/localhost/zones", RouteClass::ZoneCreate)]
    #[case("PUT", "v1/servers/localhost/zones", RouteClass::ZoneCollectionOther)]
    #[case("GET", "v1/servers/localhost/search-data", RouteClass::Search)]
    #[case("GET", "v1/servers/localhost/statistics", RouteClass::ServerRead)]
    #[case("GET", "v1/servers/localhost/config", RouteClass::ServerRead)]
    #[case(
        "GET",
        "v1/servers/localhost/config/allow-axfr-ips",
        RouteClass::ServerRead
    )]
    #[case(
        "PUT",
        "v1/servers/localhost/config/allow-axfr-ips",
        RouteClass::ServerAdmin
    )]
    #[case("GET", "v1/servers/localhost/tsigkeys", RouteClass::ServerAdmin)]
    #[case("PUT", "v1/servers/localhost/cache/flush", RouteClass::ServerAdmin)]
    #[case("GET", "v1/servers/localhost/autoprimaries", RouteClass::ServerAdmin)]
    #[case("GET", "v1/servers/localhost/views", RouteClass::ServerAdmin)]
    #[case(
        "GET",
        "v1/servers/localhost/networks/10.0.0.0/8",
        RouteClass::ServerAdmin
    )]
    #[case("GET", "v1/servers/localhost/bogus", RouteClass::Unknown)]
    #[case("GET", "v2", RouteClass::Unknown)]
    #[case("GET", "v1/servers/localhost/zones/bad=zz", RouteClass::BadZoneId)]
    fn classification(#[case] method: &str, #[case] path: &str, #[case] expected: RouteClass) {
        let m: Method = method.parse().expect("method");
        assert_eq!(class(&m, path), expected);
    }

    #[test]
    fn zone_classification_canonicalizes() {
        match class(
            &Method::PATCH,
            "v1/servers/localhost/zones/EXAMPLE.com/metadata/X",
        ) {
            RouteClass::Zone { zone, rest, .. } => {
                assert_eq!(zone.as_str(), "example.com.");
                assert_eq!(rest, vec!["metadata", "X"]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[rstest]
    // admin
    #[case(Level::Admin, "GET", "v1/servers/localhost/tsigkeys", Decision::Allow)]
    #[case(Level::Admin, "GET", "v1/servers/localhost/bogus", Decision::Allow)]
    #[case(Level::Admin, "GET", "v1/servers/other", Decision::NotFound)]
    // operator
    #[case(Level::Operator, "GET", "v1/servers/localhost/zones", Decision::Allow)]
    #[case(Level::Operator, "POST", "v1/servers/localhost/zones", Decision::Allow)]
    #[case(
        Level::Operator,
        "PATCH",
        "v1/servers/localhost/zones/x.test.",
        Decision::Allow
    )]
    #[case(
        Level::Operator,
        "GET",
        "v1/servers/localhost/statistics",
        Decision::Allow
    )]
    #[case(
        Level::Operator,
        "GET",
        "v1/servers/localhost/tsigkeys",
        Decision::Unauthorized
    )]
    #[case(
        Level::Operator,
        "PUT",
        "v1/servers/localhost/cache/flush",
        Decision::Unauthorized
    )]
    #[case(
        Level::Operator,
        "GET",
        "v1/servers/localhost/bogus",
        Decision::Unauthorized
    )]
    // limited
    #[case(Level::Limited, "GET", "v1", Decision::Allow)]
    #[case(Level::Limited, "GET", "v1/servers/localhost", Decision::Allow)]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/zones",
        Decision::AllowFiltered
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/search-data",
        Decision::AllowFiltered
    )]
    #[case(
        Level::Limited,
        "PATCH",
        "v1/servers/localhost/zones/a.test.",
        Decision::Allow
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/zones/A.TEST",
        Decision::Allow
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/zones/a.test..v1",
        Decision::Allow
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/zones/b.test.",
        Decision::NotFound
    )]
    #[case(
        Level::Limited,
        "DELETE",
        "v1/servers/localhost/zones/b.test.",
        Decision::NotFound
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/zones/bad=zz",
        Decision::NotFound
    )]
    #[case(
        Level::Limited,
        "POST",
        "v1/servers/localhost/zones",
        Decision::Unauthorized
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/statistics",
        Decision::Unauthorized
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/config",
        Decision::Unauthorized
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/tsigkeys",
        Decision::Unauthorized
    )]
    #[case(
        Level::Limited,
        "GET",
        "v1/servers/localhost/bogus",
        Decision::Unauthorized
    )]
    fn decisions(
        #[case] level: Level,
        #[case] method: &str,
        #[case] path: &str,
        #[case] expected: Decision,
    ) {
        let p = principal(level, &["a.test."], false);
        let m: Method = method.parse().expect("method");
        assert_eq!(decide(&p, &m, &class(&m, path)), expected);
    }

    #[test]
    fn limited_create_and_delete_permissions() {
        let mut p = principal(Level::Limited, &["a.test."], true);
        let create = class(&Method::POST, "v1/servers/localhost/zones");
        assert_eq!(decide(&p, &Method::POST, &create), Decision::Allow);
        p.can_delete_zones = false;
        let del = class(&Method::DELETE, "v1/servers/localhost/zones/a.test.");
        assert_eq!(decide(&p, &Method::DELETE, &del), Decision::Unauthorized);
        let del_meta = class(
            &Method::DELETE,
            "v1/servers/localhost/zones/a.test./metadata/X",
        );
        assert_eq!(decide(&p, &Method::DELETE, &del_meta), Decision::Allow);
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    fn p(level: Level) -> Principal {
        Principal {
            actor_type: "user",
            actor_id: 1,
            actor_name: "u".into(),
            level,
            zones: ["a.test.".to_owned()].into_iter().collect(),
            create_account_id: None,
            can_create: true,
            can_delete_zones: true,
            manage_dnssec: true,
            allowed_types: None,
            manage_secondaries: level != Level::Limited,
        }
    }

    fn z(n: &str) -> ZoneName {
        ZoneName::from_name(n).expect("zone")
    }

    #[test]
    fn metadata_allowlist() {
        let admin = p(Level::Admin);
        let op = p(Level::Operator);
        assert!(admin.check_metadata_kind("LUA-AXFR-SCRIPT").is_ok());
        assert!(op.check_metadata_kind("LUA-AXFR-SCRIPT").is_err());
        assert!(op.check_metadata_kind("ALSO-NOTIFY").is_err());
        assert!(op.check_metadata_kind("allow-axfr-from").is_ok());
        let mut limited = p(Level::Limited);
        limited.manage_dnssec = false;
        assert!(limited.check_metadata_kind("SOA-EDIT-API").is_err());
    }

    #[test]
    fn record_types() {
        let mut u = p(Level::Limited);
        assert!(u.check_record_type(&z("a.test"), "LUA").is_err());
        assert!(
            p(Level::Admin)
                .check_record_type(&z("a.test"), "LUA")
                .is_ok()
        );
        u.allowed_types = Some((vec!["A".into()], vec!["PTR".into()]));
        assert!(u.check_record_type(&z("a.test"), "a").is_ok());
        assert!(u.check_record_type(&z("a.test"), "MX").is_err());
        assert!(
            u.check_record_type(&z("2.0.192.in-addr.arpa"), "PTR")
                .is_ok()
        );
        assert!(
            u.check_record_type(&z("2.0.192.in-addr.arpa"), "A")
                .is_err()
        );
    }

    #[test]
    fn new_zone_overlap() {
        let u = p(Level::Limited);
        let existing = vec![z("a.test"), z("victim.test")];
        assert!(u.check_new_zone(&z("sub.a.test"), &existing).is_ok());
        assert!(
            u.check_new_zone(&z("login.victim.test"), &existing)
                .is_err()
        );
        assert!(u.check_new_zone(&z("test"), &existing).is_err());
        assert!(u.check_new_zone(&z("other.test"), &existing).is_ok());
        assert!(
            p(Level::Operator)
                .check_new_zone(&z("login.victim.test"), &existing)
                .is_ok()
        );
    }

    #[test]
    fn secondary_detection() {
        assert!(is_secondary_request(&serde_json::json!({"kind": "Slave"})));
        assert!(is_secondary_request(
            &serde_json::json!({"kind": "Native", "masters": ["192.0.2.1"]})
        ));
        assert!(!is_secondary_request(
            &serde_json::json!({"kind": "Native", "masters": []})
        ));
    }
}
