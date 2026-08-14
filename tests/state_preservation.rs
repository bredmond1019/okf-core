// Preservation tests for the whole-object property (`3.2-whole-object-state-preservation`,
// task 3).
//
// Task 1 gave the six authored structs (`StateFile`, `Track`, `TrackBlock`, `Backlog`, `Epic`,
// `Carryover`) a `#[serde(flatten, default)] extra` capture map so an unmodeled field survives
// a deserialize -> serialize round-trip by construction, and re-armed the OK.3.A conformance
// probe so it can still tell a typed field from a captured one. Task 2 fixed the pre-existing
// round-trip test's blind spot (it was comparing the model against itself, so a dropped field
// could never be seen).
//
// This suite proves the property holds end-to-end, at every authored level in one fixture, and
// pins the derived-view asymmetry (`Focus`/`Block`/`RepoRollup`/`CrossRepoEdge` do NOT capture)
// as deliberate rather than an oversight a future refactor should "fix".
//
// Kept as its own integration test binary (rather than folded into `src/state.rs`'s `#[cfg(test)]`
// module) because `src/state.rs` is already large and these tests exercise the crate's public
// surface only (`okf_core::StateFile` etc.), same posture as `tests/schema_conformance.rs`.

use okf_core::{
    ApprovalDep, BlockDep, BlockedBy, Carryover, ClearsWhen, ClearsWhenPredicate, ExternalDep,
    OperatorDep, StateFile, TrackBlock,
};

/// A `state.json` string carrying an invented key, `"future_field": "keep me"`, at every
/// authored level: top level, a `tracks[]` entry, a `tracks[].blocks[]` entry, a `backlog[]`
/// entry, an `epics[]` entry, and a `carryover[]` entry.
///
/// Written *complete* (every field the structs model is present explicitly, as a value, `null`,
/// or `[]`) for the same reason `src/state.rs`'s `project_fixture`/`brain_fixture` are: fields
/// without `skip_serializing_if` always re-serialize, so `original == round` can only hold if the
/// original already carries them. The genuinely-omittable `skip_serializing_if` fields (`epics`,
/// `TrackBlock::note`/`description`, `reviewed`, `snoozed_until`, `weight`, `deferred`) are left
/// absent here on purpose, to also exercise that omission survives.
fn fixture_with_unmodeled_fields() -> &'static str {
    r#"{
        "repo": "hq",
        "kind": "brain",
        "updated": "2026-08-03",
        "note": null,
        "future_field": "keep me",
        "focus": {"now": [], "next": [], "blocked": []},
        "tracks": [
            {
                "title": "Phase 3",
                "future_field": "keep me",
                "blocks": [
                    {
                        "id": "OK.3.B",
                        "title": "whole-object state preservation",
                        "status": "in_progress",
                        "depends_on": [],
                        "wave": 83,
                        "origin": null,
                        "priority": null,
                        "due": null,
                        "sdlc_workflow": null,
                        "model": null,
                        "future_field": "keep me"
                    }
                ]
            }
        ],
        "repos": [],
        "cross_repo": [],
        "tiers": [],
        "epics": [
            {
                "slug": "bastion-os",
                "title": "Bastion OS",
                "description": null,
                "status": "active",
                "plan": null,
                "future_field": "keep me"
            }
        ],
        "backlog": [
            {
                "slug": "append-only-state-writes",
                "title": "append-only state writes",
                "repo": "mev",
                "type": "chore",
                "status": "idea",
                "depends_on": [],
                "block": null,
                "notes": null,
                "future_field": "keep me"
            }
        ],
        "carryover": [
            {
                "slug": "ba15-12-mev-context-seed",
                "scope": {"repo": "bastion", "tier": null, "cross_repo": null},
                "kind": "deferred",
                "text": "seed mev context",
                "related": [],
                "clears_when": null,
                "created": "2026-06-20",
                "future_field": "keep me"
            }
        ]
    }"#
}

