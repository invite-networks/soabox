//! API key generation, lookup, and failed-attempt limiting.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use ipnet::IpNet;
use sha2::{Digest, Sha256};

use crate::store::{ApiKeyRecord, Store, now};

/// A newly generated key. `secret` is shown to the user once.
#[derive(Debug)]
pub struct GeneratedKey {
    pub secret: String,
    pub hash: String,
    pub hint: String,
}

/// Generates a random 256-bit key with no recognizable prefix.
pub fn generate() -> Result<GeneratedKey, getrandom::Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    let secret = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let hint = secret[secret.len() - 4..].to_owned();
    Ok(GeneratedKey {
        hash: digest(&secret),
        secret,
        hint,
    })
}

/// SHA-256 digest of a key, hex encoded. See ADR-0013.
pub fn digest(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

/// Why a key was not accepted. All map to the same PowerDNS `401`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRejection {
    Missing,
    Unknown,
    Disabled,
    Expired,
    Network,
    RateLimited,
}

/// Authenticates keys and limits repeated failures per client address.
#[derive(Debug)]
pub struct KeyService {
    failures: Mutex<HashMap<IpAddr, (u32, Instant)>>,
    last_touch: Mutex<HashMap<i64, Instant>>,
    max_failures: u32,
    window: Duration,
}

impl Default for KeyService {
    fn default() -> Self {
        Self::new(20, Duration::from_secs(60))
    }
}

impl KeyService {
    /// `max_failures` failed attempts within `window` blocks the address
    /// until the window ends.
    pub fn new(max_failures: u32, window: Duration) -> Self {
        Self {
            failures: Mutex::new(HashMap::new()),
            last_touch: Mutex::new(HashMap::new()),
            max_failures,
            window,
        }
    }

    fn is_blocked(&self, ip: IpAddr) -> bool {
        let Ok(mut map) = self.failures.lock() else {
            return false;
        };
        match map.get(&ip) {
            Some((count, start)) if start.elapsed() < self.window => *count >= self.max_failures,
            Some(_) => {
                map.remove(&ip);
                false
            }
            None => false,
        }
    }

    fn record_failure(&self, ip: IpAddr) {
        if let Ok(mut map) = self.failures.lock() {
            if map.len() > 100_000 {
                let window = self.window;
                map.retain(|_, (_, start)| start.elapsed() < window);
            }
            let entry = map.entry(ip).or_insert((0, Instant::now()));
            if entry.1.elapsed() >= self.window {
                *entry = (0, Instant::now());
            }
            entry.0 += 1;
        }
    }

    /// Checks a presented key. On success the key's last-used time is
    /// updated at most once a minute.
    pub async fn authenticate(
        &self,
        store: &Store,
        presented: Option<&str>,
        client_ip: IpAddr,
    ) -> Result<Result<ApiKeyRecord, KeyRejection>, sqlx::Error> {
        if self.is_blocked(client_ip) {
            return Ok(Err(KeyRejection::RateLimited));
        }
        let Some(secret) = presented.filter(|s| !s.is_empty()) else {
            self.record_failure(client_ip);
            return Ok(Err(KeyRejection::Missing));
        };
        let Some(key) = store.find_api_key_by_hash(&digest(secret)).await? else {
            self.record_failure(client_ip);
            return Ok(Err(KeyRejection::Unknown));
        };
        let at = now();
        let owner_inactive = match key.owner_user_id {
            Some(uid) => !store
                .get_user(uid)
                .await?
                .is_some_and(|u| u.status == crate::store::UserStatus::Active),
            None => false,
        };
        let rejection = if key.disabled || owner_inactive {
            Some(KeyRejection::Disabled)
        } else if key.is_expired(at) {
            Some(KeyRejection::Expired)
        } else if !network_allowed(&key.networks, client_ip) {
            Some(KeyRejection::Network)
        } else {
            None
        };
        if let Some(r) = rejection {
            self.record_failure(client_ip);
            return Ok(Err(r));
        }
        if self.should_touch(key.id) {
            store.touch_api_key(key.id, at).await?;
        }
        Ok(Ok(key))
    }

    fn should_touch(&self, id: i64) -> bool {
        let Ok(mut map) = self.last_touch.lock() else {
            return false;
        };
        match map.get(&id) {
            Some(t) if t.elapsed() < Duration::from_secs(60) => false,
            _ => {
                map.insert(id, Instant::now());
                true
            }
        }
    }
}

