//! A `disposal.json` file — the output of `/dispose-run`, filing each row of a pattern
//! analysis as a block, a carryover entry, an operator edge, or explicitly nothing.
//!
//! Authored against the two real files on disk — `disposal-2026-09-02.json` (11 rows) and
//! `disposal-2026-09-03.json` (14 rows), under
//! `planning/open-work/orchestration-runs/retros/` — not against the spec prose alone,
//! per this block's correction C12: the spec prose omits `rationale` and `already_filed`,
//! both of which are present on every real row, and both are modeled here.
//!
//! **Name collision, avoided on purpose:** [`crate::DisposalReason`] already exists in
//! `state.rs` and is about *carryover-archive* disposal (cleared/superseded/promoted/
//! withdrawn) — an unrelated concept. This module's types are named [`DisposalFile`],
//! [`DisposalRow`] and [`DisposalRoute`] so the two never collide in `lib.rs`'s re-export
//! list.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

use super::Coord;

/// A `disposal.json` file, wrapped in [`Coord`] so a file already on disk in some shape
/// this module doesn't yet know about still deserializes as [`Coord::Legacy`] instead of
/// failing outright.
///
/// **Not** a plain `Coord<DisposalFile>` type alias: `route` is a closed vocabulary this
/// module DOES type ([`DisposalRoute`]), and a value outside it must be REFUSED with a hard
/// parse error, not swallowed into `Legacy` the way an unrecognized shape is. `Coord<T>`'s
/// `#[serde(untagged)]` derive can't tell those two failure causes apart on its own — any
/// error deserializing `T` just falls through to `Legacy` — so this wrapper inspects the raw
/// JSON for an out-of-enum `route` before ever attempting the generic `Coord` parse. (OK.6.A
/// task 4.)
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Disposal(Coord<DisposalFile>);

impl Disposal {
    /// True if this record fell back to the untyped [`Coord::Legacy`] escape.
    pub fn is_legacy(&self) -> bool {
        self.0.is_legacy()
    }

    /// The strict typed value, if this record parsed into one.
    pub fn typed(&self) -> Option<&DisposalFile> {
        self.0.typed()
    }

    /// The raw JSON of a legacy record, if this record fell back to [`Coord::Legacy`].
    pub fn legacy_value(&self) -> Option<&serde_json::Value> {
        self.0.legacy_value()
    }
}

/// The `route` values [`DisposalRoute`] accepts on the wire — kept alongside the enum so the
/// pre-parse refusal check in [`Disposal`]'s `Deserialize` impl and the enum's own
/// `#[serde(rename_all)]` can never quietly drift apart from each other.
const VALID_DISPOSAL_ROUTES: [&str; 5] = ["block", "operator", "none", "carryover", "chore"];

/// Finds the first row (if any) whose `route` is a string outside [`VALID_DISPOSAL_ROUTES`].
/// Returns `None` when `rows` is absent, not an array, or every row's `route` is either
/// missing, not a string, or in the closed vocabulary — those cases are left to the generic
/// [`Coord`] parse (and its `Legacy` fallback) to sort out.
fn first_out_of_enum_route(value: &serde_json::Value) -> Option<&str> {
    value.get("rows")?.as_array()?.iter().find_map(|row| {
        let route = row.get("route")?.as_str()?;
        (!VALID_DISPOSAL_ROUTES.contains(&route)).then_some(route)
    })
}

impl<'de> Deserialize<'de> for Disposal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(bad_route) = first_out_of_enum_route(&value) {
            return Err(de::Error::custom(format!(
                "disposal row `route` {bad_route:?} is not one of the closed DisposalRoute \
                 values {VALID_DISPOSAL_ROUTES:?}"
            )));
        }
        let coord: Coord<DisposalFile> =
            serde_json::from_value(value).map_err(de::Error::custom)?;
        Ok(Disposal(coord))
    }
}