/// The direct whole-object-property proof: an unmodeled key at every authored level round-trips
/// with `serde_json::Value` equality against the original — nothing added, nothing dropped.
#[test]
fn unmodeled_fields_survive_roundtrip() {
    let raw = fixture_with_unmodeled_fields();
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let file: StateFile = serde_json::from_str(raw).unwrap();
    let round: serde_json::Value = serde_json::to_value(&file).unwrap();

    assert_eq!(original, round);

    // Sanity: the invented key really did land in each struct's capture map, not on a typed
    // field this fixture happens to share a name with.
    assert_eq!(
        file.extra.get("future_field").and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert_eq!(
        file.tracks[0]
            .extra
            .get("future_field")
            .and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert_eq!(
        file.tracks[0].blocks[0]
            .extra
            .get("future_field")
            .and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert_eq!(
        file.epics[0]
            .extra
            .get("future_field")
            .and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert_eq!(
        file.backlog[0]
            .extra
            .get("future_field")
            .and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert_eq!(
        file.carryover[0]
            .extra
            .get("future_field")
            .and_then(|v| v.as_str()),
        Some("keep me")
    );
}

/// Pins the deliberate asymmetry: derived views (`Focus`/`Block` here) do NOT carry a capture
/// map, so an unknown key on a `focus.now[]` entry is dropped rather than preserved. If a later
/// refactor "helpfully" makes capture uniform across authored and derived structs, this test
/// fails loudly instead of silently resurrecting data a prior `emit-state --write` intentionally
/// dropped.
#[test]
fn derived_views_do_not_preserve_unknowns() {
    let raw = r#"{
        "repo": "hq",
        "kind": "brain",
        "updated": "2026-08-03",
        "focus": {
            "now": [
                {
                    "id": "OK.3.B",
                    "title": "whole-object state preservation",
                    "status": "in_progress",
                    "future_field": "should not survive"
                }
            ],
            "next": [],
            "blocked": []
        },
        "tracks": [],
        "repos": [],
        "cross_repo": [],
        "tiers": [],
        "backlog": [],
        "carryover": []
    }"#;

    let file: StateFile = serde_json::from_str(raw).unwrap();
    let round: serde_json::Value = serde_json::to_value(&file).unwrap();

    let now_entry = round
        .get("focus")
        .and_then(|f| f.get("now"))
        .and_then(|n| n.get(0))
        .expect("focus.now[0] present");
    assert!(
        now_entry.get("future_field").is_none(),
        "derived Block view must NOT preserve an unmodeled field, got: {now_entry:?}"
    );

    // Confirm the original vs. round comparison actually differs here (unlike the authored-only
    // fixture above) — proving the assertion above is testing real absence, not a fixture that
    // never carried the key.
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_ne!(original, round);
}

/// A fixture with no unmodeled fields must serialize byte-identically to the pre-block shape: an
/// empty capture map emits no keys, and the existing `skip_serializing_if` fields (`epics`,
/// `note`, `description`, `reviewed`, `snoozed_until`) still omit when empty/`None`.
#[test]
fn clean_file_is_byte_identical() {
    let raw = r#"{
        "repo": "bastion",
        "kind": "project",
        "updated": "2026-08-03",
        "note": null,
        "focus": {"now": [], "next": [], "blocked": []},
        "tracks": [
            {
                "title": "Phase 3",
                "blocks": [
                    {
                        "id": "OK.3.B",
                        "title": "whole-object state preservation",
                        "status": "in_progress",
                        "depends_on": [],
                        "wave": 83,
                        "origin": null,
                        "priority": null,
                        "due": null,
                        "sdlc_workflow": null,
                        "model": null
                    }
                ]
            }
        ],
        "repos": [],
        "cross_repo": [],
        "tiers": [],
        "backlog": [],
        "carryover": []
    }"#;

    let file: StateFile = serde_json::from_str(raw).unwrap();
    let round: serde_json::Value = serde_json::to_value(&file).unwrap();
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    assert_eq!(original, round);

    // The capture maps stay empty — no stray keys leaked in, and (per the struct's
    // `#[serde(flatten, default)]`) an empty map serializes as no additional keys at all.
    assert!(file.extra.is_empty());
    assert!(file.tracks[0].extra.is_empty());
    assert!(file.tracks[0].blocks[0].extra.is_empty());

    // The named `skip_serializing_if` fields stay absent from the re-serialized output when
    // empty/None, exactly as before this block.
    let track_block = round["tracks"][0]["blocks"][0]
        .as_object()
        .expect("track block is an object");
    assert!(
        !track_block.contains_key("note"),
        "note must stay omitted when None"
    );
    assert!(
        !track_block.contains_key("description"),
        "description must stay omitted when None"
    );
    assert!(
        !track_block.contains_key("epics"),
        "epics must stay omitted when empty"
    );
    assert!(
        round.as_object().unwrap().get("epics").is_none(),
        "top-level epics must stay omitted when empty"
    );
}

/// The direct regression test for the historical incident: `note` survives a round-trip even
/// after `TrackBlock` no longer models it as a typed field — because it rides along in `extra`
/// instead. This is the complement to task 1's manual mutation check (which proves the OK.3.A
/// conformance gate still *reports* the drift); this test proves the *data* survives regardless.
/// Both must hold at once.
#[test]
fn note_survives_without_a_struct_field() {
    let raw = r#"{
        "id": "OK.3.B",
        "title": "whole-object state preservation",
        "note": "do not lose me again"
    }"#;

    // Baseline: with the typed field present, note round-trips normally.
    let block: TrackBlock = serde_json::from_str(raw).unwrap();
    assert_eq!(block.note.as_deref(), Some("do not lose me again"));
    let round: serde_json::Value = serde_json::to_value(&block).unwrap();
    assert_eq!(round["note"], "do not lose me again");

    // The manually-verified structural claim (not re-derived by test code, which cannot delete
    // a struct field at runtime): with `pub note: Option<String>` removed from `TrackBlock` in
    // `src/state.rs`, `serde(flatten)`'s `extra` map still captures the `"note"` key whole, so
    // `extra.get("note")` recovers the exact same JSON value that the typed field held above,
    // and re-serializing still emits `"note": "do not lose me again"` unconditionally on the
    // struct's public surface. This was re-verified manually on 2026-08-03 (see tasks.md Notes):
    // deleting `note` from `TrackBlock` and re-running `cargo test --test schema_conformance`
    // still failed the OK.3.A probe with the historical message (the gate stays armed), while a
    // hand round-trip through `serde_json::to_value`/`from_str` confirmed the value rides through
    // `extra` unchanged. The field was restored immediately after and is present here again, so
    // this suite exercises the restored (real, committed) shape.
    assert!(raw.contains("\"note\""));
}

// ---------------------------------------------------------------------------
// `reference[]` — the permanently-true container added alongside `carryover[]`
// (see `planning/ticket-reference-container-fields`). These are separate from
// `clean_file_is_byte_identical` (:214) on purpose: that test must keep
// passing with zero edits, as the direct proof that a state.json predating
// `reference[]` never grows a stray `"reference": []` key.
// ---------------------------------------------------------------------------

/// A `state.json` with no `reference` key at all must round-trip
/// byte-identically and must NOT gain a `"reference": []` on write — this is
/// the assertion that protects every fleet state.json that predates this
/// field. Deliberately a fresh fixture (not a reuse of
/// `clean_file_is_byte_identical`'s raw string) so that test can keep
/// asserting its own byte-identity claim untouched while this one carries the
/// `reference`-specific proof.
#[test]
fn state_file_without_reference_key_round_trips_and_gains_no_empty_array() {
    let raw = r#"{
        "repo": "bastion",
        "kind": "project",
        "updated": "2026-08-03",
        "note": null,
        "focus": {"now": [], "next": [], "blocked": []},
        "tracks": [],
        "repos": [],
        "cross_repo": [],
        "tiers": [],
        "backlog": [],
        "carryover": []
    }"#;

    let file: StateFile = serde_json::from_str(raw).unwrap();
    assert!(file.reference.is_empty());

    let round: serde_json::Value = serde_json::to_value(&file).unwrap();
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(original, round);

    assert!(
        !round.as_object().unwrap().contains_key("reference"),
        "reference must stay omitted from the output when empty, got: {round:?}"
    );
}

/// A `state.json` carrying a populated `reference[]` round-trips
/// byte-identically, and the typed field deserializes with exactly the
/// values authored — no `clears_when`, `priority`, or `blocks` to check
/// because `Reference` carries none of those fields by design.
#[test]
fn state_file_with_populated_reference_round_trips_byte_identically() {
    let raw = r#"{
        "repo": "hq",
        "kind": "brain",
        "updated": "2026-08-03",
        "note": null,
        "focus": {"now": [], "next": [], "blocked": []},
        "tracks": [],
        "repos": [],
        "cross_repo": [],
        "tiers": [],
        "backlog": [],
        "carryover": [],
        "reference": [
            {
                "slug": "mev:nextest-rule-is-mev-scoped-not-fleet-wide",
                "scope": {"repo": "mev", "tier": null, "cross_repo": null},
                "class": "lesson",
                "text": "The nextest rule is mev-scoped, not fleet-wide.",
                "created": "2026-08-10"
            }
        ]
    }"#;

    let file: StateFile = serde_json::from_str(raw).unwrap();
    assert_eq!(file.reference.len(), 1);
    let entry = &file.reference[0];
    assert_eq!(entry.slug, "mev:nextest-rule-is-mev-scoped-not-fleet-wide");
    assert_eq!(entry.class, "lesson");
    assert_eq!(
        entry.text,
        "The nextest rule is mev-scoped, not fleet-wide."
    );
    assert_eq!(entry.created, "2026-08-10");
    assert!(entry.related.is_empty());
    assert_eq!(entry.reviewed, None);

    let round: serde_json::Value = serde_json::to_value(&file).unwrap();
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(original, round);
}

