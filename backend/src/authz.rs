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
}

impl Principal {
    /// Builds the principal for an API key, resolving account scope.
    pub async fn for_api_key(store: &Store, key: &ApiKeyRecord) -> Result<Self, sqlx::Error> {
        let level = match key.role {
            Role::Admin => Level::Admin,
            Role::Operator => Level::Operator,
            Role::Scoped => Level::Limited,
        };
        let mut zones: HashSet<String> = key.zones.iter().cloned().collect();
        if level == Level::Limited {
            zones.extend(store.zones_for_accounts(&key.account_ids).await?);
        }
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
            can_create: key.create_account_id.is_some(),
            can_delete_zones: true,
        })
    }

    /// Builds the principal for a signed-in user.
    pub async fn for_user(
        store: &Store,
        user: &User,
        settings: &Settings,
    ) -> Result<Self, sqlx::Error> {
        let level = match user.role {
            UserRole::Admin => Level::Admin,
            UserRole::Operator => Level::Operator,
            UserRole::User => Level::Limited,
        };
        let zones = if level == Level::Limited {
            store
                .user_access_zones(user.id)
                .await?
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        };
        Ok(Self {
            actor_type: "user",
            actor_id: user.id,
            actor_name: user.username.clone(),
            level,
            zones,
            create_account_id: None,
            can_create: settings.user_can_create_zones,
            can_delete_zones: settings.user_can_delete_zones,
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
