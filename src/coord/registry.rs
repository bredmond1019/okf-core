//! The lane-agent registry claim — one running lane's addressable identity.
//!
//! Pinned to `base-template/.claude/workflows/lane-agent.schema.json`: `agent_name`, `repo`,
//! `lane`, `roadmap`, `started_at` and `heartbeat` are required; `current_block` and
//! `block_started_at` are optional (absent on every claim written before those fields
//! existed). `started_at`/`heartbeat`/`block_started_at` are kept as `String` here rather than
//! a timestamp type — this crate has no I/O/time dependency (see `AGENTS.md` standing rule
//! 4) and the schema's own format is an ISO-8601 string, which a plain `String` already
//! round-trips byte-for-byte.
//!
//! Note the sibling divergence this module exists to make visible: this record's
//! `started_at` is an ISO-8601 **string**, while [`crate::coord::slot::SlotRecord`]'s
//! `started_at` — same field name, unrelated artifact — is an epoch **number**.

use serde::{Deserialize, Serialize};

use super::Coord;

/// A lane-agent registry claim (`lane-agent.schema.json`), wrapped in [`Coord`] so a claim
/// already on disk in some shape this module doesn't yet know about still deserializes as
/// [`Coord::Legacy`] instead of erroring.
pub type Registry = Coord<RegistryClaim>;

/// The strict, current shape of a lane-agent registry claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryClaim {
    /// The `ListAgents` nickname this agent is currently reachable at.
    pub agent_name: String,
    /// Repo slug this lane is driving, as registered in `brain.toml`.
    pub repo: String,
    /// The lane's name/slug.
    pub lane: String,
    /// Slug of the owning roadmap directory this lane runs under.
    pub roadmap: String,
    /// ISO-8601 timestamp with timezone marking when this agent claimed the lane. Fixed for
    /// the life of the claim.
    pub started_at: String,
    /// ISO-8601 timestamp with timezone, updated periodically while the agent is alive.
    pub heartbeat: String,
    /// OPTIONAL. The block id this lane is currently working on. Absent on every claim
    /// written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_block: Option<String>,
    /// OPTIONAL. ISO-8601 timestamp marking when `current_block` started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_started_at: Option<String>,
    /// OPTIONAL. Which physical/logical host wrote this record — not part of
    /// `lane-agent.schema.json` today; added uniformly across every `coord` record kind per
    /// the OK.6.A block's `what`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_claim() -> RegistryClaim {
        RegistryClaim {
            agent_name: "base-template-b6".to_string(),
            repo: "base-template".to_string(),
            lane: "types".to_string(),
            roadmap: "coordination-layer-port".to_string(),
            started_at: "2026-09-03T12:17:35Z".to_string(),
            heartbeat: "2026-09-03T13:06:26Z".to_string(),
            current_block: Some("OK.6.A".to_string()),
            block_started_at: Some("2026-09-03T12:20:00Z".to_string()),
            host: Some("brain-mini".to_string()),
        }
    }

    #[test]
    fn full_claim_round_trips() {
        let claim = full_claim();
        let json = serde_json::to_string(&claim).unwrap();
        let parsed: Registry = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&claim));

        // Re-serialize and re-parse once more: parse -> serialize -> parse == identical value.
        let re_json = serde_json::to_string(parsed.typed().unwrap()).unwrap();
        let re_parsed: Registry = serde_json::from_str(&re_json).unwrap();
        assert_eq!(re_parsed.typed(), Some(&claim));
    }

    #[test]
    fn minimal_claim_omits_optional_fields_on_round_trip() {
        let raw = serde_json::json!({
            "agent_name": "engine-rs-5b",
            "repo": "engine-rs",
            "lane": "engine-rs",
            "roadmap": "coordination-layer-port",
            "started_at": "2026-08-21T10:00:00Z",
            "heartbeat": "2026-08-21T10:05:00Z"
        });
        let claim: Registry = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(!claim.is_legacy());
        let typed = claim.typed().unwrap();
        assert_eq!(typed.current_block, None);
        assert_eq!(typed.block_started_at, None);
        assert_eq!(typed.host, None);

        // host and the block fields are absent, not null, once re-serialized.
        let value = serde_json::to_value(typed).unwrap();
        let obj = value.as_object().unwrap();
        assert!(!obj.contains_key("current_block"));
        assert!(!obj.contains_key("block_started_at"));
        assert!(!obj.contains_key("host"));
    }

    #[test]
    fn malformed_claim_missing_required_field_lands_in_legacy() {
        // Missing `heartbeat`, a required field.
        let raw = serde_json::json!({
            "agent_name": "mev-a8",
            "repo": "mev",
            "lane": "mev",
            "roadmap": "coordination-layer-port",
            "started_at": "2026-09-03T01:00:00Z"
        });
        let claim: Registry = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(claim.is_legacy());
        assert_eq!(claim.legacy_value(), Some(&raw));
    }
}