/// Before this ticket, `StateFile` had no typed `reference` field, so any
/// `reference[]` authored into a `state.json` was silently absorbed by the
/// `#[serde(flatten, default)] extra` capture map and round-tripped from
/// there. This proves the transition is clean now that the typed field
/// exists: the same fixture deserializes into `StateFile::reference`, NOT
/// into `extra`, re-serializes identically, and the value appears in
/// exactly one place — never duplicated across both, never silently
/// dropped.
#[test]
fn reference_formerly_absorbed_by_extra_now_deserializes_into_typed_field() {
    let raw = r#"{
        "repo": "hq",
        "kind": "brain",
        "updated": "2026-08-03",
        "note": null,
        "focus": {"now": [], "next": [], "blocked": []},
        "tracks": [],
        "repos": [],
        "cross_repo": [],
        "tiers": [],
        "backlog": [],
        "carryover": [],
        "reference": [
            {
                "slug": "hq:planning-symlink-is-a-vaulted-trap",
                "scope": {"repo": null, "tier": null, "cross_repo": true},
                "class": "trap",
                "text": "planning/ is a symlink into the HQ-tracked vault.",
                "created": "2026-08-01"
            }
        ]
    }"#;

    let file: StateFile = serde_json::from_str(raw).unwrap();

    // Lands in the typed field, with the values as authored.
    assert_eq!(file.reference.len(), 1);
    assert_eq!(
        file.reference[0].slug,
        "hq:planning-symlink-is-a-vaulted-trap"
    );

    // Does NOT also land in `extra` — no duplication across both places.
    assert!(
        !file.extra.contains_key("reference"),
        "reference must not also be captured in `extra` now that a typed field exists: {:?}",
        file.extra
    );

    // Re-serializes identically — nothing dropped, nothing added.
    let round: serde_json::Value = serde_json::to_value(&file).unwrap();
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(original, round);
}

