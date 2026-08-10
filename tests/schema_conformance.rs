// The schema/struct conformance gate (`3.1-schema-struct-conformance`).
//
// Wires up `tests/support/{schema_doc,schema_parse,struct_probe}.rs` into a
// single `cargo test` gate: every field the brain's authored
// `docs/state/state-schema.md` pipe tables document as *authored* on one of
// the four shared node types (`TrackBlock`, `Backlog`, `Epic`, `Carryover`)
// must have a corresponding field on the matching okf-core struct.
//
// This closes the silent data-loss hole that deleted authored
// `TrackBlock.note` values twice (2026-08-02, 2026-08-03): the doc always
// listed `note?`, but the struct omitted it, so every re-serialization
// silently dropped the field with nothing failing loudly. See tasks.md for
// the full defect history and the settled design decisions this test
// implements.

mod support;

use std::collections::BTreeSet;

use serde_json::{Value, json};

use okf_core::{Backlog, Carryover, Epic, TrackBlock};

use support::schema_doc::read_schema_doc;
use support::schema_floor::expected_field_names;
use support::schema_parse::{
    DocumentedField, is_derived, parse_derived_fields, parse_field_tables,
};
use support::struct_probe::{HasExtra, struct_has_field};

/// Whether a parsed section heading (which may carry trailing qualifiers,
/// e.g. `` `backlog[]` (HQ brain only — authored) ``) is the section this
/// block maps to a struct — matched on the heading's LEADING identifier,
/// after stripping a leading backtick if present (some headings wrap the
/// identifier in backticks, `Block vocabulary` does not).
fn section_matches(section: &str, leading_identifier: &str) -> bool {
    section
        .trim_start_matches('`')
        .starts_with(leading_identifier)
}

/// Minimal valid seed JSON for `TrackBlock` (needs `id` + `title`).
fn track_block_seed() -> Value {
    json!({
        "id": "OK.3.A",
        "title": "seed block",
    })
}

/// Minimal valid seed JSON for `Backlog` (needs `slug`, `title`, `repo`,
/// `type`, `status`).
fn backlog_seed() -> Value {
    json!({
        "slug": "seed-backlog-item",
        "title": "seed backlog item",
        "repo": "okf-core",
        "type": "chore",
        "status": "idea",
    })
}

/// Minimal valid seed JSON for `Epic` (needs `slug` + `title`).
fn epic_seed() -> Value {
    json!({
        "slug": "seed-epic",
        "title": "seed epic",
    })
}

/// Minimal valid seed JSON for `Carryover` (needs `slug`, `scope`, `kind`,
/// `text`, `created`).
fn carryover_seed() -> Value {
    json!({
        "slug": "seed-carryover",
        "scope": {},
        "kind": "known_issue",
        "text": "seed carryover text",
        "created": "2026-01-01",
    })
}

/// Check every documented, non-derived field in `fields` against struct `T`,
/// pushing one violation line per failure onto `violations` rather than
/// panicking on the first — a multi-field drift should surface in one run,
/// not take N runs to diagnose.
fn check_struct<T>(
    seed: Value,
    fields: &[&DocumentedField],
    derived: &BTreeSet<String>,
    struct_name: &str,
    violations: &mut Vec<String>,
) where
    T: serde::Serialize + serde::de::DeserializeOwned + HasExtra,
{
    for field in fields {
        if is_derived(&field.name, derived) {
            continue;
        }
        if !struct_has_field::<T>(seed.clone(), &field.name, &field.shape) {
            violations.push(format!(
                "documented field `{}` in section `{}` has no corresponding field on struct `{struct_name}`",
                field.name, field.section
            ));
        }
    }
}