/// The strict, current shape of a `disposal.json` file's top level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisposalFile {
    /// Path to the pattern-analysis markdown this disposal file was generated from.
    pub analysis: String,
    /// ISO-8601 timestamp this file was generated.
    pub generated: String,
    /// The roadmaps this disposal run covered.
    pub roadmaps: Vec<String>,
    /// Whether this file's rows were backfilled from a prior run rather than freshly
    /// generated.
    pub backfilled: bool,
    /// Present on the 2026-09-02 file, absent on 2026-09-03's — a free-text note explaining
    /// a backfill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backfill_note: Option<String>,
    /// Present on the 2026-09-03 file, absent on 2026-09-02's — a free-text note on how
    /// this run reconciles with rows lanes already filed themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation_note: Option<String>,
    /// File-level conventions governing how to read every row (e.g.
    /// `ungrounded_excludes`). Kept as raw JSON: its own keys have already varied between
    /// the two real files on disk, and no row-typed guarantee depends on this section's
    /// shape.
    #[serde(default)]
    pub conventions: serde_json::Value,
    /// The disposal rows themselves.
    pub rows: Vec<DisposalRow>,
}

/// One row of a `disposal.json` file — one mechanism's filing decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisposalRow {
    pub finding_id: String,
    pub mechanism: String,
    /// Where this finding was filed. A closed vocabulary — see [`DisposalRoute`].
    pub route: DisposalRoute,
    pub owner_repo: String,
    /// What kind of work closes this finding (e.g. `"code"`, `"docs"`, `"dedupe"`,
    /// `"operator"`). Free text on disk today, not yet a closed vocabulary the way `route`
    /// is.
    pub needs: String,
    /// `"P0"` / `"P1"` / `"P2"` on every real row observed; kept as free text rather than a
    /// closed enum since this task's acceptance criteria only require `route` to refuse an
    /// out-of-enum value.
    pub severity: String,
    pub breadth: DisposalBreadth,
    pub evidence: Vec<String>,
    /// The row's own payload — a spec-block record, a carryover entry, or an operator-edge
    /// shape, depending on `route`. Kept as raw JSON: its shape is route-dependent and this
    /// task's scope is the row envelope, not exhaustively typing every route's payload.
    #[serde(default)]
    pub payload: serde_json::Value,
    /// Set when this finding was already filed by the lane that hit it, independent of this
    /// disposal run. Absent (not merely null) on a row no lane has already filed — observed
    /// on every row of the 2026-09-02 file and most of 2026-09-03's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub already_filed: Option<String>,
    /// Every field this row's evidence cannot support — required and non-empty on every
    /// real row observed, governed by the file-level `conventions.ungrounded_excludes`.
    pub ungrounded: Vec<String>,
    pub rationale: String,
}

/// `breadth` on a disposal row: how many repos and instances the mechanism was measured
/// across.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisposalBreadth {
    pub repos: i64,
    /// `null` on several real rows (a mechanism counted by repo, not by instance) — kept
    /// `Option` rather than defaulting to zero, since zero and "not counted" are different
    /// claims. Real rows carry this key explicitly as `null` rather than omitting it, so
    /// (unlike this module's other optional fields) it is NOT `skip_serializing_if`: a
    /// round trip must reproduce the explicit `"instances": null`, not drop the key.
    #[serde(default)]
    pub instances: Option<i64>,
}

