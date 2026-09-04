//! A commander heartbeat — `.fleet-locks/commander-heartbeats/<name>.heartbeat`.
//!
//! Unlike every other kind in `coord`, a heartbeat file is **not a JSON object**: it holds
//! one bare scalar, and there are two live formats on this machine today —
//! an epoch integer (`agentic-portfolio-agentic-portfolio`, `bastion-test-gate`,
//! `brain-main`) and an ISO-8601 string (`brain-commander`). Because the raw file content is
//! not itself valid JSON in the ISO case (an unquoted `2026-09-03T09:12:25Z` is not a legal
//! JSON document), [`HeartbeatValue`] is parsed from the raw file text via
//! [`HeartbeatValue::parse_raw`]/[`HeartbeatValue::to_raw`] rather than through
//! `serde_json` — those two methods are this kind's round-trip contract. [`HeartbeatValue`]
//! also implements `Serialize`/`Deserialize` (as a JSON string or number) so it composes
//! normally inside a JSON *fixture*, e.g. as the `value` field of [`HeartbeatRecord`] below.
//!
//! `HeartbeatValue::parse_raw` never errors: any input that isn't a bare integer is treated
//! as the ISO-string form, which is why this kind has no `Legacy` fallback (there is no
//! input shape it can fail to place into one of its two known variants).

use serde::{Deserialize, Serialize};

/// The value found in a `.heartbeat` file — one of the two live formats on this machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HeartbeatValue {
    /// Unix epoch seconds, as written by `agentic-portfolio-agentic-portfolio`,
    /// `bastion-test-gate` and `brain-main`.
    Epoch(i64),
    /// ISO-8601 timestamp, as written by `brain-commander`. Declared second so a
    /// `#[serde(untagged)]` JSON deserialize tries the numeric form first.
    Iso(String),
}

impl HeartbeatValue {
    /// Parse the raw text of a `.heartbeat` file. Never fails: anything that isn't a bare
    /// integer is treated as an ISO-8601 string, verbatim.
    pub fn parse_raw(raw: &str) -> HeartbeatValue {
        let trimmed = raw.trim();
        match trimmed.parse::<i64>() {
            Ok(epoch) => HeartbeatValue::Epoch(epoch),
            Err(_) => HeartbeatValue::Iso(trimmed.to_string()),
        }
    }

    /// Render back to the exact text a `.heartbeat` file would hold.
    pub fn to_raw(&self) -> String {
        match self {
            HeartbeatValue::Epoch(e) => e.to_string(),
            HeartbeatValue::Iso(s) => s.clone(),
        }
    }
}

/// A heartbeat record: the raw scalar value from the `.heartbeat` file, plus an optional
/// `host` — added uniformly across every `coord` record kind per the OK.6.A block's `what`,
/// even though the on-disk `.heartbeat` file itself carries no such field (a caller that
/// knows which host wrote a given file, e.g. from its filename, fills this in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatRecord {
    /// The heartbeat's own value.
    pub value: HeartbeatValue,
    /// OPTIONAL. Which physical/logical host wrote this record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_raw_text_round_trips() {
        let raw = "1787478266";
        let value = HeartbeatValue::parse_raw(raw);
        assert_eq!(value, HeartbeatValue::Epoch(1_787_478_266));
        assert_eq!(value.to_raw(), raw);

        // parse -> serialize -> parse produces an identical value.
        let reparsed = HeartbeatValue::parse_raw(&value.to_raw());
        assert_eq!(reparsed, value);
    }

    #[test]
    fn iso_raw_text_round_trips() {
        let raw = "2026-09-03T09:12:25Z";
        let value = HeartbeatValue::parse_raw(raw);
        assert_eq!(value, HeartbeatValue::Iso(raw.to_string()));
        assert_eq!(value.to_raw(), raw);

        let reparsed = HeartbeatValue::parse_raw(&value.to_raw());
        assert_eq!(reparsed, value);
    }

    #[test]
    fn raw_text_with_surrounding_whitespace_is_trimmed() {
        assert_eq!(
            HeartbeatValue::parse_raw("  1787478266\n"),
            HeartbeatValue::Epoch(1_787_478_266)
        );
        assert_eq!(
            HeartbeatValue::parse_raw("  2026-09-03T09:12:25Z\n"),
            HeartbeatValue::Iso("2026-09-03T09:12:25Z".to_string())
        );
    }

    #[test]
    fn record_with_host_round_trips_through_json() {
        let record = HeartbeatRecord {
            value: HeartbeatValue::Iso("2026-09-03T09:12:25Z".to_string()),
            host: Some("brain-commander".to_string()),
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: HeartbeatRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, record);
    }

    #[test]
    fn record_without_host_omits_it_on_round_trip() {
        let record = HeartbeatRecord {
            value: HeartbeatValue::Epoch(1_787_900_662),
            host: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(!value.as_object().unwrap().contains_key("host"));

        let parsed: HeartbeatRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, record);
    }
}
