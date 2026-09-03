//! Coordination record types — the single-source shape for every artifact the fleet's
//! coordination layer reads or writes: registry entries, leases, slots, messages,
//! heartbeats, escalations, sweep snapshots, drain-log rows, and `disposal.json`.
//!
//! This module (Task 1 of block OK.6.A) lays the skeleton two things every later record
//! type in this module will share:
//!
//! 1. [`COORD_STALE_TTL_SECONDS`] — one staleness constant, replacing three that already
//!    disagree across the fleet.
//! 2. [`Coord<T>`] — a typed/legacy escape hatch so a record already on disk in some older
//!    or malformed shape still deserializes (as `Legacy`) instead of erroring, which is what
//!    lets a later live-tree lint *name* an offending record rather than merely fail to parse it.

use serde::{Deserialize, Serialize};

/// Staleness threshold for a coordination record, in seconds (5400s / 90 minutes).
///
/// This fleet has accumulated three different "how old is too old" constants for what is
/// conceptually the same question — a coordination artifact's liveness window — and they
/// disagree:
///
/// - **14400s / 4h** — `core/mev/src/brain/lease.rs:35`, `availability.rs`'s own
///   `DEFAULT_TTL_SECONDS`, governing ordinary pid-keyed `.fleet-locks/*.json` entries.
/// - **10800s / 3h** — `core/mev/src/brain/lease.rs:40` (documented) and `:64`
///   (`LEASE_STALE_THRESHOLD_SECONDS`, the actual constant definition) — the value that
///   governs *lease* staleness specifically, mirroring `check_lane_agents.py`'s
///   `STALE_THRESHOLD_SECONDS`.
/// - **5400s / 90 min** — `base-template/scripts/fleet_concurrency_check.py:82`
///   (`DEFAULT_TTL_SECONDS`), governing the same ordinary `.fleet-locks/*.json` entries as
///   the 14400s value above, but from the Python side.
///
/// `COORD_STALE_TTL_SECONDS` adopts **5400s**: it is the narrowest of the three, and the
/// only one matched to a real lane segment's length rather than an arbitrary "an afternoon"
/// round number. This constant does **not** change what `mev`'s `lease.rs` or
/// `fleet_concurrency_check.py` actually use at runtime today — those two call sites are out
/// of scope for this block (see the OK.6.A block record's `out_of_scope`); this is the one
/// value new `coord` consumers should read going forward.
pub const COORD_STALE_TTL_SECONDS: u64 = 5400;

/// A coordination record that may or may not parse into its strict typed shape `T`.
///
/// Coordination artifacts on disk today were written by several different, independently
/// evolved writers (mev, the Python fleet-lock scripts, hand-authored fixtures), so a strict
/// `T`-only deserializer would error on any record that predates or deviates from the current
/// shape. `Coord<T>` instead falls back to holding the raw JSON as [`Coord::Legacy`], so a
/// caller — in particular the live-tree lint this block also ships — can walk a whole
/// directory of real records and *name* every one that didn't parse, instead of aborting on
/// the first mismatch.
///
/// `Typed` is declared first so `#[serde(untagged)]` tries the strict shape before falling
/// back to `Legacy` — untagged enums try variants in declaration order and take the first
/// one that deserializes without error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Coord<T> {
    /// The record deserialized into its strict, current shape.
    Typed(T),
    /// The record did not match the strict shape; the original JSON is preserved unchanged.
    Legacy(serde_json::Value),
}

impl<T> Coord<T> {
    /// True if this record fell back to the untyped [`Coord::Legacy`] escape.
    pub fn is_legacy(&self) -> bool {
        matches!(self, Coord::Legacy(_))
    }

    /// The strict typed value, if this record parsed into one.
    pub fn typed(&self) -> Option<&T> {
        match self {
            Coord::Typed(t) => Some(t),
            Coord::Legacy(_) => None,
        }
    }

    /// The raw JSON of a legacy record, if this record fell back to [`Coord::Legacy`] —
    /// unchanged from what was on disk, so a caller (e.g. the live-tree lint) can print or
    /// log the exact offending record.
    pub fn legacy_value(&self) -> Option<&serde_json::Value> {
        match self {
            Coord::Legacy(v) => Some(v),
            Coord::Typed(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Point {
        x: i64,
        y: i64,
    }

    #[test]
    fn ttl_constant_is_5400() {
        assert_eq!(COORD_STALE_TTL_SECONDS, 5400);
    }

    #[test]
    fn typed_value_round_trips_unchanged() {
        let point = Point { x: 1, y: 2 };
        let json = serde_json::to_string(&point).unwrap();

        let coord: Coord<Point> = serde_json::from_str(&json).unwrap();

        assert!(!coord.is_legacy());
        assert_eq!(coord.typed(), Some(&point));
        assert_eq!(coord.legacy_value(), None);

        // Round-trip: serializing the Coord back out reproduces the same JSON value.
        let re_serialized = serde_json::to_value(&coord).unwrap();
        let original_value = serde_json::to_value(&point).unwrap();
        assert_eq!(re_serialized, original_value);
    }

    #[test]
    fn unparseable_object_lands_in_legacy_byte_equal() {
        let raw = serde_json::json!({
            "totally_unexpected_field": "hello",
            "nested": { "a": 1, "b": [1, 2, 3] }
        });
        let json = raw.to_string();

        let coord: Coord<Point> = serde_json::from_str(&json).unwrap();

        assert!(coord.is_legacy());
        assert_eq!(coord.typed(), None);
        assert_eq!(coord.legacy_value(), Some(&raw));
    }

    #[test]
    fn legacy_variant_preserves_extra_fields_not_in_t() {
        // Even a value that shares field names with T, but has extras, is a case worth
        // covering: this documents that Coord<T> does not attempt partial/lossy matching —
        // it is Typed only if T's own Deserialize succeeds outright.
        let raw = serde_json::json!({ "x": 1, "y": 2, "unexpected_extra": true });
        let json = raw.to_string();

        let coord: Coord<Point> = serde_json::from_str(&json).unwrap();

        // serde_json's default struct deserialization ignores unknown fields, so this value
        // actually parses fine into Point/Typed. This test documents that behavior rather
        // than asserting Legacy, so a future #[serde(deny_unknown_fields)] change on some T
        // is a deliberate, visible decision rather than a silent behavior change here.
        assert!(!coord.is_legacy());
        assert_eq!(coord.typed(), Some(&Point { x: 1, y: 2 }));
    }
}
