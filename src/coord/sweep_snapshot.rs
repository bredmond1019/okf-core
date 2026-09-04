//! A fleet-wide "sweep" snapshot — one JSON file per sweep, written under
//! `planning/roadmaps/<roadmap>/sweeps/<ts>.json` by the sweep tooling that walks every
//! lane's discovered state.
//!
//! Pinned to the real files under `planning/roadmaps/autonomous-foundation/sweeps/*.json`
//! (there are dozens; this module was authored against a representative sample). A real
//! sweep file carries far more than what this module models — per-lane `leases`,
//! `lane_registry`, `carryover`, `operator_gates` and `message_queue` sections, and
//! top-level `escalations`/`routed`/`diff` sections — but this block's `what` scopes the
//! modeled shape to the top-level identity fields and the run-record lifecycle a lane
//! reports, which is the part every consumer named in this block's `interfaces` actually
//! reads. Anything not modeled here is simply ignored by `serde`'s default
//! unknown-field behavior on the next field down, rather than causing a real file to fall
//! to [`Coord::Legacy`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Coord;

/// A sweep snapshot file, wrapped in [`Coord`] so a snapshot already on disk in some shape
/// this module doesn't yet know about (or missing a field this module treats as required)
/// still deserializes as [`Coord::Legacy`] instead of failing outright.
pub type SweepSnapshot = Coord<SweepSnapshotRecord>;

/// The strict, current shape of a sweep snapshot's top level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepSnapshotRecord {
    /// UTC timestamp the sweep ran, and the source of the file's own `<ts>.json` name.
    pub ts_utc: String,
    /// Slug of the roadmap this sweep covers.
    pub roadmap: String,
    /// The HQ git SHA the sweep ran against.
    pub git_sha: String,
    /// What the sweep discovered, per lane.
    pub discovery: SweepDiscovery,
}

/// The `discovery` section of a sweep snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepDiscovery {
    /// Slug of the roadmap this sweep covers (repeated from the top level in every real
    /// file observed).
    pub roadmap: String,
    /// Absolute path to the roadmap's directory on the machine that ran the sweep.
    pub roadmap_dir: String,
    /// Per-lane discovered state, keyed by lane/repo name.
    pub lanes: BTreeMap<String, SweepLane>,
}

/// One lane's discovered state within a sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepLane {
    /// Repo slug this lane drives.
    pub repo: String,
    /// The blocks this lane has recorded, in the sweep's own raw shape — not modeled
    /// further here (see the module-level note on scope).
    #[serde(default)]
    pub blocks: Vec<serde_json::Value>,
    /// The lane's orchestration-run lifecycle, when the sweep found one. Absent for a lane
    /// with no `notes.md`/`review.md` on disk (e.g. a lane never yet run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_record: Option<SweepRunRecord>,
}

/// A lane's `notes.md`/`review.md` lifecycle, as the sweep read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepRunRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<SweepRunRecordEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<SweepRunRecordEntry>,
}

/// One `notes.md` or `review.md` entry within a lane's run record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepRunRecordEntry {
    /// Absolute path to the file on the machine that ran the sweep.
    pub path: String,
    /// The file's lifecycle stamp (e.g. `"active"`, `"lane-complete"`) — free text on disk
    /// today, not yet a closed vocabulary.
    pub lifecycle: String,
    /// Run start date. Present even when `run_ended` is the empty string (a run still in
    /// flight when the sweep ran).
    pub run_started: String,
    /// Run end date, or the empty string `""` for a run still in flight — observed on real
    /// files, not `null`.
    pub run_ended: String,
    pub open_count: i64,
    pub held_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> SweepSnapshotRecord {
        let mut lanes = BTreeMap::new();
        lanes.insert(
            "okf-core".to_string(),
            SweepLane {
                repo: "okf-core".to_string(),
                blocks: vec![serde_json::json!({"block": "OK.6.A", "status": "in_progress"})],
                run_record: Some(SweepRunRecord {
                    notes: Some(SweepRunRecordEntry {
                        path: "/tmp/notes.md".to_string(),
                        lifecycle: "active".to_string(),
                        run_started: "2026-09-01".to_string(),
                        run_ended: String::new(),
                        open_count: 1,
                        held_count: 0,
                    }),
                    review: None,
                }),
            },
        );
        lanes.insert(
            "base-template".to_string(),
            SweepLane {
                repo: "base-template".to_string(),
                blocks: vec![],
                run_record: None,
            },
        );

        SweepSnapshotRecord {
            ts_utc: "2026-09-03T00:00:00Z".to_string(),
            roadmap: "coordination-layer-port".to_string(),
            git_sha: "deadbeef".to_string(),
            discovery: SweepDiscovery {
                roadmap: "coordination-layer-port".to_string(),
                roadmap_dir: "/tmp/roadmap".to_string(),
                lanes,
            },
        }
    }

    #[test]
    fn sample_snapshot_round_trips() {
        let record = sample_snapshot();
        let json = serde_json::to_string(&record).unwrap();
        let snapshot: SweepSnapshot = serde_json::from_str(&json).unwrap();
        assert!(!snapshot.is_legacy());
        assert_eq!(snapshot.typed(), Some(&record));
    }

    #[test]
    fn lane_with_no_run_record_omits_the_field() {
        let record = sample_snapshot();
        let json = serde_json::to_value(&record).unwrap();
        let lane = &json["discovery"]["lanes"]["base-template"];
        assert!(!lane.as_object().unwrap().contains_key("run_record"));
    }

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/coord")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"))
    }

    #[test]
    fn real_sweep_snapshot_excerpt_parses_as_typed() {
        // tests/fixtures/coord/sweep-snapshot.json is a size-trimmed excerpt of a real
        // sweep — planning/roadmaps/autonomous-foundation/sweeps/2026-08-28T01-30-33Z.json
        // — keeping two real lanes' `repo`/`blocks`/`run_record` untouched and dropping only
        // the per-lane sections this module doesn't model (leases, lane_registry,
        // carryover, operator_gates, message_queue) and all but two of each lane's real
        // `blocks` entries, to avoid checking in a ~600KB fixture for a shape this small.
        let raw = fixture("sweep-snapshot.json");
        let snapshot: SweepSnapshot = serde_json::from_str(&raw).unwrap();
        assert!(!snapshot.is_legacy(), "expected Typed, got Legacy");
        let record = snapshot.typed().unwrap();
        assert_eq!(record.roadmap, "autonomous-foundation");
        assert!(record.discovery.lanes.contains_key("base-template"));
        assert!(record.discovery.lanes.contains_key("okf-core"));
        let base_template = &record.discovery.lanes["base-template"];
        let run_record = base_template.run_record.as_ref().unwrap();
        let notes = run_record.notes.as_ref().unwrap();
        assert_eq!(notes.run_ended, "");
        assert_eq!(notes.lifecycle, "active");
    }
}
