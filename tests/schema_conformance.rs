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
use support::schema_floor::expected_field_count;
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

/// Check that `fields` (the fields parsed under `section`) meet or exceed
/// the canonical floor from `schema_floor::expected_field_count`, pushing a
/// violation onto `violations` rather than panicking immediately — this
/// matches `check_struct`'s accumulate-don't-panic-on-first shape, so a run
/// that narrows multiple sections at once surfaces all of them, not just
/// the first.
///
/// This replaces the old `!fields.is_empty()` asserts, which were a floor
/// of ONE: enough to catch total parser failure, not enough to catch
/// PARTIAL parser failure (a doc reformat that drops most, but not all, of
/// a section's rows). See `schema_floor.rs`'s module doc comment for why
/// the floor is derived from an explicit per-section table rather than
/// hardcoded at each of the four call sites.
fn check_field_count_floor(
    section: &str,
    fields: &[&DocumentedField],
    violations: &mut Vec<String>,
) {
    let observed = fields.len();
    let floor = expected_field_count(section);
    if observed < floor {
        violations.push(format!(
            "section `{section}` parsed only {observed} field(s), expected at least {floor} \
             (floor from `schema_floor::expected_field_count`, a per-section table hand-derived \
             from `docs/state/state-schema.md`) — either the doc's `| Field | Shape | Meaning |` \
             table for this section was reformatted and rows were silently dropped by the parser \
             (check schema_parse.rs's exact header match), or the section legitimately shrank and \
             the floor in `schema_floor.rs` needs a deliberate, reviewed lowering"
        ));
    }
}

/// The gate: every authored field documented on one of the four mapped
/// sections has a corresponding field on the matching struct.
#[test]
fn schema_struct_conformance() {
    let doc = read_schema_doc();
    let fields = parse_field_tables(&doc);
    let derived = parse_derived_fields(&doc);

    let mut violations = Vec::new();

    let block_vocabulary: Vec<&DocumentedField> = fields
        .iter()
        .filter(|f| section_matches(&f.section, "Block vocabulary"))
        .collect();
    check_field_count_floor("Block vocabulary", &block_vocabulary, &mut violations);
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
    check_field_count_floor("backlog[]", &backlog_fields, &mut violations);
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
    check_field_count_floor("epics[]", &epic_fields, &mut violations);
    check_struct::<Epic>(epic_seed(), &epic_fields, &derived, "Epic", &mut violations);

    let carryover_fields: Vec<&DocumentedField> = fields
        .iter()
        .filter(|f| section_matches(&f.section, "carryover[]"))
        .collect();
    check_field_count_floor("carryover[]", &carryover_fields, &mut violations);
    check_struct::<Carryover>(
        carryover_seed(),
        &carryover_fields,
        &derived,
        "Carryover",
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "schema/struct conformance drift found:\n{}",
        violations.join("\n")
    );
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