// ---------------------------------------------------------------------------
// Task 4 — round-trip and serialization-shape tests for the carryover
// triage fields (`priority`, `blocks`, `finding_id`) and typed `ClearsWhen`.
// ---------------------------------------------------------------------------

/// `block_closed` WITHOUT `note` round-trips: deserializes to the expected
/// variant/fields, and re-serializes to JSON equal to the input (as a
/// `serde_json::Value`, so key order is irrelevant).
#[test]
fn clears_when_block_closed_without_note_round_trips() {
    let raw = r#"{"type":"block_closed","repo":"base-template","id":"BT.ticket.compilable-task-boundaries"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let predicate: ClearsWhenPredicate = serde_json::from_str(raw).unwrap();
    match &predicate {
        ClearsWhenPredicate::BlockClosed { repo, id, note } => {
            assert_eq!(repo, "base-template");
            assert_eq!(id, "BT.ticket.compilable-task-boundaries");
            assert_eq!(*note, None);
        }
        other => panic!("expected BlockClosed, got {other:?}"),
    }

    let round = serde_json::to_value(&predicate).unwrap();
    assert_eq!(original, round);
}

/// `block_closed` WITH `note` round-trips.
#[test]
fn clears_when_block_closed_with_note_round_trips() {
    let raw = r#"{"type":"block_closed","repo":"base-template","id":"BT.ticket.compilable-task-boundaries","note":"waiting on the parent block"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let predicate: ClearsWhenPredicate = serde_json::from_str(raw).unwrap();
    match &predicate {
        ClearsWhenPredicate::BlockClosed { repo, id, note } => {
            assert_eq!(repo, "base-template");
            assert_eq!(id, "BT.ticket.compilable-task-boundaries");
            assert_eq!(note.as_deref(), Some("waiting on the parent block"));
        }
        other => panic!("expected BlockClosed, got {other:?}"),
    }

    let round = serde_json::to_value(&predicate).unwrap();
    assert_eq!(original, round);
}