/// Where a disposal row was filed. Closed over the five values observed across both real
/// files (`block`, `operator`, `none`, `carryover`, `chore`); anything else must be refused
/// rather than silently accepted, since `route` decides which container downstream tooling
/// writes the row into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisposalRoute {
    Block,
    Operator,
    /// Explicitly filed nowhere — the analysis found nothing actionable for this
    /// mechanism.
    None,
    Carryover,
    Chore,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> DisposalRow {
        DisposalRow {
            finding_id: "sample-finding".to_string(),
            mechanism: "M1".to_string(),
            route: DisposalRoute::Carryover,
            owner_repo: "okf-core".to_string(),
            needs: "code".to_string(),
            severity: "P1".to_string(),
            breadth: DisposalBreadth {
                repos: 2,
                instances: None,
            },
            evidence: vec!["some/path.md:12".to_string()],
            payload: serde_json::json!({"origin": {"type": "mechanism", "slug": "sample-finding"}}),
            already_filed: None,
            ungrounded: vec!["origin".to_string()],
            rationale: "Sample rationale.".to_string(),
        }
    }

    fn sample_file() -> DisposalFile {
        DisposalFile {
            analysis: "planning/pattern-analysis.md".to_string(),
            generated: "2026-09-03T00:00:00Z".to_string(),
            roadmaps: vec!["coordination-layer-port".to_string()],
            backfilled: false,
            backfill_note: None,
            reconciliation_note: Some("nothing merged here".to_string()),
            conventions: serde_json::json!({"ungrounded_excludes": ["id"]}),
            rows: vec![sample_row()],
        }
    }

    #[test]
    fn sample_file_round_trips() {
        let file = sample_file();
        let json = serde_json::to_string(&file).unwrap();
        let disposal: Disposal = serde_json::from_str(&json).unwrap();
        assert!(!disposal.is_legacy());
        assert_eq!(disposal.typed(), Some(&file));
    }

    #[test]
    fn route_rejects_values_outside_the_enum() {
        let mut value = serde_json::to_value(sample_file()).unwrap();
        value["rows"][0]["route"] = serde_json::json!("bogus_route");
        let result: Result<Disposal, _> = serde_json::from_value(value);
        // An out-of-enum `route` is a closed vocabulary we DO type, so it must be a hard
        // parse error — never accepted, and never swallowed into Legacy (OK.6.A task 4).
        assert!(
            result.is_err(),
            "expected a bogus route to be refused, got: {result:?}"
        );
    }

    #[test]
    fn route_none_variant_serializes_as_lowercase_none() {
        assert_eq!(
            serde_json::to_string(&DisposalRoute::None).unwrap(),
            "\"none\""
        );
    }

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/coord")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"))
    }

    fn assert_real_file_round_trips_losslessly(name: &str, expected_rows: usize) {
        let raw = fixture(name);
        let original: serde_json::Value = serde_json::from_str(&raw).unwrap();

        let disposal: Disposal = serde_json::from_str(&raw).unwrap();
        assert!(!disposal.is_legacy(), "{name}: expected Typed, got Legacy");
        let file = disposal.typed().unwrap();
        assert_eq!(file.rows.len(), expected_rows);

        // Lossless: re-serializing the typed value reproduces the same JSON value as the
        // original file (serde_json::Value equality ignores key order).
        let round_tripped = serde_json::to_value(&disposal).unwrap();
        assert_eq!(round_tripped, original, "{name}: round trip lost data");
    }

    #[test]
    fn disposal_2026_09_02_round_trips_losslessly() {
        assert_real_file_round_trips_losslessly("disposal-2026-09-02.json", 11);
    }

    #[test]
    fn disposal_2026_09_03_round_trips_losslessly() {
        // This file also carries a row with an explicit `already_filed` string and rows
        // whose `breadth.instances` is null (both asserted below).
        assert_real_file_round_trips_losslessly("disposal-2026-09-03.json", 14);

        let raw = fixture("disposal-2026-09-03.json");
        let disposal: Disposal = serde_json::from_str(&raw).unwrap();
        let file = disposal.typed().unwrap();

        assert!(
            file.rows.iter().any(|r| r.already_filed.is_some()),
            "expected at least one row with already_filed set"
        );
        assert!(
            file.rows.iter().any(|r| r.breadth.instances.is_none()),
            "expected at least one row with a null breadth.instances"
        );
        assert!(
            file.rows.iter().all(|r| !r.rationale.is_empty()),
            "every row must carry a non-empty rationale"
        );
    }
}
