// Per-section field-NAME floor for the schema/struct conformance gate
// (`3.1-schema-struct-conformance`).
//
// `schema_conformance.rs`'s five `!is_empty()` asserts are a floor of ONE —
// enough to catch total parser failure, not enough to catch PARTIAL parser
// failure. `parse_field_tables`'s table detection is an exact string match
// (`schema_parse.rs:98`, `| Field | Shape | Meaning |`), and its row
// collection is conditional per line (`schema_parse.rs:82`), so a doc
// reformat — a renamed heading, an extra column, a table split under a new
// sub-heading — can silently drop most of a section's rows while the
// non-empty assert still passes. This module is the fix: a per-section
// floor derived from ONE canonical source, so the five call sites in
// `schema_conformance.rs` never carry their own magic numbers.
//
// ## Canonical source chosen: an explicit per-section table, here
//
// The ticket (`ticket-conformance-field-count-floor/tasks.md`, task 1) named
// three candidate sources and asked for a judgment call between them:
//
// (a) **The mapped struct's own serde field set** — self-maintaining (a new
//     struct field raises the floor automatically), but this crate has no
//     reflection and adding one would violate AGENT.md rule 3 (`okf-core`'s
//     `src/` is I/O- and dependency-free; `serde`/`serde_json`/`thiserror`
//     only). Simulating it without reflection means hand-building a
//     "complete" seed JSON with every field set — which is exactly the same
//     kind of list this module keeps in one place, just moved and harder to
//     audit. Worse: it is derived from the STRUCT, not the doc, so it
//     inherits the struct's own blind spots — it cannot catch a narrowing
//     that drops a field the struct ALSO lacks, which is exactly the
//     `TrackBlock.note` incident this whole gate exists to catch (see the
//     module doc comment on `schema_conformance.rs`).
//
// (b) **An explicit per-section expected-names table, in one place** — not
//     self-maintaining for TIGHTENING (a legitimate doc removal that drops a
//     documented field still requires a deliberate, reviewed edit here — that
//     is the point, not a gap), but honest and singular: one list per
//     section, in exactly one file. No five-call-site duplication, no
//     reflection, no risk of the floor secretly depending on the same doc
//     formatting it exists to protect against.
//
// (c) **A machine-readable list recorded inside `state-schema.md` itself**
//     — rejected outright: that puts the floor in the SAME file whose
//     reformat is the threat this gate defends against. A header rename
//     that drops table rows could just as easily carry off a stray
//     "expected fields: ..." annotation in the same edit, and now the floor
//     narrows in lockstep with the thing it is supposed to catch narrowing.
//
// **Chosen: (b).** This ticket (`ticket-conformance-field-name-floor`)
// replaces the previous COUNT floor's unit with NAMES, keeping the floor's
// location unchanged — see that ticket's tasks.md and the Amendment Log on
// `ticket-conformance-field-count-floor/tasks.md` (2026-08-09) for why the
// location stays an explicit table here rather than the struct or the doc
// itself. The names below were read off the current
// `docs/state/state-schema.md` field tables (Block vocabulary = 14 names,
// `backlog[]` = 12, `epics[]` = 7, `carryover[]` = 12) — every row currently
// documented, not filtered to non-derived fields, because the floor guards
// the PARSE (what `parse_field_tables` returns), not the post-exemption
// conformance check that runs on top of it.
//
// ## Subset semantics, not equality
//
// The floor asserts that every NAME stored here appears among the names
// `parse_field_tables` actually observed for that section — it is a subset
// check, not an equality check. A section with MORE documented fields than
// its stored list is legitimate growth, not a failure, and growth requires
// NO edit to this file: the new field simply appears in `observed` without a
// corresponding entry in `expected`, and the subset relation still holds.
// This is what fixes the drift the old count floor had — under `>=`, growth
// silently loosened the floor and TIGHTENING it back up was a manual,
// four-site edit (commit `c99d21d`, when `carryover[]` grew 9 -> 12). Under
// a name-based subset check there is nothing to tighten: the list only
// changes on a deliberate, reviewed field REMOVAL.
//
// ## What this floor CANNOT catch
//
// The two gaps the old count floor had are now CLOSED: an offsetting
// add/drop in the same doc edit (previously net-positive, so a count floor
// never noticed) now fails, because the dropped name disappears from
// `observed` regardless of how many other names were added; and a plain row
// deletion now fails and names the missing field, rather than only degrading
// a count that might still clear a stale floor.
//
// What remains uncatchable: a field renamed SIMULTANEOUSLY in the doc, the
// mapped struct, and this floor's list, in one edit — nothing short of
// review catches that, and it is not silent, since it appears in the diff.
// This floor is scoped to exactly the five sections `section_matches` maps
// to a struct (`reference[]` joined the other four via
// `ticket-reference-container-schema-doc`, task 3); any further authored
// section added to the doc without a matching edit here and in
// `schema_conformance.rs` stays unmapped and unnoticed by this floor.