/// Check that `fields` (the fields parsed under `section`) cover every name
/// in the canonical floor from `schema_floor::expected_field_names`, pushing
/// ONE violation naming every missing field onto `violations` rather than
/// panicking immediately — this matches `check_struct`'s
/// accumulate-don't-panic-on-first shape, so a run that narrows multiple
/// sections at once surfaces all of them, not just the first.
///
/// This is a SUBSET check, not an equality: `fields` may legitimately be a
/// strict superset of the stored names (growth needs no edit to
/// `schema_floor.rs`), but every stored name must appear among the parsed
/// names. It replaces the old count-based floor, which could not catch an
/// offsetting add/drop in the same doc edit, nor name which field a plain
/// row deletion removed. See `schema_floor.rs`'s module doc comment for the
/// full rationale.
fn check_field_name_floor(
    section: &str,
    fields: &[&DocumentedField],
    violations: &mut Vec<String>,
) {
    let observed: BTreeSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    let missing: Vec<&str> = expected_field_names(section)
        .iter()
        .copied()
        .filter(|name| !observed.contains(name))
        .collect();
    if !missing.is_empty() {
        violations.push(format!(
            "section `{section}` is missing {} documented field(s): {} (floor from \
             `schema_floor::expected_field_names`, a per-section table hand-derived from \
             `docs/state/state-schema.md`) — either the doc's `| Field | Shape | Meaning |` table \
             for this section was reformatted and rows were silently dropped by the parser (check \
             schema_parse.rs's exact header match), or the fields were deliberately removed from \
             the schema and `schema_floor.rs` needs a deliberate, reviewed edit",
            missing.len(),
            missing.join(", ")
        ));
    }
}

/// The reusable core of the gate: parse `doc` (an already-loaded schema-doc
/// string, real or fixture) and return every accumulated violation —
/// count-floor shortfalls and struct-conformance drift alike — WITHOUT
/// asserting. Factored out so the fixture-driven tests below (`floor_*`)
/// can drive the exact same logic the live gate (`schema_struct_conformance`)
/// runs, against synthetic docs, instead of duplicating it.
fn run_conformance_check(doc: &str) -> Vec<String> {
    let fields = parse_field_tables(doc);
    let derived = parse_derived_fields(doc);

    let mut violations = Vec::new();

    let block_vocabulary: Vec<&DocumentedField> = fields
        .iter()
        .filter(|f| section_matches(&f.section, "Block vocabulary"))
        .collect();
    check_field_name_floor("Block vocabulary", &block_vocabulary, &mut violations);
    check_struct::<TrackBlock>(
        track_block_seed(),
        &block_vocabulary,
        &derived,
        "TrackBlock",
        &mut violations,
    );

    let backlog_fields: Vec<&DocumentedField> = fields
        .iter()
        .filter(|f| section_matches(&f.section, "backlog[]"))
        .collect();
    check_field_name_floor("backlog[]", &backlog_fields, &mut violations);
    check_struct::<Backlog>(
        backlog_seed(),
        &backlog_fields,
        &derived,
        "Backlog",
        &mut violations,
    );

    let epic_fields: Vec<&DocumentedField> = fields
        .iter()
        .filter(|f| section_matches(&f.section, "epics[]"))
        .collect();
    check_field_name_floor("epics[]", &epic_fields, &mut violations);
    check_struct::<Epic>(epic_seed(), &epic_fields, &derived, "Epic", &mut violations);

    let carryover_fields: Vec<&DocumentedField> = fields
        .iter()
        .filter(|f| section_matches(&f.section, "carryover[]"))
        .collect();
    check_field_name_floor("carryover[]", &carryover_fields, &mut violations);
    check_struct::<Carryover>(
        carryover_seed(),
        &carryover_fields,
        &derived,
        "Carryover",
        &mut violations,
    );

    violations
}

/// The gate: every authored field documented on one of the four mapped
/// sections has a corresponding field on the matching struct.
#[test]
fn schema_struct_conformance() {
    let doc = read_schema_doc();
    let violations = run_conformance_check(&doc);

    assert!(
        violations.is_empty(),
        "schema/struct conformance drift found:\n{}",
        violations.join("\n")
    );
}

