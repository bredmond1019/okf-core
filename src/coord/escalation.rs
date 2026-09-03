//! An escalation line — one row of `planning/roadmaps/*/escalations.jsonl`.
//!
//! This kind has no JSON Schema and two disjoint families on disk. Measured over all 34
//! lines across all seven roadmaps: 24 carry `ts` and no `channel` (an older family: `repo`,
//! `lane`, `kind`, `summary`, plus a long tail of loosely-shared optional fields — `block`,
//! `agent`, `status`, `severity`, `resolution`, `owner`, `note`, `notes_ref`, `decision`,
//! `decided_by`, `needs_operator`, `operator_question`, `subject`, `detail`,
//! `verified_by` — no two rows share the same optional-field set); 10 carry `ts_utc` plus
//! `channel`, `gate_id`, `durable_home`, `verified_at_sha` and `verified_by`, all required in
//! every one of the 10, alongside `repo`, `lane`, `kind`, `severity` and `summary` — with
//! `block` and `clears_when` optional. [`EscalationRecord`] types the CURRENT (10-line)
//! family strictly; the 24 older lines land in [`crate::coord::Coord::Legacy`], not an error.

use serde::{Deserialize, Serialize};

use super::Coord;

/// An escalation line, wrapped in [`Coord`] so the 24 older-family lines on disk today —
/// and any future shape this module doesn't yet know about — deserialize as
/// [`Coord::Legacy`] instead of erroring.
pub type Escalation = Coord<EscalationRecord>;

/// Where the item an escalation is about was already, or is about to be, written to disk.
///
/// Distinct from [`crate::coord::message::MessageDurableHome`]: an escalation's `channel` is
/// a free-form locator observed as e.g. `"session:jynx"` or `"session:BT.2.A"`, not
/// `message.schema.json`'s closed four-value enum — a different artifact with a
/// coincidentally identical field name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscalationDurableHome {
    /// A free-form channel locator (e.g. `"session:jynx"`), not a closed vocabulary.
    pub channel: String,
    /// A locator within that channel precise enough for a receiver to go find the durable
    /// item itself.
    #[serde(rename = "ref")]
    pub reference: String,
}

/// The strict, current (10-line-family) shape of an escalation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscalationRecord {
    /// ISO-8601 UTC timestamp. Named `ts_utc` in the current family, distinguishing it from
    /// the older family's bare `ts` (which is not always UTC and lands in `Legacy`).
    pub ts_utc: String,
    /// Repo slug this escalation concerns.
    pub repo: String,
    /// The lane raising this escalation.
    pub lane: String,
    /// Free-form escalation kind (e.g. `"disagreement"`, `"bail"`) — no closed vocabulary
    /// observed across the corpus.
    pub kind: String,
    /// Free-form severity (e.g. `"blocking"`) — no closed vocabulary observed.
    pub severity: String,
    /// Summary of the escalation.
    pub summary: String,
    /// Free-form channel this escalation's own durable home lives under.
    pub channel: String,
    /// The gate/block id this escalation concerns (e.g.
    /// `"engine-comparison/jynx/JX.3.B"`).
    pub gate_id: String,
    /// Where the durable record behind this escalation already lives.
    pub durable_home: EscalationDurableHome,
    /// The git sha this escalation's `verified_by` evidence was captured against.
    pub verified_at_sha: String,
    /// Evidence backing this escalation's claim.
    pub verified_by: String,
    /// OPTIONAL. The block id this escalation concerns, when narrower than `gate_id` alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    /// OPTIONAL. Prose (or a structured predicate, as free text) naming what would clear
    /// this escalation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clears_when: Option<String>,
    /// OPTIONAL. Which physical/logical host wrote this record — not observed on disk today;
    /// added uniformly across every `coord` record kind per the OK.6.A block's `what`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_family_record() -> EscalationRecord {
        EscalationRecord {
            ts_utc: "2026-08-31T02:55:00Z".to_string(),
            repo: "jynx".to_string(),
            lane: "jynx".to_string(),
            kind: "disagreement".to_string(),
            severity: "blocking".to_string(),
            summary: "JX.3.B bailed at task 3/7 after 3 attempts on ONE failing test.".to_string(),
            channel: "session:jynx".to_string(),
            gate_id: "engine-comparison/jynx/JX.3.B".to_string(),
            durable_home: EscalationDurableHome {
                channel: "session:jynx".to_string(),
                reference:
                    "jynx/planning/orchestration-run/engine-comparison/notes.md#session-jynx-7a"
                        .to_string(),
            },
            verified_at_sha: "7dc6f32".to_string(),
            verified_by: "cd core/jynx && cargo nextest run --workspace --all-features\n  1 failed"
                .to_string(),
            block: Some("JX.3.B".to_string()),
            clears_when: None,
            host: None,
        }
    }

    #[test]
    fn current_family_round_trips_as_typed() {
        let record = current_family_record();
        let json = serde_json::to_string(&record).unwrap();
        let parsed: Escalation = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&record));

        let re_json = serde_json::to_string(parsed.typed().unwrap()).unwrap();
        let re_parsed: Escalation = serde_json::from_str(&re_json).unwrap();
        assert_eq!(re_parsed.typed(), Some(&record));
    }

    #[test]
    fn host_and_optional_fields_omitted_when_absent() {
        let record = current_family_record();
        let value = serde_json::to_value(&record).unwrap();
        let obj = value.as_object().unwrap();
        assert!(!obj.contains_key("clears_when"));
        assert!(!obj.contains_key("host"));
    }

    /// A representative sample of the 24 older-family lines, measured directly from
    /// `planning/roadmaps/context-handling-between-nodes/escalations.jsonl` on this machine —
    /// `ts` (not `ts_utc`) and no `channel`, `gate_id`, `durable_home` or `verified_at_sha`.
    #[test]
    fn representative_older_family_line_lands_in_legacy() {
        let raw = serde_json::json!({
            "ts": "2026-09-01T23:05:00-03:00",
            "roadmap": "context-handling-between-nodes",
            "lane": "bastion",
            "repo": "bastion",
            "agent": "bastion-91",
            "kind": "cross-repo-edit",
            "block": null,
            "summary": "engine-rs-31 holds an exclusive lease on repo bastion; lane bastion cannot register (exit 3) and no block can start.",
            "status": "open"
        });
        let escalation: Escalation = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(escalation.is_legacy());
        assert_eq!(escalation.legacy_value(), Some(&raw));
    }

    /// A second older-family line with an entirely different optional-field set (`resolution`
    /// instead of `status`/`agent`/`block`), confirming the Legacy escape isn't accidentally
    /// keyed to one specific shape of the older family.
    #[test]
    fn a_second_older_family_shape_also_lands_in_legacy() {
        let raw = serde_json::json!({
            "ts": "2026-09-03T01:20:00Z",
            "roadmap": "context-handling-between-nodes",
            "lane": "mev",
            "repo": "mev",
            "agent": "mev-a8",
            "kind": "bail",
            "block": "MV.ticket.emit-state-write-is-corpus-wide-and-unscoped",
            "note": "spec-authoring defect, corrected in place",
            "resolution": "respec-and-resume"
        });
        let escalation: Escalation = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(escalation.is_legacy());
    }
}