/// The expected field names documented for a schema section, keyed by the
/// same leading identifier `section_matches` (in `schema_conformance.rs`)
/// uses to map a parsed heading to a struct: `Block vocabulary`, `backlog[]`,
/// `epics[]`, `carryover[]`, `reference[]`.
///
/// This is a SUBSET floor, not an equality — every name returned here must
/// appear among the section's parsed field names, but the parsed names may
/// legitimately be a strict superset (growth needs no edit here). See the
/// module doc comment above for the full rationale and what this still
/// cannot catch.
///
/// Panics on an unmapped section name: the five sections here are exactly
/// the five `schema_struct_conformance` checks, so an unrecognised section
/// is a caller bug (a typo'd identifier, or a sixth section added to the
/// gate without updating this table), not a data condition to tolerate
/// silently.
pub fn expected_field_names(section: &str) -> &'static [&'static str] {
    match section {
        "Block vocabulary" => &[
            "id",
            "title",
            "status",
            "wave",
            "depends_on",
            "note",
            "description",
            "origin",
            "tasks",
            "priority",
            "due",
            "sdlc_workflow",
            "model",
            "epics",
        ],
        "backlog[]" => &[
            "slug",
            "title",
            "repo",
            "type",
            "status",
            "depends_on",
            "block",
            "notes",
            "origin",
            "created",
            "reviewed",
            "snoozed_until",
        ],
        "epics[]" => &[
            "slug",
            "title",
            "description",
            "status",
            "weight",
            "plan",
            "repos",
        ],
        "carryover[]" => &[
            "slug",
            "scope",
            "kind",
            "text",
            "related",
            "clears_when",
            "priority",
            "blocks",
            "finding_id",
            "created",
            "reviewed",
            "snoozed_until",
        ],
        "reference[]" => &[
            "slug",
            "scope",
            "class",
            "text",
            "created",
            "related",
            "reviewed",
        ],
        other => panic!(
            "expected_field_names: unmapped section `{other}` — the name \
             floor only covers the five sections `section_matches` maps to \
             a struct (`Block vocabulary`, `backlog[]`, `epics[]`, \
             `carryover[]`, `reference[]`); if a new section needs a floor, \
             add it here rather than skipping the check."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_expected_names_for_each_mapped_section() {
        assert_eq!(
            expected_field_names("Block vocabulary"),
            &[
                "id",
                "title",
                "status",
                "wave",
                "depends_on",
                "note",
                "description",
                "origin",
                "tasks",
                "priority",
                "due",
                "sdlc_workflow",
                "model",
                "epics",
            ]
        );
        assert_eq!(
            expected_field_names("backlog[]"),
            &[
                "slug",
                "title",
                "repo",
                "type",
                "status",
                "depends_on",
                "block",
                "notes",
                "origin",
                "created",
                "reviewed",
                "snoozed_until",
            ]
        );
        assert_eq!(
            expected_field_names("epics[]"),
            &[
                "slug",
                "title",
                "description",
                "status",
                "weight",
                "plan",
                "repos"
            ]
        );
        assert_eq!(
            expected_field_names("carryover[]"),
            &[
                "slug",
                "scope",
                "kind",
                "text",
                "related",
                "clears_when",
                "priority",
                "blocks",
                "finding_id",
                "created",
                "reviewed",
                "snoozed_until",
            ]
        );
        assert_eq!(
            expected_field_names("reference[]"),
            &[
                "slug",
                "scope",
                "class",
                "text",
                "created",
                "related",
                "reviewed",
            ]
        );
    }

    #[test]
    fn lists_are_all_non_empty() {
        // An empty list would defeat the point — it would tolerate total
        // parser failure the same way the old `!is_empty()` assert did not,
        // and the same way a floor of zero would under the count design.
        for section in [
            "Block vocabulary",
            "backlog[]",
            "epics[]",
            "carryover[]",
            "reference[]",
        ] {
            assert!(
                !expected_field_names(section).is_empty(),
                "name list for `{section}` must be non-empty"
            );
        }
    }

    #[test]
    #[should_panic(expected = "unmapped section")]
    fn panics_on_an_unmapped_section() {
        expected_field_names("not a real section");
    }
}