// ─── Fixture-driven load-bearing test triple (ticket-conformance-field-count-floor, task 4) ───
//
// A floor set too high and a floor set too low both pass a single-direction
// suite. These tests land the OTHER directions the red-first observation in
// task 3 (see tasks.md's Amendment Log) proved were missing:
//   - a fixture with one extra, DERIVED-exempt field must still PASS
//     (legitimate schema growth is not a failure);
//   - a fixture with zero parseable tables must still fail loudly, exactly
//     as the old `!is_empty()` asserts did;
//   - a fixture narrowing exactly one of the four mapped sections by one
//     row (parses >=1, fewer than its floor) must trip exactly one
//     violation, naming that section — repeated for ALL FOUR sections
//     rather than one representative, since building each is mechanical
//     (see `fixture_doc` below).
//
// Every fixture here is fully self-contained synthetic markdown — never a
// derivative of the brain's real `docs/state/state-schema.md` — so these
// tests stay stable across edits to that external doc. They read the real
// per-section floors from `schema_floor::expected_field_names` (never a
// re-hardcoded literal), so a future change to those floors updates these
// fixtures' row counts automatically instead of silently decoupling from
// the source of truth these tests exist to exercise.

/// The four sections the gate maps to a struct, each paired with ONE
/// struct-valid field name/shape that `check_struct` will find present on
/// the matching struct. The parser and `check_struct` do not dedupe by
/// field name, so repeating a single valid row N times isolates the NAME
/// floor from struct-conformance concerns — a fixture built this way can
/// never fail on `check_struct`, only on `check_field_name_floor`.
const FIXTURE_SECTIONS: [(&str, &str, &str); 4] = [
    ("Block vocabulary", "id", "string"),
    ("backlog[]", "slug", "string"),
    ("epics[]", "slug", "string"),
    ("carryover[]", "slug", "string"),
];

/// Build a self-contained fixture doc covering all four mapped sections,
/// each populated with exactly its real floor's row count (from
/// `schema_floor::expected_field_names`).
///
/// - `narrow`: if `Some(section_heading)`, that section gets `floor - 1`
///   rows instead of `floor` — a table whose header parses fine but has one
///   fewer row than documented, the exact "partial drop" failure mode this
///   ticket exists to catch (see `schema_parse.rs:82`'s per-row
///   conditional collection).
/// - `extra_derived`: if `Some((section_heading, extra_field_name))`, that
///   section gets ONE additional row beyond its floor, and a synthetic `##
///   Authored vs derived` table is appended marking that extra field name
///   as derived — so `check_struct` exempts it and only the name floor is
///   exercised, matching "the doc legitimately documents a new field."
fn fixture_doc(narrow: Option<&str>, extra_derived: Option<(&str, &str)>) -> String {
    let mut doc = String::new();
    let mut derived_rows = String::new();

    for (heading, field, shape) in FIXTURE_SECTIONS {
        let floor = expected_field_names(heading).len();
        let mut rows = floor;
        if narrow == Some(heading) {
            rows -= 1;
        }

        doc.push_str(&format!(
            "## {heading}\n\n| Field | Shape | Meaning |\n|---|---|---|\n"
        ));
        for _ in 0..rows {
            doc.push_str(&format!("| `{field}` | {shape} | fixture row. |\n"));
        }
        if let Some((extra_heading, extra_field)) = extra_derived
            && extra_heading == heading
        {
            doc.push_str(&format!(
                "| `{extra_field}` | string? | fixture derived extra field. |\n"
            ));
            derived_rows.push_str(&format!("| **Derived** | `{extra_field}` | fixture |\n"));
        }
        doc.push('\n');
    }

    if !derived_rows.is_empty() {
        doc.push_str(
            "## Authored vs derived\n\n| Bucket | Fields | Who writes it |\n|---|---|---|\n",
        );
        doc.push_str(&derived_rows);
    }

    doc
}

/// Every mapped section documented at exactly its floor: the gate passes.
/// This is the `>=` boundary itself — a floor that were accidentally `>`
/// would fail this.
#[test]
fn floor_fixture_at_exactly_the_floor_passes() {
    let doc = fixture_doc(None, None);
    let violations = run_conformance_check(&doc);
    assert!(
        violations.is_empty(),
        "a fixture with every mapped section at exactly its floor should pass; got: {violations:?}"
    );
}

