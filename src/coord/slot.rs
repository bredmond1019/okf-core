//! A fleet-concurrency heavy-lane slot — one active heavy lane's registration in the
//! `MAX_HEAVY_LANES` advisory lock registry.
//!
//! This kind has no JSON Schema; it is derived directly from
//! `base-template/scripts/fleet_concurrency_check.py`'s `register()`, which writes
//! `{repo, pid, pid_source, agent, category, started_at}` to one small JSON file per active
//! heavy lane (`.fleet-locks/*.json`).
//!
//! Note the sibling divergence this module exists to make visible: this record's
//! `started_at` is an **epoch number** (`time.time()`, a float count of seconds since the
//! Unix epoch), while [`crate::coord::registry::RegistryClaim`]'s `started_at` — same field
//! name, unrelated artifact — is an **ISO-8601 string**.

use serde::{Deserialize, Serialize};

use super::Coord;

/// A fleet-concurrency slot record, wrapped in [`Coord`] so a record already on disk in some
/// shape this module doesn't yet know about still deserializes as [`Coord::Legacy`].
pub type Slot = Coord<SlotRecord>;

/// Why an entry's `pid` should (or should not) be trusted as a liveness signal.
///
/// `"self"` is the Python source's literal JSON string for the default case; it collides
/// with the Rust keyword `self`, so the variant is named [`PidSource::OwnProcess`] and
/// mapped onto the wire value with `#[serde(rename = "self")]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PidSource {
    /// No `--pid` was supplied, or the supplied pid is the writer's own short-lived process —
    /// relies solely on TTL expiry plus an explicit release.
    #[serde(rename = "self")]
    OwnProcess,
    /// The caller passed a `--pid` that is not the writer process's own — a real,
    /// potentially long-lived process the caller vouches for.
    #[serde(rename = "explicit")]
    Explicit,
}

/// The strict, current shape of a fleet-concurrency slot record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotRecord {
    /// Repo slug this heavy lane is registered against.
    pub repo: String,
    /// The pid recorded for this entry (the writer's own pid, or an explicitly supplied one).
    pub pid: i64,
    /// Why `pid` should (or should not) be trusted as a liveness signal.
    pub pid_source: PidSource,
    /// OPTIONAL. The requester's own identity, when supplied — `None` for an old-scheme,
    /// pid-keyed entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The heavy-lane category this slot counts against (e.g. `"browser-automation"`).
    pub category: String,
    /// Epoch seconds (`time.time()`) marking when this slot was registered or last
    /// heartbeat-refreshed. NOTE: an epoch **number**, unlike
    /// [`crate::coord::registry::RegistryClaim::started_at`], which is an ISO-8601 string.
    pub started_at: f64,
    /// OPTIONAL. Which physical/logical host wrote this record — not part of the Python
    /// writer's shape today; added uniformly across every `coord` record kind per the
    /// OK.6.A block's `what`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_slot() -> SlotRecord {
        SlotRecord {
            repo: "price-scout".to_string(),
            pid: 42,
            pid_source: PidSource::Explicit,
            agent: Some("price-scout-a1".to_string()),
            category: "browser-automation".to_string(),
            started_at: 1_787_478_266.123,
            host: Some("brain-mini".to_string()),
        }
    }

    #[test]
    fn full_slot_round_trips() {
        let slot = full_slot();
        let json = serde_json::to_string(&slot).unwrap();
        let parsed: Slot = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&slot));
    }

    #[test]
    fn pid_source_wire_values_match_the_python_writer() {
        assert_eq!(
            serde_json::to_string(&PidSource::OwnProcess).unwrap(),
            "\"self\""
        );
        assert_eq!(
            serde_json::to_string(&PidSource::Explicit).unwrap(),
            "\"explicit\""
        );
    }

    #[test]
    fn null_agent_and_absent_host_round_trip_as_none() {
        // The Python writer always includes `agent`, but as `null` for an old-scheme,
        // pid-keyed entry — not omitted.
        let raw = serde_json::json!({
            "repo": "amistad",
            "pid": 99999,
            "pid_source": "self",
            "agent": null,
            "category": "browser-automation",
            "started_at": 1787900662.0
        });
        let slot: Slot = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(!slot.is_legacy());
        let typed = slot.typed().unwrap();
        assert_eq!(typed.agent, None);
        assert_eq!(typed.host, None);

        let value = serde_json::to_value(typed).unwrap();
        // Re-serializing omits `agent`/`host` rather than writing `null` — a different wire
        // shape, but an equal *value* once re-parsed, which is what the round-trip criterion
        // is about.
        let reparsed: SlotRecord = serde_json::from_value(value).unwrap();
        assert_eq!(&reparsed, typed);
    }
}
