---
type: Log
title: okf-core Log
description: Chronological log of work and decisions for okf-core.
doc_id: log
layer: [brain, factory]
project: okf-core
status: active
keywords: [log, okf-core]
timestamp: "2026-08-06T21:24:36-03:00"
---

# Log

## [2026-08-06]

### OK.ticket.harness-json-all-targets-clippy — the repo's first harness.json

- **What:** Authored `planning/ticket-harness-json-all-targets-clippy/` and ran it with `/sdlc-task`
  in place on `main` — 5/5 tasks PASS, all first attempt. Added `planning/harness.json` with four
  gated checks (`fmt`, `clippy --all-targets -- -D warnings`, `cargo test` authoritative,
  `cargo build --release` at `perTask: false`). Corrected the stale narrow clippy command in
  `AGENT.md`, `README.md`, and `docs/architecture.md` (`9bfafad`), and in HQ's canonical
  `hooks/README.md` (HQ `8f9573c1`). Verified the gates independently: all four green, 194 tests.
  No `src/` or `tests/` change.
- **Why:** okf-core was the last of 19 repos without a `harness.json`, so its gates lived only as
  prose and its `hooks/pre-push` stage 2 skipped entirely. The documented lint gate was the narrow
  `cargo clippy`, which lints no test code — and both blocks of the 2026-08-03 run were almost
  entirely test code, so the gate never saw what those runs changed. Proven rather than assumed: a
  `needless_return` probe in `tests/support/struct_probe.rs` made narrow clippy exit 0 and
  `--all-targets` exit 101; probe reverted, tree clean.
- **Refs:** substrate lane C3 of `planning/demand-ready/roadmap.md`;
  `planning/ticket-harness-json-all-targets-clippy/tasks.md`

## [2026-08-04]

### OK.ticket.struct-field-default-derive — Default contract + fleet-wide consumer migration

- **What:** Authored the spec (`planning/ticket-struct-field-default-derive/`) and ran it with
  `/sdlc-task` in place on `main` — 4/4 tasks PASS, commits `2e93d4e`, `4e5598f`, `b4147ff`.
  `#[derive(Default)]` added to `TrackBlock`, `Track`, `Epic`, `Carryover`, `StateFile` in
  `src/state.rs`, plus `CarryoverScope` as the prerequisite (`Carryover.scope` is a non-`Option`
  `CarryoverScope`); `Backlog` already derived it and was left alone. Derive-only — no field type
  or serde attribute changed. New `tests/struct_defaults.rs` (18 tests) is the compile-time guard;
  `docs/architecture.md` records the contract. Suite 176 → 194 tests, all four gates green.
  Verified the engine's PASS independently: the OK.3.A/OK.3.B gate files
  (`schema_conformance.rs`, `struct_probe.rs`, `state_preservation.rs`) and `Cargo.toml` have an
  empty diff, so the change is provably non-weakening, and a temporary probe field added to
  `TrackBlock` compiled all targets with **0 × `E0063`** where the same change shape previously
  produced 31 in bastion and 101 in mev. Consumers migrated in parallel lanes the same day:
  exhaustive `extra: Default::default()` sites went mev 101 → 0 and bastion 31 → 0, replaced by
  the `..Default::default()` spread.
- **Why:** Third occurrence of one defect class — adding a field to a shared, non-`#[non_exhaustive]`
  okf-core struct breaks every downstream exhaustive struct literal, every time (`Epic`+`weight` →
  bastion in mev Phase 11; `extra` → mev; `extra` → bastion). The break is invisible to
  `cargo build` because only test code constructs these literals, so a consumer CI that builds but
  does not test reports all-clear while its whole test suite is uncompilable. Two incidents were
  bad luck; three is a pattern, and the fix belongs upstream where the structs live.
- **Refs:** carryover C-2 in `planning/orchestrate-2026-08-03/notes.md` (option 1 of 2 —
  `#[derive(Default)]`, not `#[non_exhaustive]` + builders); HQ backlog
  `okf-core-struct-field-additions-break-consumers`; resolves C-1 (bastion test targets) as a side
  effect. New `carryover[]` entry
  `okf-core-default-contract-needs-spread-literals-downstream` records that the derive only helps
  consumers that actually use the spread form.

## [run: 2026-08-03]