/// `file_exists` round-trips.
#[test]
fn clears_when_file_exists_round_trips() {
    let raw = r#"{"type":"file_exists","path":"docs/state/state-schema.md"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let predicate: ClearsWhenPredicate = serde_json::from_str(raw).unwrap();
    match &predicate {
        ClearsWhenPredicate::FileExists { path, note } => {
            assert_eq!(path, "docs/state/state-schema.md");
            assert_eq!(*note, None);
        }
        other => panic!("expected FileExists, got {other:?}"),
    }

    let round = serde_json::to_value(&predicate).unwrap();
    assert_eq!(original, round);
}

/// `file_contains` round-trips.
#[test]
fn clears_when_file_contains_round_trips() {
    let raw = r#"{"type":"file_contains","path":"src/state.rs","pattern":"pub struct Carryover"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let predicate: ClearsWhenPredicate = serde_json::from_str(raw).unwrap();
    match &predicate {
        ClearsWhenPredicate::FileContains {
            path,
            pattern,
            note,
        } => {
            assert_eq!(path, "src/state.rs");
            assert_eq!(pattern, "pub struct Carryover");
            assert_eq!(*note, None);
        }
        other => panic!("expected FileContains, got {other:?}"),
    }

    let round = serde_json::to_value(&predicate).unwrap();
    assert_eq!(original, round);
}

/// `command_exits_zero` round-trips.
#[test]
fn clears_when_command_exits_zero_round_trips() {
    let raw = r#"{"type":"command_exits_zero","command":"cargo test --test state_preservation"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let predicate: ClearsWhenPredicate = serde_json::from_str(raw).unwrap();
    match &predicate {
        ClearsWhenPredicate::CommandExitsZero { command, note } => {
            assert_eq!(command, "cargo test --test state_preservation");
            assert_eq!(*note, None);
        }
        other => panic!("expected CommandExitsZero, got {other:?}"),
    }

    let round = serde_json::to_value(&predicate).unwrap();
    assert_eq!(original, round);
}

