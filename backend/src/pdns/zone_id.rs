//! Zone names and the zone ids PowerDNS uses in URLs.
//!
//! PowerDNS accepts a zone id in several spellings: with or without the
//! trailing dot, in any letter case, with `/` and other characters escaped as
//! `=XX`, percent-encoded, and with a `..variant` suffix for views. SOAbox
//! reduces every spelling to one canonical zone name so scope checks cannot be
//! bypassed by writing the same zone differently.

use std::fmt;

use percent_encoding::percent_decode_str;

/// A canonical zone name: lowercase, unescaped, with one trailing dot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ZoneName(String);

impl ZoneName {
    /// Canonicalizes a zone name as written in a JSON body (`"name"`).
    pub fn from_name(name: &str) -> Option<Self> {
        let trimmed = name.trim().trim_end_matches('.');
        if trimmed.is_empty() && !name.trim().starts_with('.') {
            return None;
        }
        let lowered = trimmed.to_ascii_lowercase();
        Some(Self(format!("{lowered}.")))
    }

    /// Canonicalizes a zone id taken from a URL path segment.
    ///
    /// Returns the zone name and the view variant, if any.
    pub fn from_url_id(segment: &str) -> Option<(Self, Option<String>)> {
        let decoded = percent_decode_str(segment).decode_utf8().ok()?;
        let unescaped = unescape_equals(&decoded)?;
        let (zone, variant) = match unescaped.find("..") {
            Some(pos) => {
                let variant = &unescaped[pos + 2..];
                let v = (!variant.is_empty()).then(|| variant.to_ascii_lowercase());
                (&unescaped[..=pos], v)
            }
            None => (unescaped.as_str(), None),
        };
        Some((Self::from_name(zone)?, variant))
    }

    /// The canonical name, for example `example.com.`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The id PowerDNS uses in URLs for this zone, with `/` escaped as `=2F`.
    pub fn to_url_id(&self) -> String {
        let mut out = String::with_capacity(self.0.len());
        for c in self.0.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => out.push(c),
                other => {
                    let mut buf = [0u8; 4];
                    for b in other.encode_utf8(&mut buf).bytes() {
                        out.push_str(&format!("={b:02X}"));
                    }
                }
            }
        }
        out
    }

    /// True if `name` (a record owner name) is this zone or below it.
    pub fn contains_name(&self, name: &str) -> bool {
        let Some(n) = Self::from_name(name) else {
            return false;
        };
        n.0 == self.0 || n.0.ends_with(&format!(".{}", self.0))
    }
}

impl fmt::Display for ZoneName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn unescape_equals(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("example.com.", "example.com.", None)]
    #[case("example.com", "example.com.", None)]
    #[case("EXAMPLE.Com.", "example.com.", None)]
    #[case("example.com..", "example.com.", None)]
    #[case("example.com..internal", "example.com.", Some("internal"))]
    #[case("example.com.%2E", "example.com.", None)]
    #[case("0=2F26.2.0.192.in-addr.arpa.", "0/26.2.0.192.in-addr.arpa.", None)]
    #[case("0%2F26.2.0.192.in-addr.arpa.", "0/26.2.0.192.in-addr.arpa.", None)]
    #[case("0%3D2F26.2.0.192.in-addr.arpa.", "0/26.2.0.192.in-addr.arpa.", None)]
    fn url_ids_canonicalize(
        #[case] input: &str,
        #[case] name: &str,
        #[case] variant: Option<&str>,
    ) {
        let (z, v) = ZoneName::from_url_id(input).expect("valid");
        assert_eq!(z.as_str(), name);
        assert_eq!(v.as_deref(), variant);
    }

    #[test]
    fn rejects_bad_escapes_and_empty() {
        assert!(ZoneName::from_url_id("bad=zz.").is_none());
        assert!(ZoneName::from_url_id("bad=2").is_none());
        assert!(ZoneName::from_name("").is_none());
    }

    #[test]
    fn root_zone() {
        assert_eq!(ZoneName::from_name(".").expect("root").as_str(), ".");
    }

    #[test]
    fn url_id_round_trip() {
        let z = ZoneName::from_name("0/26.2.0.192.in-addr.arpa.").expect("valid");
        assert_eq!(z.to_url_id(), "0=2F26.2.0.192.in-addr.arpa.");
        let (back, _) = ZoneName::from_url_id(&z.to_url_id()).expect("valid");
        assert_eq!(back, z);
    }

    #[test]
    fn contains_names_below_zone_only() {
        let z = ZoneName::from_name("example.com").expect("valid");
        assert!(z.contains_name("www.example.com."));
        assert!(z.contains_name("EXAMPLE.COM"));
        assert!(!z.contains_name("badexample.com."));
    }
}
