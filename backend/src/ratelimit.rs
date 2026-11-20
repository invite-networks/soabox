//! A small in-memory limiter for failed attempts, keyed by any string.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Blocks a key after `max` failures within `window`, until the window ends.
#[derive(Debug)]
pub struct FailureLimiter {
    entries: Mutex<HashMap<String, (u32, Instant)>>,
    max: u32,
    window: Duration,
}

impl FailureLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            max,
            window,
        }
    }

    /// True if any of the keys is currently blocked.
    pub fn blocked(&self, keys: &[&str]) -> bool {
        let Ok(map) = self.entries.lock() else {
            return false;
        };
        keys.iter().any(|k| {
            map.get(*k)
                .is_some_and(|(n, start)| start.elapsed() < self.window && *n >= self.max)
        })
    }

    /// Counts a failure against every key.
    pub fn fail(&self, keys: &[&str]) {
        let Ok(mut map) = self.entries.lock() else {
            return;
        };
        if map.len() > 100_000 {
            let w = self.window;
            map.retain(|_, (_, s)| s.elapsed() < w);
        }
        for k in keys {
            let e = map.entry((*k).to_owned()).or_insert((0, Instant::now()));
            if e.1.elapsed() >= self.window {
                *e = (0, Instant::now());
            }
            e.0 += 1;
        }
    }

    /// Clears the keys after a success.
    pub fn succeed(&self, keys: &[&str]) {
        if let Ok(mut map) = self.entries.lock() {
            for k in keys {
                map.remove(*k);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_and_clears_on_success() {
        let l = FailureLimiter::new(2, Duration::from_secs(60));
        assert!(!l.blocked(&["a"]));
        l.fail(&["a", "b"]);
        l.fail(&["a"]);
        assert!(l.blocked(&["a"]));
        assert!(!l.blocked(&["b"]));
        assert!(l.blocked(&["b", "a"]));
        l.succeed(&["a"]);
        assert!(!l.blocked(&["a"]));
    }

    #[test]
    fn window_expires() {
        let l = FailureLimiter::new(1, Duration::from_millis(10));
        l.fail(&["a"]);
        assert!(l.blocked(&["a"]));
        std::thread::sleep(Duration::from_millis(20));
        assert!(!l.blocked(&["a"]));
        l.fail(&["a"]);
        assert!(l.blocked(&["a"]));
    }
}