Completed OK.3.B — whole-object state preservation — via `/sdlc-flow 3.2-whole-object-state-preservation`,
resuming after the earlier bail. Task 1 (already landed pre-bail, commit `c54d261`) added a
`#[serde(flatten, default)] extra: serde_json::Map<String, Value>` capture field to the six authored
`state.json` structs (`StateFile`, `Track`, `TrackBlock`, `Backlog`, `Epic`, `Carryover`) plus a
test-only `HasExtra` trait so the OK.3.A `struct_has_field` conformance probe distinguishes typed
fields from the flatten capture map, re-arming the schema/struct conformance gate. Task 2 fixed
`load_state_roundtrip_real_fixture` / `load_state_brain_fixture`, which had been comparing the model
against itself, to assert original-vs-round `serde_json::Value` equality — closing the blind spot that
let the historical `note`-drop bug pass unnoticed — and rewrote both fixtures to be field-complete so
the new equality check holds without weakening `skip_serializing_if` semantics. Task 3 added
`tests/state_preservation.rs`, proving the whole-object property directly: unmodeled fields at every
authored level round-trip byte for byte, derived views still drop unknown keys, clean files stay byte
identical, and `note` survives via the capture map even with its typed field manually removed. Task 4
documented the invariant, the authored/derived asymmetry, and its interaction with the OK.3.A probe in
`docs/architecture.md`. Task 5 validated the full tree: `cargo fmt --check`, `cargo clippy -- -D
warnings`, `cargo test` (176 tests, incl. schema conformance + state preservation), and `cargo build
--release` all green. Review verdict: PASS. Notable decision: the original spec's task split (capture
in task 1, probe fix in task 4) could never pass `cargo test`'s per-task gate on its own, since the
capture disarms the OK.3.A probe the instant it exists — the spec was re-sequenced (task 1 broadened to
include the probe fix, former tasks 2/3/5/6 renumbered to 2/3/4/5) rather than retried as originally
authored; this is recorded in the spec's own Amendment Log from the earlier bailed session. Block
OK.3.B flipped to closed in `planning/state.json`. Next: no further okf-core block is currently defined
after OK.3.B — check `planning/master-plan.md` for what comes next when the roadmap is extended.

```
e24684f docs: document the whole-object state preservation invariant
8306c0e feat: implement 3.2-whole-object-state-preservation-task3
6eff21c feat: implement 3.2-whole-object-state-preservation-task2
98083d9 feat: implement 3.2-whole-object-state-preservation-task1
653035a chore: wrap up 3.2-whole-object-state-preservation
```

## [run: 2026-08-03]

Attempted OK.3.B — whole-object state preservation — via `/sdlc-flow 3.2-whole-object-state-preservation`
and BAILED after task 1. Task 1 added `#[serde(flatten, default)] extra: serde_json::Map<String, Value>`
capture fields to the six authored `state.json` structs (`StateFile`, `Track`, `TrackBlock`, `Backlog`,
`Epic`, `Carryover`), plus doc comments on `Focus` (derived-exclusion rationale) and `TrackBlock.extra`
(incident context), giving them the whole-object round-trip preservation property; `cargo fmt --check`,
`cargo clippy -- -D warnings`, `cargo build`, and all 141 `src/state.rs` unit tests passed. The run then
BAILED: `cargo test` failed two `tests/schema_conformance.rs` tests
(`unknown_field_is_absent_on_track_block`, `derived_fields_are_exempt`) — not a task-1 defect but the
exact interaction task 4's own description predicts verbatim ("With a flatten capture map in place,
EVERY field now survives ... the probe would report every documented field as present ... fix it in
tests/support/struct_probe.rs"). Task 1 is scoped to `src/state.rs` only; the correct fix lives in
`tests/support/struct_probe.rs`, task 4's designated file, which depends on tasks 2-3 completing first
per the tasks.json dependsOn chain 1->2->3->4. Retrying task 1 cannot close this failure without an
out-of-scope, out-of-order edit — the pipeline needs to proceed task-by-task (or a human needs to
re-sequence how/when `cargo test` is gated) rather than another task-1 fix attempt. No commits beyond
task 1 were made; no block status was flipped. Next: resume `/sdlc-flow 3.2-whole-object-state-preservation`
from task 2, letting tasks 2-4 land in order so the struct_probe.rs fix arrives in its designated task.

```
c54d261 feat: implement 3.2-whole-object-state-preservation-task1
076a1f7 chore: init worktree 3.2-whole-object-state-preservation-flow
```

## [run: 2026-08-03]

