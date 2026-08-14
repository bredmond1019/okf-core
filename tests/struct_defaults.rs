//! Proves the literal-construction property `#[derive(Default)]` buys
//! (`OK.ticket.struct-field-default-derive`, task 2).
//!
//! Task 1 added `#[derive(Default)]` to the six authored `src/state.rs` structs
//! (`StateFile`, `Track`, `TrackBlock`, `Backlog`, `Epic`, `Carryover`) plus the
//! `CarryoverScope` prerequisite. This file is the compile-time guard: removing
//! the derive from any of the six will fail this file at compile time, not at
//! some downstream consumer's `cargo test --no-run`.
//!
//! Three historical breakages motivate this file:
//! 1. `Epic` gained `weight` during mev Phase 11 -> broke `bastion`'s literals.
//! 2. `OK.3.B` added `#[serde(flatten, default)] extra` -> broke `mev` at 101 sites (`de81724`).
//! 3. The same `extra` broke `bastion` at 31 sites (`cargo test --no-run` -> 31 x `E0063`).
//!
//! Do NOT edit `tests/schema_conformance.rs` or `tests/state_preservation.rs` from this
//! task — those pin the OK.3.A and OK.3.B properties and must stay untouched so this
//! change is provably non-weakening.

use okf_core::{Backlog, Carryover, Epic, Reference, StateFile, Track, TrackBlock};

// ---------------------------------------------------------------------------
// Test 1 — literal construction naming only the caller-relevant fields.
// ---------------------------------------------------------------------------
//
// This is the exact pattern the 101 mev sites and 31 bastion sites had to be
// rewritten into. The test *compiling* is the regression guard: a future field
// addition can only break this file if the `Default` derive is removed.

#[test]
fn state_file_constructs_with_partial_literal() {
    let state = StateFile {
        repo: "okf-core".to_string(),
        kind: "project".to_string(),
        updated: "2026-08-04".to_string(),
        ..Default::default()
    };
    assert_eq!(state.repo, "okf-core");
}

#[test]
fn track_constructs_with_partial_literal() {
    let track = Track {
        title: "Phase 1".to_string(),
        ..Default::default()
    };
    assert_eq!(track.title, "Phase 1");
}

#[test]
fn track_block_constructs_with_partial_literal() {
    let block = TrackBlock {
        id: "OK.9.A".to_string(),
        title: "t".to_string(),
        ..Default::default()
    };
    assert_eq!(block.id, "OK.9.A");
    assert_eq!(block.title, "t");
}

#[test]
fn backlog_constructs_with_partial_literal() {
    let backlog = Backlog {
        slug: "seed-item".to_string(),
        title: "seed item".to_string(),
        ..Default::default()
    };
    assert_eq!(backlog.slug, "seed-item");
}

#[test]
fn epic_constructs_with_partial_literal() {
    let epic = Epic {
        slug: "bastion-os".to_string(),
        title: "Bastion OS".to_string(),
        ..Default::default()
    };
    assert_eq!(epic.slug, "bastion-os");
}

#[test]
fn carryover_constructs_with_partial_literal() {
    let carryover = Carryover {
        slug: "seed-carryover".to_string(),
        kind: "known_issue".to_string(),
        text: "some caveat".to_string(),
        created: "2026-08-04".to_string(),
        ..Default::default()
    };
    assert_eq!(carryover.slug, "seed-carryover");
}

// ---------------------------------------------------------------------------
// Test 2 — a defaulted value carries no phantom keys.
// ---------------------------------------------------------------------------
//
// The flattened `extra` capture map must default to empty, so serializing a
// `Default::default()` value never resurrects unmodeled keys that were never
// authored.

/// Returns the set of keys the struct itself never models — i.e. anything a
/// caller could reasonably suspect leaked in from a non-empty `extra` map.
/// We check this indirectly: assert the serialized object has exactly the
/// keys the struct's own fields produce (no unexpected extras beyond them).
fn assert_no_phantom_extra_keys(value: &serde_json::Value, known_fields: &[&str]) {
    let obj = value.as_object().expect("expected a JSON object");
    for key in obj.keys() {
        assert!(
            known_fields.contains(&key.as_str()),
            "unexpected key `{key}` in serialized default value \
             (extra capture map should be empty on a Default::default() value): {value}"
        );
    }
}

#[test]
fn state_file_default_has_no_phantom_keys() {
    let value = serde_json::to_value(StateFile::default()).unwrap();
    assert_no_phantom_extra_keys(
        &value,
        &[
            "repo",
            "kind",
            "updated",
            "focus",
            "tracks",
            "repos",
            "cross_repo",
            "tiers",
            "epics",
            "note",
            "backlog",
            "carryover",
        ],
    );
}

#[test]
fn track_default_has_no_phantom_keys() {
    let value = serde_json::to_value(Track::default()).unwrap();
    assert_no_phantom_extra_keys(&value, &["title", "blocks"]);
}

