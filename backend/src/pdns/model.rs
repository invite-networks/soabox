//! Typed views of PowerDNS objects that SOAbox reads.
//!
//! Unknown fields are kept in `extra` so values pass through untouched.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A zone as returned by `GET /zones` (no records).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub dnssec: bool,
    #[serde(default)]
    pub serial: u64,
    #[serde(default)]
    pub masters: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A zone with its records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Zone {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub dnssec: bool,
    #[serde(default)]
    pub serial: u64,
    #[serde(default)]
    pub masters: Vec<String>,
    #[serde(default)]
    pub rrsets: Vec<RrSet>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A resource record set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RrSet {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    #[serde(default)]
    pub ttl: u32,
    #[serde(default)]
    pub records: Vec<Record>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

/// One record in an RRset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Record {
    pub content: String,
    #[serde(default)]
    pub disabled: bool,
}

/// A comment attached to an RRset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comment {
    pub content: String,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub modified_at: u64,
}