/// Legitimate growth: a fixture documenting one EXTRA field (marked
/// derived, so struct-conformance is not in play) in `Block vocabulary`
/// must still pass. Without this, the next real schema addition turns the
/// gate red for a correct change.
#[test]
fn floor_fixture_with_extra_derived_field_passes() {
    let doc = fixture_doc(None, Some(("Block vocabulary", "extra_growth_field")));
    let violations = run_conformance_check(&doc);
    assert!(
        violations.is_empty(),
        "a fixture with one extra DOCUMENTED+DERIVED field should pass — \
         legitimate growth is not a failure; got: {violations:?}"
    );
}

/// Total parse failure: headings present, no `| Field | Shape | Meaning |`
/// table under any of the four mapped sections. Must still fail, with a
/// message at least as actionable as the old `!is_empty()` asserts — the
/// floor must not weaken the case that already worked.
#[test]
fn floor_fixture_with_zero_parseable_tables_fails_loudly() {
    let doc = "## Block vocabulary\n\nNo table here — total parser failure fixture.\n\n\
               ## backlog[]\n\nNo table here.\n\n\
               ## epics[]\n\nNo table here.\n\n\
               ## carryover[]\n\nNo table here.\n";
    let violations = run_conformance_check(doc);
    assert_eq!(
        violations.len(),
        4,
        "expected one floor violation per mapped section on total parse failure; got: {violations:?}"
    );
    for (heading, ..) in FIXTURE_SECTIONS {
        assert!(
            violations
                .iter()
                .any(|v| v.contains(&format!("section `{heading}`"))
                    && v.contains("parsed only 0 field")),
            "expected a floor violation naming `{heading}` with 0 observed fields; got: {violations:?}"
        );
    }
}

/// Narrowing is caught, repeated for each of the four mapped sections
/// (rather than justifying one representative — building each fixture is
/// mechanical via `fixture_doc`, so full coverage costs nothing extra): a
/// section with `floor - 1` rows (table header intact, one row silently
/// dropped, exactly like `parse_field_tables`'s conditional row collection
/// under a reformatted table) trips exactly one violation, naming that
/// section, while the other three sections — still at their own floor —
/// stay clean.
#[test]
fn floor_narrowing_is_caught_for_each_mapped_section() {
    for (heading, ..) in FIXTURE_SECTIONS {
        let doc = fixture_doc(Some(heading), None);
        let violations = run_conformance_check(&doc);
        assert_eq!(
            violations.len(),
            1,
            "narrowing `{heading}` by one row should trip exactly one floor \
             violation (the other three sections stay at their own floor); got: {violations:?}"
        );
        assert!(
            violations[0].contains(&format!("section `{heading}`")),
            "expected the single violation to name `{heading}`; got: {violations:?}"
        );
    }
}

/// Regression guard for design decision 3 (tasks.md): `tasks` is documented
/// in the `Block vocabulary` table, is classified `derived` in the `##
/// Authored vs derived` table, is absent from `TrackBlock`, and the
/// conformance check above nonetheless passes — a derived field with no
/// struct field is not a defect.
#[test]
fn derived_fields_are_exempt() {
    let doc = read_schema_doc();
    let fields = parse_field_tables(&doc);
    let derived = parse_derived_fields(&doc);

    let tasks_field = fields
        .iter()
        .find(|f| section_matches(&f.section, "Block vocabulary") && f.name == "tasks")
        .expect("expected `tasks` to be documented in the `Block vocabulary` pipe table");

    assert!(
        is_derived(&tasks_field.name, &derived),
        "expected `tasks` to be classified as derived"
    );

    assert!(
        !struct_has_field::<TrackBlock>(track_block_seed(), "tasks", &tasks_field.shape),
        "expected `tasks` to be absent from TrackBlock today — this is the regression guard for \
         decision 3; if this now fails because TrackBlock grew a `tasks` field, the exemption \
         is no longer being exercised and this assertion (not the exemption) should be revisited"
    );
}