#[test]
fn track_block_default_has_no_phantom_keys() {
    let value = serde_json::to_value(TrackBlock::default()).unwrap();
    assert_no_phantom_extra_keys(
        &value,
        &[
            "id",
            "title",
            "status",
            "depends_on",
            "wave",
            "origin",
            "note",
            "description",
            "priority",
            "due",
            "sdlc_workflow",
            "model",
            "epics",
        ],
    );
}

#[test]
fn backlog_default_has_no_phantom_keys() {
    let value = serde_json::to_value(Backlog::default()).unwrap();
    assert_no_phantom_extra_keys(
        &value,
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
        ],
    );
}

#[test]
fn epic_default_has_no_phantom_keys() {
    let value = serde_json::to_value(Epic::default()).unwrap();
    assert_no_phantom_extra_keys(
        &value,
        &[
            "slug",
            "title",
            "description",
            "status",
            "weight",
            "plan",
            "repos",
        ],
    );
}

#[test]
fn carryover_default_has_no_phantom_keys() {
    let value = serde_json::to_value(Carryover::default()).unwrap();
    assert_no_phantom_extra_keys(
        &value,
        &[
            "slug",
            "scope",
            "kind",
            "text",
            "related",
            "clears_when",
            "created",
            "reviewed",
            "snoozed_until",
        ],
    );
}

/// Explicit named guard for the three triage fields added alongside typed
/// `ClearsWhen`: a default `Carryover` must serialize WITHOUT `priority`,
/// `blocks`, or `finding_id`. `carryover_default_has_no_phantom_keys` above
/// already covers this by construction (those three names are simply absent
/// from its `known_fields` list); the point of naming them here is that a
/// future author who "tidies" their `#[serde(default, skip_serializing_if =
/// ...)]` attributes to match `Carryover.related`'s legacy bare
/// `#[serde(default)]` (`src/state.rs:496` at the time of writing) gets a
/// failure that says exactly which field regressed, rather than a generic
/// "unexpected key" message.
#[test]
fn carryover_default_omits_priority_blocks_finding_id() {
    let value = serde_json::to_value(Carryover::default()).unwrap();
    let obj = value.as_object().expect("expected a JSON object");
    assert!(
        !obj.contains_key("priority"),
        "a default Carryover must omit `priority` (do not copy `related`'s bare #[serde(default)]): {value}"
    );
    assert!(
        !obj.contains_key("blocks"),
        "a default Carryover must omit `blocks` (do not copy `related`'s bare #[serde(default)]): {value}"
    );
    assert!(
        !obj.contains_key("finding_id"),
        "a default Carryover must omit `finding_id` (do not copy `related`'s bare #[serde(default)]): {value}"
    );
}

/// Mirrors `carryover_default_has_no_phantom_keys` for the new `Reference`
/// struct (`OK.ticket.reference-container-fields`, task 3). `Reference`
/// carries no `clears_when`, `priority`, or `blocks` by design — it never
/// had them to omit, unlike `Carryover`'s explicit-omission guard above.
#[test]
fn reference_default_has_no_phantom_keys() {
    let value = serde_json::to_value(Reference::default()).unwrap();
    assert_no_phantom_extra_keys(
        &value,
        &[
            "slug", "scope", "class", "text", "created", "related", "reviewed",
        ],
    );
}

// ---------------------------------------------------------------------------
// Test 3 — round-trip safety of the defaults.
// ---------------------------------------------------------------------------

#[test]
fn state_file_default_round_trips() {
    let original = serde_json::to_value(StateFile::default()).unwrap();
    let parsed: StateFile = serde_json::from_value(original.clone()).unwrap();
    let round = serde_json::to_value(parsed).unwrap();
    assert_eq!(original, round);
}

#[test]
fn track_default_round_trips() {
    let original = serde_json::to_value(Track::default()).unwrap();
    let parsed: Track = serde_json::from_value(original.clone()).unwrap();
    let round = serde_json::to_value(parsed).unwrap();
    assert_eq!(original, round);
}

#[test]
fn track_block_default_round_trips() {
    let original = serde_json::to_value(TrackBlock::default()).unwrap();
    let parsed: TrackBlock = serde_json::from_value(original.clone()).unwrap();
    let round = serde_json::to_value(parsed).unwrap();
    assert_eq!(original, round);
}

#[test]
fn backlog_default_round_trips() {
    // `Backlog` does not derive `PartialEq`, so compare via `serde_json::Value`.
    let original = serde_json::to_value(Backlog::default()).unwrap();
    let parsed: Backlog = serde_json::from_value(original.clone()).unwrap();
    let round = serde_json::to_value(parsed).unwrap();
    assert_eq!(original, round);
}

#[test]
fn epic_default_round_trips() {
    let original = Epic::default();
    let parsed: Epic = serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn carryover_default_round_trips() {
    let original = serde_json::to_value(Carryover::default()).unwrap();
    let parsed: Carryover = serde_json::from_value(original.clone()).unwrap();
    let round = serde_json::to_value(parsed).unwrap();
    assert_eq!(original, round);
}