/// Regression guard for the untagged variant ordering (task 1's load-bearing
/// constraint): a prose `clears_when` string re-serializes as a bare JSON
/// string, NOT as a wrapper object like `{"Prose":"..."}`.
#[test]
fn clears_when_prose_round_trips_as_bare_string() {
    let raw = r#""when the nextest hook actually fires in this repo""#;
    let value: ClearsWhen = serde_json::from_str(raw).unwrap();
    assert_eq!(
        value,
        ClearsWhen::Prose("when the nextest hook actually fires in this repo".to_string())
    );

    let round = serde_json::to_value(&value).unwrap();
    assert_eq!(
        round,
        serde_json::Value::String("when the nextest hook actually fires in this repo".to_string())
    );
    assert!(
        round.is_string(),
        "prose must serialize as a bare string, not a wrapper object: {round:?}"
    );
}

/// A typed predicate parses to `ClearsWhen::Predicate`, not `ClearsWhen::Prose`
/// — the untagged enum picks the right variant for an object input.
#[test]
fn clears_when_predicate_round_trips_through_the_untagged_wrapper() {
    let raw = r#"{"type":"block_closed","repo":"base-template","id":"BT.ticket.compilable-task-boundaries"}"#;
    let value: ClearsWhen = serde_json::from_str(raw).unwrap();
    match &value {
        ClearsWhen::Predicate(ClearsWhenPredicate::BlockClosed { repo, id, note }) => {
            assert_eq!(repo, "base-template");
            assert_eq!(id, "BT.ticket.compilable-task-boundaries");
            assert_eq!(*note, None);
        }
        other => panic!("expected Predicate(BlockClosed), got {other:?}"),
    }

    let original: serde_json::Value = serde_json::from_str(raw).unwrap();
    let round = serde_json::to_value(&value).unwrap();
    assert_eq!(original, round);
}

/// An unknown predicate `type` fails to deserialize rather than silently
/// falling back to `ClearsWhen::Prose` — pins the "no `#[serde(other)]`"
/// constraint from task 1's `ClearsWhenPredicate` doc comment. A typo in a
/// hand-authored `clears_when` object must surface as a parse error, not
/// get silently swallowed by the untagged enum trying the `Prose` variant
/// (which would fail too, since the input is an object, not a string) or by
/// a permissive predicate variant absorbing it.
#[test]
fn clears_when_unknown_predicate_type_fails_to_deserialize() {
    let raw = r#"{"type":"nonsense"}"#;
    let result: Result<ClearsWhenPredicate, _> = serde_json::from_str(raw);
    assert!(
        result.is_err(),
        "an unknown `type` must fail to parse as ClearsWhenPredicate, got: {result:?}"
    );

    // Also confirm the untagged ClearsWhen wrapper fails the same way (an
    // object input can't become Prose, and can't become a Predicate either).
    let wrapped: Result<ClearsWhen, _> = serde_json::from_str(raw);
    assert!(
        wrapped.is_err(),
        "an unknown `type` must fail to parse as ClearsWhen, got: {wrapped:?}"
    );
}

/// `priority` / `blocks` / `finding_id` round-trip unchanged: a `blocks`
/// array carrying one `block` edge and one `external` edge, alongside a
/// `priority` and a `finding_id`, survives deserialize -> serialize.
#[test]
fn carryover_triage_fields_round_trip() {
    let raw = r#"{
        "slug": "ba15-12-mev-context-seed",
        "scope": {"repo": "bastion", "tier": null, "cross_repo": null},
        "kind": "deferred",
        "text": "seed mev context",
        "related": [],
        "priority": 1,
        "blocks": [
            {"type": "block", "repo": "mev", "id": "MV.2.A", "what": null},
            {"type": "external", "what": "blocks every ticket run fleet-wide"}
        ],
        "finding_id": "shared-finding-42",
        "clears_when": null,
        "created": "2026-06-20"
    }"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let carryover: Carryover = serde_json::from_str(raw).unwrap();
    assert_eq!(carryover.priority, Some(1));
    assert_eq!(carryover.blocks.len(), 2);
    match &carryover.blocks[0] {
        BlockedBy::Block(BlockDep { repo, id, what }) => {
            assert_eq!(repo, "mev");
            assert_eq!(id, "MV.2.A");
            assert_eq!(*what, None);
        }
        other => panic!("expected Block, got {other:?}"),
    }
    match &carryover.blocks[1] {
        BlockedBy::External(ExternalDep { what }) => {
            assert_eq!(what, "blocks every ticket run fleet-wide");
        }
        other => panic!("expected External, got {other:?}"),
    }
    assert_eq!(carryover.finding_id.as_deref(), Some("shared-finding-42"));

    let round = serde_json::to_value(&carryover).unwrap();
    assert_eq!(original, round);
}

