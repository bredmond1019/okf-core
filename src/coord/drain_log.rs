//! A drain-log row — one append-only line in `planning/roadmaps/<roadmap>/drain-log.jsonl`,
//! the durable evidence base written by the sweep's drain step.
//!
//! Measured over all 232 real lines across every roadmap's `drain-log.jsonl` (2026-09-03):
//! every line carries a `record` field, and its value is one of exactly three tags —
//! `"drain"` (16 lines, a drain-cycle summary), `"receipt"` (140 lines, an inbox->processing
//! or processing->done transition for one message) or `"message"` (76 lines, a completed
//! message's own envelope alongside its filename). [`DrainLogEntry`] is an internally-tagged
//! enum on `record`; a fourth, unrecognised tag value fails to match any variant and the
//! surrounding [`Coord`] falls back to [`Coord::Legacy`] rather than erroring.

use serde::{Deserialize, Serialize};

use super::Coord;

/// A drain-log row, wrapped in [`Coord`] so a row already on disk with an unrecognised
/// `record` tag (or missing a field this module treats as required) still deserializes as
/// [`Coord::Legacy`] instead of failing outright.
pub type DrainLog = Coord<DrainLogEntry>;

/// The three drain-log row shapes, dispatched on the row's own `record` field.
///
/// Exhaustive, because it has zero consumer references today (measured 2026-09-03) — no
/// existing match for `#[non_exhaustive]` to soften into a handled default. A fourth,
/// unrecognised `record` tag is already the wrapping [`Coord`]'s degradation path (falls back
/// to `Coord::Legacy` rather than erroring), which is the orthogonal runtime contract with
/// data on disk; `#[non_exhaustive]` would only affect Rust match sites, of which there are
/// none yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum DrainLogEntry {
    /// A drain cycle's summary counts.
    Drain {
        ts: String,
        drained: i64,
        routed: i64,
        completed: i64,
        manifest_paths: i64,
        orphan_inbox: i64,
        orphan_processing: i64,
        orphan_receipts: i64,
    },
    /// One message's `from` -> `to` queue-state transition (e.g. `inbox` -> `processing`).
    Receipt {
        repo: String,
        lane: String,
        message_id: String,
        from: String,
        to: String,
        ts: String,
    },
    /// A completed message's own envelope, preserved alongside its inbox filename. The
    /// nested `message` value is a message envelope in the shape [`super::MessageRecord`]
    /// describes, but is kept as raw JSON here rather than typed as
    /// [`super::Message`] — real drain-log message rows omit `verified_by`, which
    /// `MessageRecord` requires, so typing it strictly would push every real row to
    /// `Legacy` for a field this row shape doesn't actually need.
    Message {
        repo: String,
        lane: String,
        message_id: String,
        filename: String,
        message: serde_json::Value,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_lines() -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/coord/drain-log.jsonl");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
        raw.lines().map(|l| l.to_string()).collect()
    }

    #[test]
    fn drain_row_round_trips() {
        let entry = DrainLogEntry::Drain {
            ts: "2026-09-03T03:31:11.448090+00:00".to_string(),
            drained: 2,
            routed: 0,
            completed: 2,
            manifest_paths: 0,
            orphan_inbox: 0,
            orphan_processing: 0,
            orphan_receipts: 0,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: DrainLog = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&entry));
    }

    #[test]
    fn receipt_row_round_trips() {
        let entry = DrainLogEntry::Receipt {
            repo: "base-template".to_string(),
            lane: "base-template".to_string(),
            message_id: "8ffbf483-04f5-4e39-add6-301900ad66ec".to_string(),
            from: "inbox".to_string(),
            to: "processing".to_string(),
            ts: "2026-08-23T03:01:02.809242+00:00".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: DrainLog = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&entry));
    }

    #[test]
    fn message_row_round_trips() {
        let entry = DrainLogEntry::Message {
            repo: "base-template".to_string(),
            lane: "base-template".to_string(),
            message_id: "drain-log-call-site-unwired".to_string(),
            filename: "20260823T131044Z-drain-log-call-site-unwired.json".to_string(),
            message: serde_json::json!({"kind": "FINDING", "body": "hello"}),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: DrainLog = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&entry));
    }

    #[test]
    fn unrecognised_record_tag_falls_to_legacy() {
        let raw = serde_json::json!({"record": "purge", "repo": "x"}).to_string();
        let parsed: DrainLog = serde_json::from_str(&raw).unwrap();
        assert!(parsed.is_legacy());
    }

    #[test]
    fn real_drain_log_fixture_lines_dispatch_correctly() {
        let lines = fixture_lines();
        assert_eq!(
            lines.len(),
            4,
            "expected drain, receipt, message, and one unrecognised line"
        );

        let drain: DrainLog = serde_json::from_str(&lines[0]).unwrap();
        assert!(matches!(drain.typed(), Some(DrainLogEntry::Drain { .. })));

        let receipt: DrainLog = serde_json::from_str(&lines[1]).unwrap();
        assert!(matches!(
            receipt.typed(),
            Some(DrainLogEntry::Receipt { .. })
        ));

        let message: DrainLog = serde_json::from_str(&lines[2]).unwrap();
        assert!(matches!(
            message.typed(),
            Some(DrainLogEntry::Message { .. })
        ));

        // The fourth line carries an unrecognised `record` value ("purge") — must land in
        // Legacy, not error and not silently match one of the three known variants.
        let unrecognised: DrainLog = serde_json::from_str(&lines[3]).unwrap();
        assert!(unrecognised.is_legacy());
    }
}