/// True if `ip` is inside one of `networks`, or `networks` is empty.
pub fn network_allowed(networks: &[String], ip: IpAddr) -> bool {
    if networks.is_empty() {
        return true;
    }
    networks.iter().any(|n| {
        n.parse::<IpNet>()
            .or_else(|_| n.parse::<IpAddr>().map(IpNet::from))
            .is_ok_and(|net| net.contains(&ip))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{NewApiKey, Role, testing};

    #[test]
    fn generated_keys_are_unique_and_unprefixed() {
        let a = generate().expect("a");
        let b = generate().expect("b");
        assert_ne!(a.secret, b.secret);
        assert_eq!(a.secret.len(), 43);
        assert_eq!(a.hash, digest(&a.secret));
        assert!(a.secret.ends_with(&a.hint));
        assert_eq!(a.hash.len(), 64);
    }

    #[test]
    fn networks() {
        let ip: IpAddr = "10.1.2.3".parse().expect("ip");
        assert!(network_allowed(&[], ip));
        assert!(network_allowed(&["10.0.0.0/8".into()], ip));
        assert!(network_allowed(&["10.1.2.3".into()], ip));
        assert!(!network_allowed(&["192.0.2.0/24".into()], ip));
        assert!(!network_allowed(&["garbage".into()], ip));
    }

    async fn store_with_key(
        networks: Vec<String>,
        expires_at: Option<i64>,
    ) -> (Store, tempfile::TempDir, GeneratedKey, i64) {
        let (s, d) = testing::sqlite().await;
        let g = generate().expect("gen");
        let id = s
            .insert_api_key(&NewApiKey {
                key_hash: g.hash.clone(),
                hint: g.hint.clone(),
                role: Some(Role::Admin),
                networks,
                expires_at,
                ..Default::default()
            })
            .await
            .expect("insert");
        (s, d, g, id)
    }

    #[tokio::test]
    async fn authenticate_paths() {
        let ip: IpAddr = "192.0.2.10".parse().expect("ip");
        let svc = KeyService::default();
        let (s, _d, g, id) = store_with_key(vec![], None).await;

        let ok = svc.authenticate(&s, Some(&g.secret), ip).await.expect("db");
        assert_eq!(ok.expect("accepted").id, id);
        assert!(
            s.get_api_key(id)
                .await
                .expect("q")
                .expect("k")
                .last_used_at
                .is_some()
        );

        assert_eq!(
            svc.authenticate(&s, None, ip).await.expect("db"),
            Err(KeyRejection::Missing)
        );
        assert_eq!(
            svc.authenticate(&s, Some("nope"), ip).await.expect("db"),
            Err(KeyRejection::Unknown)
        );
        s.set_api_key_disabled(id, true).await.expect("dis");
        assert_eq!(
            svc.authenticate(&s, Some(&g.secret), ip).await.expect("db"),
            Err(KeyRejection::Disabled)
        );
    }

    #[tokio::test]
    async fn expired_and_network_rejections() {
        let ip: IpAddr = "192.0.2.10".parse().expect("ip");
        let svc = KeyService::default();
        let (s, _d, g, _) = store_with_key(vec![], Some(1)).await;
        assert_eq!(
            svc.authenticate(&s, Some(&g.secret), ip).await.expect("db"),
            Err(KeyRejection::Expired)
        );
        let (s, _d, g, _) = store_with_key(vec!["10.0.0.0/8".into()], None).await;
        assert_eq!(
            svc.authenticate(&s, Some(&g.secret), ip).await.expect("db"),
            Err(KeyRejection::Network)
        );
        assert!(
            svc.authenticate(&s, Some(&g.secret), "10.9.9.9".parse().expect("ip"))
                .await
                .expect("db")
                .is_ok()
        );
    }

    #[tokio::test]
    async fn repeated_failures_block_the_address() {
        let ip: IpAddr = "198.51.100.1".parse().expect("ip");
        let other: IpAddr = "198.51.100.2".parse().expect("ip");
        let svc = KeyService::new(3, Duration::from_secs(60));
        let (s, _d, g, _) = store_with_key(vec![], None).await;
        for _ in 0..3 {
            assert_eq!(
                svc.authenticate(&s, Some("bad"), ip).await.expect("db"),
                Err(KeyRejection::Unknown)
            );
        }
        assert_eq!(
            svc.authenticate(&s, Some(&g.secret), ip).await.expect("db"),
            Err(KeyRejection::RateLimited)
        );
        assert!(
            svc.authenticate(&s, Some(&g.secret), other)
                .await
                .expect("db")
                .is_ok()
        );
    }

    #[tokio::test]
    async fn block_expires_after_window() {
        let ip: IpAddr = "198.51.100.3".parse().expect("ip");
        let svc = KeyService::new(1, Duration::from_millis(50));
        let (s, _d, g, _) = store_with_key(vec![], None).await;
        let _ = svc.authenticate(&s, Some("bad"), ip).await.expect("db");
        assert_eq!(
            svc.authenticate(&s, Some(&g.secret), ip).await.expect("db"),
            Err(KeyRejection::RateLimited)
        );
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(
            svc.authenticate(&s, Some(&g.secret), ip)
                .await
                .expect("db")
                .is_ok()
        );
    }
}