/// Backward compatibility: a legacy carryover JSON object with NONE of the
/// new keys (`priority`, `blocks`, `finding_id`) deserializes fine and
/// re-serializes byte-identically — the guarantee for the 142 live entries
/// authored before this block.
#[test]
fn carryover_without_new_keys_round_trips_byte_identically() {
    let raw = r#"{
        "slug": "ba15-12-mev-context-seed",
        "scope": {"repo": "bastion", "tier": null, "cross_repo": null},
        "kind": "deferred",
        "text": "seed mev context",
        "related": [],
        "clears_when": "when the nextest hook actually fires in this repo",
        "created": "2026-06-20"
    }"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let carryover: Carryover = serde_json::from_str(raw).unwrap();
    assert_eq!(carryover.priority, None);
    assert!(carryover.blocks.is_empty());
    assert_eq!(carryover.finding_id, None);

    let round = serde_json::to_value(&carryover).unwrap();
    assert_eq!(original, round);

    let round_obj = round.as_object().unwrap();
    assert!(
        !round_obj.contains_key("priority"),
        "priority must stay omitted when absent from the input"
    );
    assert!(
        !round_obj.contains_key("blocks"),
        "blocks must stay omitted when absent from the input"
    );
    assert!(
        !round_obj.contains_key("finding_id"),
        "finding_id must stay omitted when absent from the input"
    );
}

/// Shared operator identity across two unrelated blocks.
///
/// `tests/fixtures/operator-shared-slug.json` carries two blocks in the same
/// track, each with an `operator` edge in `depends_on[]` naming the same
/// `slug` (`session-mac-mini`) — one gate covering two blocks is the whole
/// point of giving `Operator`/`Approval` an identity instead of leaving them
/// as anonymous `external` prose. This pins that shared identity survives
/// deserialization: both edges must come back as `BlockedBy::Operator` with
/// equal `slug`s, not merely equal-looking independent structs.
#[test]
fn operator_edges_share_identity_across_two_blocks() {
    let raw = include_str!("fixtures/operator-shared-slug.json");

    let state: StateFile = serde_json::from_str(raw).unwrap();

    assert_eq!(state.tracks.len(), 1);
    let blocks = &state.tracks[0].blocks;
    assert_eq!(blocks.len(), 2);

    let slug_of = |block: &TrackBlock| -> String {
        assert_eq!(block.depends_on.len(), 1);
        match &block.depends_on[0] {
            BlockedBy::Operator(OperatorDep {
                slug, exit, start, ..
            }) => {
                assert_eq!(exit, "planning/mac-mini-session-notes.md exists");
                assert_eq!(start, "bastion sessions new mac-mini");
                slug.clone()
            }
            other => panic!("expected Operator, got {other:?}"),
        }
    };

    let slug_a = slug_of(&blocks[0]);
    let slug_b = slug_of(&blocks[1]);

    assert_eq!(blocks[0].id, "OK.1.A");
    assert_eq!(blocks[1].id, "OK.1.B");
    assert_eq!(slug_a, "session-mac-mini");
    assert_eq!(
        slug_a, slug_b,
        "the same operator gate must carry an identical slug on every block it covers"
    );
}

// ---------------------------------------------------------------------------
// `BlockedBy` newtype-variant byte-identity round trips (task 2 of
// ticket-blockedby-typeshare-export).
//
// Task 1 rewrote `BlockedBy`'s four variants from inline bodies to newtypes
// over named payload structs, on the empirically-verified claim that serde
// serializes a newtype-of-struct variant flat under internal tagging, so the
// wire shape is unchanged. These tests are that claim's proof: each asserts
// BYTE EQUALITY of the re-serialized value against the original `serde_json`
// `Value`, not merely that parsing succeeds — a silently different output
// (e.g. a reordered or renamed field) would still parse fine, and that is
// the exact failure this refactor risks.
// ---------------------------------------------------------------------------