Completed OK.3.A — the schema-to-struct conformance check — across tasks 1-6 of
`/sdlc-flow 3.1-schema-struct-conformance`, ending in a PASS review. Task 1 added
`tests/support/schema_doc.rs` (`locate_schema_doc()`/`read_schema_doc()`), a worktree-safe upward
search from `CARGO_MANIFEST_DIR` for `docs/state/state-schema.md` with an `OKF_STATE_SCHEMA_DOC` env
override and fail-closed panic if the doc isn't found. Task 2 added `tests/support/schema_parse.rs`,
a std-only markdown parser that extracts `DocumentedField` rows from the schema doc's pipe tables
(skipping JSON template fences) and the authored-vs-derived field set. Task 3 added
`tests/support/struct_probe.rs`, a serde round-trip probe (`struct_has_field<T>`) that detects
whether a struct carries a named field — the exact mechanism behind the historical `TrackBlock.note`
data-loss bug. Task 4 wired these into `tests/schema_conformance.rs`, the cargo-test gate that fails
when a documented authored field (Block vocabulary / `backlog[]` / `epics[]` / `carryover[]`) has no
corresponding struct field on `TrackBlock`/`Backlog`/`Epic`/`Carryover`, with a derived-field
exemption test; it also fixed a false-positive in the task-3 probe where an empty `[]` synthetic
array round-tripped invisibly through `skip_serializing_if = "Vec::is_empty"` fields. Task 5
documented the gate in `docs/architecture.md`. Task 6 ran the full validation suite (fmt, clippy -D
warnings, test, build --release) over the integrated tree, confirming no new dependency and no new
I/O in `src/`. Notable decisions: source of truth is the pipe tables, not the JSON template fences
(the fences omit `note` entirely and would have stayed green through the live bug); manual
note-deletion verification of the failure message was performed via a temporary `src/state.rs` edit,
confirmed byte-identical restore, never committed. Next: OK.3.B — append-only, whole-object writes
for the unattended state.json/OKF cron writers, now unblocked by this gate.

```
ffe9179 feat: implement 3.1-schema-struct-conformance-task5
03b95f5 feat: implement 3.1-schema-struct-conformance-task4
a9a742f feat: implement 3.1-schema-struct-conformance-task3
3e88533 feat: implement 3.1-schema-struct-conformance-task2
357a808 feat: implement 3.1-schema-struct-conformance-task1
a1235d4 chore: init worktree 3.1-schema-struct-conformance-flow
```

## [run: 2026-07-27]

Completed OK.2.A — the typed `BrainDocModel` layer + nested-frontmatter serializer/parser — across
tasks 1-7 of `/sdlc-flow 2.1-brain-doc-model-layer`, ending in a PASS review. Task 1 added a nested
`FrontmatterValue` model (Scalar/InlineList/BlockList/MapList) and `serialize_nested_frontmatter`,
reusing `frontmatter.rs`'s quoting policy. Task 2 added `parse_nested_frontmatter`, the read half of
the round-trip, with a `NestedParseError` carrying source line numbers. Task 3 added the generic
`BrainDocModel` trait, a sentinel-delimited `BodySpec`/`BodySection` renderer byte-compatible with
mev's `splice_generated`, `IndexIntent`, `render_document()`, and a pure `derive_slug()`. Task 4
implemented the full `Opportunity` model (Contact/Action structs, three constructors) matching the
live `business/docs/opportunities/index.md` contract. Task 5 sketched `LearningArtifact` and
`Proposal` to prove the abstraction generalizes beyond Opportunity, needing no changes to the core
traits. Task 6 added a fixture-based integration suite (`tests/doc_roundtrip.rs`) verifying
parse->serialize->parse fidelity, idempotence, and mev-compatible generated-body sentinels across
all three models. Task 7 confirmed fmt/clippy/`cargo test` (141 tests)/mev cargo-check all pass with
no new dependencies. Notable decisions: quoting/comma-splitting for nested map-list values was
reused as-is rather than extended; the Opportunity research brief renders as a plain `## Research
Brief` + fenced-json block (not a sentinel-wrapped Generated section) to byte-match the live
anthropic.md fixture. Next: fold OK.2.A into `planning/master-plan.md` (still ends at Phase 1) —
a separate brain-side edit per the spec's seam-pointer rule.

```
c2edc27 docs: update docs for 2.1-brain-doc-model-layer
b97dbe3 feat: implement 2.1-brain-doc-model-layer-task6
1f35d46 feat: implement 2.1-brain-doc-model-layer-task5
d30f125 feat: implement 2.1-brain-doc-model-layer-task4
5b79a81 feat: implement 2.1-brain-doc-model-layer-task3
2592470 feat: implement 2.1-brain-doc-model-layer-task2
cc23d63 feat: implement 2.1-brain-doc-model-layer-task1
```

## [2026-07-05]

### Added priority and due fields to Block and TrackBlock
- **What:** Added `priority: Option<u8>`, `due: Option<String>`, `sdlc_workflow: Option<String>`, and `model: Option<String>` to `TrackBlock` and `priority`/`due` to `Block` in `src/state.rs`. Added a roundtrip serialization test for fidelity and backwards-compatibility.
- **Why:** The upstream `mev` consumer needs these fields present in the state model to build out its scheduling and routing capabilities (`MV.6.A`).
- **Refs:** [OK.1.A](planning/1.1-block-shape-fields/tasks.md)
