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

// ─── Fixture-driven floor tests (ticket-conformance-field-name-floor, task 3) ───
//
// The floor stores expected field NAMES and asserts they are a subset of
// the parsed names, so these fixtures are built from the real name lists in
// `schema_floor::expected_field_names` — never from a re-hardcoded literal
// and never from a row COUNT. A count-shaped fixture (N copies of one row)
// would satisfy the old floor and fail this one, which is the whole point
// of the change.
//
// Directions covered, because a floor that only ever fails and a floor that
// only ever passes both look correct under a single-direction suite:
//   - baseline: every section fully documented must PASS;
//   - one row deleted must FAIL, naming the deleted field;
//   - one row deleted while two are added (the OFFSETTING case the count
//     floor passed) must FAIL, naming the deleted field;
//   - a field added and none removed (legitimate growth) must PASS with no
//     edit to `schema_floor.rs` — this is the drift fix's regression guard;
//   - zero parseable tables must FAIL loudly for all four sections.
//
// Every fixture marks all of its emitted fields DERIVED in a synthetic
// `## Authored vs derived` table. That makes `check_struct` a no-op for
// them, so a fixture can only ever fail on `check_field_name_floor` and
// these tests stay isolated from struct-conformance concerns.

/// The four sections the gate maps to a struct.
const FIXTURE_SECTIONS: [&str; 4] = ["Block vocabulary", "backlog[]", "epics[]", "carryover[]"];

/// Build a self-contained fixture doc covering all four mapped sections.
///
/// Each section starts from its real floor list
/// (`schema_floor::expected_field_names`); `mutate` may then drop or add
/// names for that section. Every name that ends up emitted is also written
/// into a synthetic `## Authored vs derived` table, so `check_struct`
/// exempts it and only the name floor is exercised.
fn fixture_doc(mutate: impl Fn(&str, &mut Vec<String>)) -> String {
    let mut doc = String::new();
    let mut derived_rows = String::new();

    for heading in FIXTURE_SECTIONS {
        let mut names: Vec<String> = expected_field_names(heading)
            .iter()
            .map(|n| (*n).to_string())
            .collect();
        mutate(heading, &mut names);

        doc.push_str(&format!(
            "## {heading}\n\n| Field | Shape | Meaning |\n|---|---|---|\n"
        ));
        for name in &names {
            doc.push_str(&format!("| `{name}` | string | fixture row. |\n"));
            derived_rows.push_str(&format!("| **Derived** | `{name}` | fixture |\n"));
        }
        doc.push('\n');
    }

    doc.push_str("## Authored vs derived\n\n| Bucket | Fields | Who writes it |\n|---|---|---|\n");
    doc.push_str(&derived_rows);
    doc
}

/// Baseline: every mapped section fully documented. Confirms the fixture
/// plumbing itself (including the synthetic derived table that neutralises
/// `check_struct`) produces a clean run, so a failure in any test below is
/// attributable to that test's mutation and not to the harness.
#[test]
fn floor_fixture_fully_documented_passes() {
    let doc = fixture_doc(|_, _| {});
    let violations = run_conformance_check(&doc);
    assert!(
        violations.is_empty(),
        "a fixture documenting every field in every mapped section should pass; got: {violations:?}"
    );
}

/// A single deleted row is caught, and the violation NAMES the field that
/// disappeared. This is the case the count floor also caught, but only as a
/// bare number — the name is what makes the failure actionable.
#[test]
fn floor_catches_a_single_deleted_row_and_names_the_field() {
    let doc = fixture_doc(|heading, names| {
        if heading == "carryover[]" {
            names.retain(|n| n != "clears_when");
        }
    });
    let violations = run_conformance_check(&doc);

    assert_eq!(
        violations.len(),
        1,
        "deleting one `carryover[]` row should trip exactly one violation; got: {violations:?}"
    );
    assert!(
        violations[0].contains("section `carryover[]`") && violations[0].contains("clears_when"),
        "the violation must name both the section and the missing field `clears_when`; \
         got: {violations:?}"
    );
}

/// The OFFSETTING case, and the reason this ticket exists: one documented
/// field is dropped while two new ones are added, so the section's row
/// COUNT rises. The old count floor passed this fixture — every field
/// silently stopped being checked while the gate stayed green. The name
/// floor fails it, naming the dropped field.
#[test]
fn floor_catches_a_drop_masked_by_a_larger_addition() {
    let doc = fixture_doc(|heading, names| {
        if heading == "Block vocabulary" {
            names.retain(|n| n != "note");
            names.push("newly_added_field_one".to_string());
            names.push("newly_added_field_two".to_string());
        }
    });
    let violations = run_conformance_check(&doc);

    assert_eq!(
        violations.len(),
        1,
        "dropping one field while adding two should still trip exactly one violation \
         (the net row count rises, which is what defeated the count floor); got: {violations:?}"
    );
    assert!(
        violations[0].contains("section `Block vocabulary`") && violations[0].contains("note"),
        "the violation must name the dropped field `note`, not merely report a count; \
         got: {violations:?}"
    );
}

/// Legitimate growth: a section documents one more field than the floor
/// lists, and nothing is removed. This must PASS with `schema_floor.rs`
/// untouched — that is the drift fix. Under the old count floor this
/// direction was safe but the floor silently went stale; under the name
/// floor it needs no maintenance at all.
#[test]
fn floor_allows_growth_without_a_floor_edit() {
    let doc = fixture_doc(|heading, names| {
        if heading == "carryover[]" {
            names.push("a_newly_documented_field".to_string());
        }
    });
    let violations = run_conformance_check(&doc);
    assert!(
        violations.is_empty(),
        "documenting a NEW field without removing any is legitimate growth and must pass \
         with no edit to schema_floor.rs; got: {violations:?}"
    );
}

/// Total parse failure: headings present, no `| Field | Shape | Meaning |`
/// table under any mapped section. Must fail for all four, and each
/// violation must name that section's missing fields rather than report a
/// bare observed count.
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
    for heading in FIXTURE_SECTIONS {
        let violation = violations
            .iter()
            .find(|v| v.contains(&format!("section `{heading}`")))
            .unwrap_or_else(|| {
                panic!("expected a violation naming `{heading}`; got: {violations:?}")
            });
        for name in expected_field_names(heading) {
            assert!(
                violation.contains(name),
                "the `{heading}` violation must name every missing field (missing `{name}`); \
                 got: {violation}"
            );
        }
    }
}

/// Narrowing is caught for each mapped section in turn — building each
/// fixture is mechanical, so full coverage costs nothing over picking one
/// representative. Each section loses its first documented field; that
/// section trips exactly one violation naming the field, and the other
/// three stay clean.
#[test]
fn floor_narrowing_is_caught_for_each_mapped_section() {
    for heading in FIXTURE_SECTIONS {
        let dropped = expected_field_names(heading)[0];
        let doc = fixture_doc(|h, names| {
            if h == heading {
                names.retain(|n| n != dropped);
            }
        });
        let violations = run_conformance_check(&doc);

        assert_eq!(
            violations.len(),
            1,
            "narrowing `{heading}` by one field should trip exactly one violation \
             (the other three sections stay fully documented); got: {violations:?}"
        );
        assert!(
            violations[0].contains(&format!("section `{heading}`"))
                && violations[0].contains(dropped),
            "expected the violation to name `{heading}` and its dropped field `{dropped}`; \
             got: {violations:?}"
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
