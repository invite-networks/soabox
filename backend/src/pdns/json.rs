//! JSON output that matches PowerDNS byte for byte.
//!
//! PowerDNS writes `": "` between keys and values and `", "` between items.
//! Responses SOAbox rewrites (filtered lists and its own errors) use the same
//! layout so clients see identical bytes.

use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, Serializer};

/// A `serde_json` formatter with PowerDNS separators.
#[derive(Debug, Default, Clone, Copy)]
pub struct PdnsFormatter;

impl Formatter for PdnsFormatter {
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }
}

/// Serializes a value with PowerDNS separators.
pub fn to_pdns_vec<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    let mut ser = Serializer::with_formatter(&mut out, PdnsFormatter);
    // Serializing a `serde_json::Value` or plain data into a Vec cannot fail.
    if value.serialize(&mut ser).is_err() {
        out.clear();
        out.extend_from_slice(b"null");
    }
    out
}

/// Serializes a value with PowerDNS separators into a string.
pub fn to_pdns_string<T: Serialize + ?Sized>(value: &T) -> String {
    String::from_utf8(to_pdns_vec(value)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_powerdns_layout() {
        let raw = r#"[{"account": "", "id": "a.test.", "masters": [], "serial": 1}, {"id": "b.test.", "masters": ["192.0.2.1", "192.0.2.2"]}]"#;
        let v: serde_json::Value = serde_json::from_str(raw).expect("json");
        assert_eq!(to_pdns_string(&v), raw);
    }

    #[test]
    fn empty_containers() {
        let v = serde_json::json!({"a": [], "b": {}});
        assert_eq!(to_pdns_string(&v), r#"{"a": [], "b": {}}"#);
    }
}
