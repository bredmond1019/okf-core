//! A repo lease — a claim on a repo's working tree, taken before any commit and released at
//! block boundaries.
//!
//! Pinned to `base-template/.claude/workflows/lease.schema.json`: `repo`, `lane`, `agent`,
//! `acquired_at` and `kind` are required; `heartbeat` and `scope` are optional (absent on
//! every lease on disk as of the schema's `heartbeat` addition, per the schema's own
//! description).

use serde::{Deserialize, Serialize};

use super::Coord;

/// A repo lease (`lease.schema.json`), wrapped in [`Coord`] so a lease already on disk in
/// some shape this module doesn't yet know about still deserializes as [`Coord::Legacy`].
pub type Lease = Coord<LeaseRecord>;

/// Whether a lease excludes other claimants.
///
/// Two `Shared` leases on the same repo are legal; two `Exclusive` leases on the same repo
/// are not — `Exclusive` means this lane is about to write to the tree and no one else may
/// hold any lease on it at the same time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseKind {
    Exclusive,
    Shared,
}

/// How far an `Exclusive` lease's refusal reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseScope {
    /// The lease only refuses a register naming this same `repo`. The default when the field
    /// is absent.
    Repo,
    /// The whole fleet is quiesced: the lease refuses every register, in any repo.
    Fleet,
}

/// The strict, current shape of a repo lease.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaseRecord {
    /// Repo slug whose working tree this lease covers, as registered in `brain.toml`.
    pub repo: String,
    /// The lane holding this lease.
    pub lane: String,
    /// The `ListAgents` nickname currently holding this lease.
    pub agent: String,
    /// ISO-8601 timestamp with timezone marking when this lease was taken. IMMUTABLE — set
    /// once, at acquisition, and never re-stamped.
    pub acquired_at: String,
    /// Whether this lease excludes other claimants.
    pub kind: LeaseKind,
    /// OPTIONAL. ISO-8601 timestamp, updated periodically while the lease is held. When
    /// absent, staleness falls back to `acquired_at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<String>,
    /// OPTIONAL. How far an `exclusive` lease's refusal reaches. Absent means `Repo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<LeaseScope>,
    /// OPTIONAL. Which physical/logical host wrote this record — added uniformly across
    /// every `coord` record kind per the OK.6.A block's `what`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_lease() -> LeaseRecord {
        LeaseRecord {
            repo: "bastion".to_string(),
            lane: "bastion".to_string(),
            agent: "bastion-91".to_string(),
            acquired_at: "2026-09-01T12:17:35Z".to_string(),
            kind: LeaseKind::Exclusive,
            heartbeat: Some("2026-09-01T13:06:26Z".to_string()),
            scope: Some(LeaseScope::Fleet),
            host: Some("brain-mini".to_string()),
        }
    }

    #[test]
    fn full_lease_round_trips() {
        let lease = full_lease();
        let json = serde_json::to_string(&lease).unwrap();
        let parsed: Lease = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&lease));
    }

    #[test]
    fn minimal_lease_omits_optional_fields() {
        let raw = serde_json::json!({
            "repo": "engine-rs",
            "lane": "engine-rs",
            "agent": "engine-rs-31",
            "acquired_at": "2026-09-01T23:00:00Z",
            "kind": "shared"
        });
        let lease: Lease = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(!lease.is_legacy());
        let typed = lease.typed().unwrap();
        assert_eq!(typed.kind, LeaseKind::Shared);
        assert_eq!(typed.heartbeat, None);
        assert_eq!(typed.scope, None);
        assert_eq!(typed.host, None);

        let value = serde_json::to_value(typed).unwrap();
        let obj = value.as_object().unwrap();
        assert!(!obj.contains_key("heartbeat"));
        assert!(!obj.contains_key("scope"));
        assert!(!obj.contains_key("host"));
    }

    #[test]
    fn kind_rejects_values_outside_the_enum() {
        let raw = serde_json::json!({
            "repo": "engine-rs",
            "lane": "engine-rs",
            "agent": "engine-rs-31",
            "acquired_at": "2026-09-01T23:00:00Z",
            "kind": "read-only"
        });
        // Falls to Legacy rather than a hard parse error, because Coord<T> is untagged —
        // but it must NOT land in Typed with a bogus kind.
        let lease: Lease = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(lease.is_legacy());
    }
}
