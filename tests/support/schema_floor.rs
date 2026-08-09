// Per-section field-count floor for the schema/struct conformance gate
// (`3.1-schema-struct-conformance`).
//
// `schema_conformance.rs`'s four `!is_empty()` asserts are a floor of ONE —
// enough to catch total parser failure, not enough to catch PARTIAL parser
// failure. `parse_field_tables`'s table detection is an exact string match
// (`schema_parse.rs:98`, `| Field | Shape | Meaning |`), and its row
// collection is conditional per line (`schema_parse.rs:82`), so a doc
// reformat — a renamed heading, an extra column, a table split under a new
// sub-heading — can silently drop most of a section's rows while the
// non-empty assert still passes. This module is the fix: a per-section
// floor derived from ONE canonical source, so the four call sites in
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
// (b) **An explicit per-section expected-count table, in one place** — not
//     self-maintaining (a legitimate doc addition that pushes a section past
//     its floor does not itself require a bump, because the check is `>=`,
//     but TIGHTENING the floor to match new growth is a manual, deliberate
//     edit), but honest and singular: one number per section, commented
//     against the doc, in exactly one file. No four-call-site duplication,
//     no reflection, no risk of the floor secretly depending on the same
//     doc formatting it exists to protect against.
//
// (c) **A machine-readable count recorded inside `state-schema.md` itself**
//     — rejected outright: that puts the floor in the SAME file whose
//     reformat is the threat this gate defends against. A header rename
//     that drops table rows could just as easily carry off a stray
//     "expected count: N" annotation in the same edit, and now the floor
//     narrows in lockstep with the thing it is supposed to catch narrowing.
//
// **Chosen: (b).** The counts below were read off the current
// `docs/state/state-schema.md` field tables (Block vocabulary = 14,
// `backlog[]` = 12, `epics[]` = 7, `carryover[]` = 12) — every row currently
// documented, not filtered to non-derived fields, because the floor guards
// the PARSE (what `parse_field_tables` returns), not the post-exemption
// conformance check that runs on top of it.
//
// ## What this floor CANNOT catch
//
// A static floor cannot distinguish "legitimate growth exactly offsetting a
// narrowing" from "no change": if a section gains two genuinely new fields
// in the SAME doc edit that also drops one existing field to a reformatted
// table, the observed count (net +1) still clears the old floor and nothing
// fails, even though a real field silently stopped being checked. Catching
// that would require a floor that tracks specific field NAMES, not just a
// count — a heavier design this ticket's fixture-driven test triple does not
// require and the ticket's Notes section warns against ("the floor is a
// floor, not an equality"). Two narrower, related gaps: this floor is not
// bumped automatically when the doc legitimately grows a section (a human
// has to notice and choose to tighten it, though the untightened `>=` floor
// keeps passing either way, so nothing is unsafe by leaving it loose); and
// it is scoped to exactly the four sections `section_matches` maps to a
// struct, per the ticket's stated out-of-scope.

/// The expected minimum number of documented field rows for a schema
/// section, keyed by the same leading identifier `section_matches` (in
/// `schema_conformance.rs`) uses to map a parsed heading to a struct:
/// `Block vocabulary`, `backlog[]`, `epics[]`, `carryover[]`.
///
/// This is a FLOOR (`>=`), not an equality — a section with MORE documented
/// fields than its floor is legitimate growth, not a failure. See the
/// module doc comment above for the rationale and the narrowing it cannot
/// catch.
///
/// Panics on an unmapped section name: the four sections here are exactly
/// the four `schema_struct_conformance` checks, so an unrecognised section
/// is a caller bug (a typo'd identifier, or a fifth section added to the
/// gate without updating this table), not a data condition to tolerate
/// silently.
pub fn expected_field_count(section: &str) -> usize {
    match section {
        // 14 documented rows as of this writing: id, title, status, wave,
        // depends_on, note, description, origin, tasks, priority, due,
        // sdlc_workflow, model, epics.
        "Block vocabulary" => 14,
        // 12 documented rows: slug, title, repo, type, status, depends_on,
        // block, notes, origin, created, reviewed, snoozed_until.
        "backlog[]" => 12,
        // 7 documented rows: slug, title, description, status, weight,
        // plan, repos.
        "epics[]" => 7,
        // 12 documented rows: slug, scope, kind, text, related, clears_when,
        // priority, blocks, finding_id, created, reviewed, snoozed_until.
        "carryover[]" => 12,
        other => panic!(
            "expected_field_count: unmapped section `{other}` — the count \
             floor only covers the four sections `section_matches` maps to \
             a struct (`Block vocabulary`, `backlog[]`, `epics[]`, \
             `carryover[]`); if a new section needs a floor, add it here \
             rather than skipping the check."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_expected_floor_for_each_mapped_section() {
        assert_eq!(expected_field_count("Block vocabulary"), 14);
        assert_eq!(expected_field_count("backlog[]"), 12);
        assert_eq!(expected_field_count("epics[]"), 7);
        assert_eq!(expected_field_count("carryover[]"), 12);
    }

    #[test]
    fn floors_are_all_at_least_one() {
        // A floor of zero would defeat the point — it would tolerate total
        // parser failure the same way the old `!is_empty()` assert did not.
        for section in ["Block vocabulary", "backlog[]", "epics[]", "carryover[]"] {
            assert!(
                expected_field_count(section) >= 1,
                "floor for `{section}` must be at least 1"
            );
        }
    }

    #[test]
    #[should_panic(expected = "unmapped section")]
    fn panics_on_an_unmapped_section() {
        expected_field_count("not a real section");
    }
}