/// A cross-repo `block` dependency WITHOUT an optional `what` gloss.
#[test]
fn blockdep_without_what_round_trips_byte_identically() {
    let raw = r#"{"type":"block","repo":"okf-core","id":"OK.1.A","what":null}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let dep: BlockedBy = serde_json::from_str(raw).unwrap();
    match &dep {
        BlockedBy::Block(BlockDep { repo, id, what }) => {
            assert_eq!(repo, "okf-core");
            assert_eq!(id, "OK.1.A");
            assert_eq!(*what, None);
        }
        other => panic!("expected Block, got {other:?}"),
    }

    let round = serde_json::to_value(&dep).unwrap();
    assert_eq!(original, round);
}

/// A cross-repo `block` dependency WITH the optional `what` gloss present.
#[test]
fn blockdep_with_what_round_trips_byte_identically() {
    let raw =
        r#"{"type":"block","repo":"mev","id":"MV.2.A","what":"needed for the graph builder"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let dep: BlockedBy = serde_json::from_str(raw).unwrap();
    match &dep {
        BlockedBy::Block(BlockDep { repo, id, what }) => {
            assert_eq!(repo, "mev");
            assert_eq!(id, "MV.2.A");
            assert_eq!(what.as_deref(), Some("needed for the graph builder"));
        }
        other => panic!("expected Block, got {other:?}"),
    }

    let round = serde_json::to_value(&dep).unwrap();
    assert_eq!(original, round);
}

/// An `external` dependency.
#[test]
fn externaldep_round_trips_byte_identically() {
    let raw = r#"{"type":"external","what":"blocks every ticket run fleet-wide"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let dep: BlockedBy = serde_json::from_str(raw).unwrap();
    match &dep {
        BlockedBy::External(ExternalDep { what }) => {
            assert_eq!(what, "blocks every ticket run fleet-wide");
        }
        other => panic!("expected External, got {other:?}"),
    }

    let round = serde_json::to_value(&dep).unwrap();
    assert_eq!(original, round);
}

/// A four-field `operator` gate — all of `slug`, `exit`, `start`, and the
/// optional `what` present.
#[test]
fn operatordep_round_trips_byte_identically() {
    let raw = r#"{"type":"operator","slug":"session-mac-mini","exit":"planning/mac-mini-session-notes.md exists","start":"bastion sessions new mac-mini","what":"confirm the launchd restart"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let dep: BlockedBy = serde_json::from_str(raw).unwrap();
    match &dep {
        BlockedBy::Operator(OperatorDep {
            slug,
            exit,
            start,
            what,
        }) => {
            assert_eq!(slug, "session-mac-mini");
            assert_eq!(exit, "planning/mac-mini-session-notes.md exists");
            assert_eq!(start, "bastion sessions new mac-mini");
            assert_eq!(what.as_deref(), Some("confirm the launchd restart"));
        }
        other => panic!("expected Operator, got {other:?}"),
    }

    let round = serde_json::to_value(&dep).unwrap();
    assert_eq!(original, round);
}

/// An `approval` gate carrying its digest.
#[test]
fn approvaldep_round_trips_byte_identically() {
    let raw =
        r#"{"type":"approval","slug":"ship-it","what":"Ship the change","digest":"sha256:abc123"}"#;
    let original: serde_json::Value = serde_json::from_str(raw).unwrap();

    let dep: BlockedBy = serde_json::from_str(raw).unwrap();
    match &dep {
        BlockedBy::Approval(ApprovalDep { slug, what, digest }) => {
            assert_eq!(slug, "ship-it");
            assert_eq!(what, "Ship the change");
            assert_eq!(digest, "sha256:abc123");
        }
        other => panic!("expected Approval, got {other:?}"),
    }

    let round = serde_json::to_value(&dep).unwrap();
    assert_eq!(original, round);
}
