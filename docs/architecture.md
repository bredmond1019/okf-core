---
type: Guideline
title: okf-core architecture
description: Module map, key types, and data flow for the okf-core crate — the OKF frontmatter, brain-graph, state-graph, and typed brain-document contracts.
doc_id: okf-core-architecture
layer: [brain, factory]
project: okf-core
status: active
keywords: [okf-core, OKF frontmatter, brain graph, state.json, BrainDocModel, Rust]
related: [okf-core]
---

# okf-core architecture

`okf-core` is a pure, no-I/O Rust leaf library (only `serde`, `serde_json`, `thiserror`) that
owns four brain contracts as a single source of truth, consumed by sibling crates (`bastion`,
`mev`) via path dependency inside the `core/` Cargo workspace.

## Module map

| Module | File(s) | Owns |
|---|---|---|
| `frontmatter` | `src/frontmatter.rs` | `OkfFrontmatter`, `serialize_frontmatter` — the flat OKF frontmatter write path (D27: 3 required + 6 optional fields) |
| `parse` | `src/parse.rs` | `Frontmatter`, `ParseResult`, `extract_frontmatter`, `parse_frontmatter` — the flat frontmatter read path |
| `graph` | `src/graph.rs` | `Edge`, `EdgeKind`, `Graph`, `Node`, `GraphArtifact`, `resolve_edge` — the brain structural graph model |
| `graph_emit` | `src/graph_emit.rs` | `ExportedEdge`, `GraphExport`, `build_graph_export` — graph export for `mev emit-graph` |
| `state` | `src/state.rs` | `StateFile`, `StateGraph`, `Epic`, `Focus`, `Block`, `Track`, `Backlog`, `Carryover`, `CarryoverKind` (+ `KnownCarryoverKind`), `CarryoverArchiveRow`, `DisposalReason`, `AmendsRef`, `Reference`, `ClearsWhen`, `ClearsWhenPredicate`, `BlockedBy` (+ its payload structs `BlockDep`, `ExternalDep`, `OperatorDep`, `ApprovalDep`), `load_state`, `build_state_graph` — the `planning/state.json` schema and its derived graph |
| `doc` | `src/doc/*.rs` | The typed **brain-document** layer — see below |

All public items are re-exported flat from `src/lib.rs`; consumers import everything as
`okf_core::<Name>` regardless of which internal module defines it.

## The `doc` module — typed brain documents

`src/doc/` is the newest layer (per decision D53): a generic, shape-agnostic abstraction for
typed brain documents that sit *above* the flat `OkfFrontmatter` model. It adds nested
frontmatter shapes (block lists, inline-map lists) that the flat parser/serializer cannot
represent, plus a sentinel-delimited body renderer and a declarative `index.md` registration
intent. `okf-core` never performs file I/O anywhere in this module — reconciling a rendered
document onto disk (including `index.md` writes) is mev's job (`MV.9.A`).

### `frontmatter_value` — nested frontmatter value model + write path

`src/doc/frontmatter_value.rs`

- `FrontmatterValue` — an enum covering the four frontmatter value shapes real brain docs use:
  - `Scalar(String)` — `key: value` (or bare `key:` when empty)
  - `InlineList(Vec<String>)` — `key: [a, b, c]`
  - `BlockList(Vec<String>)` — `key:` + one `  - <item>` line per entry (e.g. `links:`)
  - `MapList(Vec<Vec<(String, String)>>)` — `key:` + one `  - { k: v, … }` line per entry
    (e.g. `contacts:` / `actions:`)
- `serialize_nested_frontmatter(fields: &[(String, FrontmatterValue)]) -> String` — serializes an
  ordered field list into a `---`-fenced YAML block. Fields render exactly in the given order;
  the caller decides which optional fields to include (mirroring `serialize_frontmatter`'s
  omission behavior). Empty `BlockList`/`MapList` still render `key: []`.
- Reuses `crate::frontmatter`'s exact quoting policy (`needs_quote` / `yaml_scalar`, widened to
  `pub(crate)`) so a value quotes identically whether it appears in a flat scalar, an inline
  list, or an inline-map value. Map-list *keys* (`at`, `kind`, `note`, …) are never quoted —
  only values are, since keys are trusted field names.

### `parse_nested` — nested frontmatter read path

`src/doc/parse_nested.rs`

- `parse_nested_frontmatter(content: &str) -> Result<Vec<(String, FrontmatterValue)>, NestedParseError>`
  — recovers all four `FrontmatterValue` shapes from source text (the read half of the
  round-trip with `serialize_nested_frontmatter`). Key order in the source is preserved.
- `NestedParseError` — `UnterminatedFence { open_line }`, `MalformedLine { source_line }`,
  `MalformedInlineMap { source_line }`, `UnterminatedQuote { source_line }`. Line numbers are
  1-based, mirroring `crate::parse::ParseResult::MalformedLine`'s convention.
- `src/parse.rs` (the flat parser) is untouched — this module is purely additive and does not
  change `extract_frontmatter`/`parse_frontmatter`'s behavior or signatures.

### `model` — the `BrainDocModel` trait + body/index-intent types

`src/doc/model.rs`

- `trait BrainDocModel` — the generic contract every typed brain document implements:
  - `frontmatter(&self) -> Vec<(String, FrontmatterValue)>`
  - `body(&self) -> BodySpec`
  - `slug(&self) -> String`
  - `index_intent(&self) -> IndexIntent`
  - `doc_type(&self) -> &'static str` — a stable dispatch key (e.g. `"opportunity"`), used by a
    downstream materializer without matching on the concrete Rust type
- `BodySection` — `Verbatim(String)` (emitted as-is) or `Generated { marker, content }` (wrapped
  in mev-compatible sentinels: `<!-- BEGIN generated:{marker} -->` / `<!-- END generated:{marker} -->`,
  byte-compatible with mev's `splice_generated` in `src/brain/emit.rs`).
- `BodySpec { sections: Vec<BodySection> }` with `render() -> String` — renders every section in
  order; each section gets exactly one trailing newline.
- `IndexIntent { index_path, link_target, row_cells }` — declares which `index.md` a document
  registers into and what its table row looks like. Intent only; never executed here.
- `render_document(model: &impl BrainDocModel) -> String` — the single deterministic write path
  every concrete model shares: `serialize_nested_frontmatter(model.frontmatter())` followed by
  `model.body().render()`.

### `slug` — title → slug derivation

`src/doc/slug.rs`

- `derive_slug(title: &str) -> String` — pure and total (never panics). Lowercases every
  alphanumeric character (Unicode-aware via `char::to_lowercase`), collapses runs of
  non-alphanumeric characters to a single `-`, trims leading/trailing `-`. Empty or
  all-non-alphanumeric input yields an empty string.

### `opportunity` — the `Opportunity` model

`src/doc/opportunity.rs`

The full `BrainDocModel` for the `business/docs/opportunities/index.md` contract: a candidate
business opportunity captured from a `RESEARCH_AGENT` run, before promotion to a pipeline lead.

- `Opportunity { title, description, doc_id, layer, status, kind, stage, source, url, links,
  last_contact, next_action, research_ref, contacts, actions, body_prose, research_brief }`
  - `doc_id` defaults to `opportunity-<slug>` when unset (via `effective_doc_id`).
  - `kind`/`stage` are deliberately `Option<String>`, not typed enums — `okf-core` is the
    lenient data model in the stack; enforcing the allowed value sets (`kind: company |
    prospecting-sweep | job-posting`, `stage: identified | … | closed-lost`) is mev's job.
    Note that leniency is about **validation**, not about staying untyped: see
    "[Typed-with-fallback](#typed-with-fallback--how-to-type-a-vocabulary-without-a-parse-cliff)"
    below, which is how `Carryover.kind` gained a type without okf-core taking on validation.
  - `research_brief: JsonValue` is embedded verbatim as the first fenced `json` block under a
    `## Research Brief` heading; `Value::Null` (default) omits that section entirely.
- `Contact { name, role, emails, whatsapp, phones, links, note }` — an enriched contact channel;
  list sub-fields are encoded within their `MapList` inline-map string value as `[]` or a
  `, `-joined string.
- `Action { at, kind, note }` — one append-only history entry.
- `OpportunityError::MissingField(&'static str)`.
- Constructors: `from_company_brief(&JsonValue)`, `from_prospecting_result(&JsonValue)` (both
  consume the raw engine-rs `RESEARCH_AGENT` payload verbatim, `kind`/`stage` pre-set to
  `identified`), and `from_frontmatter(&[(String, FrontmatterValue)])` (the read-half
  reconstructor — recovers frontmatter-derived fields only; `body_prose`/`research_brief` stay
  at their defaults after a frontmatter-only round trip).

### `learning_artifact` — the `LearningArtifact` sketch model

`src/doc/learning_artifact.rs`

A sketched `BrainDocModel` over the engine-rs content-pipeline POST payload
(`PersistToBrainNode`, `EN.5.A`): `{artifact_id, channel_type, source_ref, summary,
digest_markdown, entities, language}`. "Sketched" means it compiles, implements the trait, and
round-trips a fixture — not a field-by-field contract the way `Opportunity` is.

- `LearningArtifact { artifact_id, channel_type, source_ref, summary, entities, language,
  digest_markdown }` — `digest_markdown` renders as a `Generated` body section under the
  `"digest"` marker (prose, not a frontmatter scalar/list value).
- `LearningArtifactError::MissingField(&'static str)`.
- `from_payload(&serde_json::Value)` — consumed leniently (missing fields default to empty
  string/list rather than erroring).
- Registers into `docs/content/learning-corpus/index.md` (`LEARNING_CORPUS_INDEX` — a
  placeholder path; that corpus index is not yet a landed contract).

### `proposal` — the `Proposal` sketch model

`src/doc/proposal.rs`

The second sketch model, over the engine-rs `PROPOSAL_GENERATOR` deliverable,
`AutomationRoadmap { situation, candidates, top_profiles, recommendation }`.

- `Proposal { company_name, title, roadmap: JsonValue }` — the roadmap is carried verbatim as a
  pretty-printed fenced-json `Generated` body section (marker `"roadmap"`) rather than mirroring
  `RankedCandidate`/`WorkflowProfile`/`FirstEngagement` as nested Rust structs.
- `ProposalError::MissingField(&'static str)`.
- `from_automation_roadmap(company_name: &str, &JsonValue)`, `from_frontmatter(&[(String,
  FrontmatterValue)])` (recovers `title`/`company_name`; `roadmap` stays `Value::Null` after a
  frontmatter-only round trip).
- Registers into `business/docs/proposals/index.md` (`PROPOSALS_INDEX`).

## Data flow

```
                     ┌─────────────────────────┐
 typed model  ──────▶│  BrainDocModel trait     │
 (Opportunity /       │  .frontmatter() ─┐       │
  LearningArtifact /   │  .body()        │       │
  Proposal)            │  .slug()        │       │
                     └──────────────────┼───────┘
                                        ▼
                          serialize_nested_frontmatter()
                                        +
                                 BodySpec::render()
                                        │
                                        ▼
                              render_document(&model)
                                        │
                                        ▼
                          full document text (frontmatter + body)
                                        │
                     (materializer — mev MV.9.A — writes to disk,
                      reconciles IndexIntent into index.md; no I/O here)

                 read half (round trip):
                 source text ──▶ parse_nested_frontmatter() ──▶ Vec<(String, FrontmatterValue)>
                                                                        │
                                                                        ▼
                                                    Model::from_frontmatter(fields)
```

## Tests

Integration fixture tests live in `tests/doc_roundtrip.rs`, backed by fixtures in
`tests/fixtures/` (`opportunity-anthropic.md` — a byte-for-byte copy of the live
`business/docs/opportunities/anthropic.md`; `learning-artifact.md`; `proposal.md`). They verify
parse→serialize→parse fidelity, idempotence, mev-compatible generated-body sentinels, and that
flat-frontmatter surfaces are unchanged by the nested layer.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Schema/struct conformance gate

`tests/schema_conformance.rs` (support modules in `tests/support/`) closes the silent data-loss
hole that twice deleted authored `TrackBlock.note` values (2026-08-02, 2026-08-03): it fails
`cargo test` when a field documented as **authored** in the brain's
`docs/state/state-schema.md` pipe tables has no corresponding field on the matching
`src/state.rs` struct.

- **Compares:** the brain's `docs/state/state-schema.md` pipe tables (parsed by
  `tests/support/schema_parse.rs`) against `src/state.rs` structs, probed by round-tripping a
  synthetic value through each struct's `Serialize`/`Deserialize` impl
  (`tests/support/struct_probe.rs`) — a field with no struct counterpart is silently dropped by
  serde on deserialize, which is exactly the historical data-loss mechanism, so the probe tests
  the real failure mode rather than a proxy for it. Four sections are checked, matched by
  leading identifier: `Block vocabulary` → `TrackBlock`, `` backlog[] `` → `Backlog`, `` epics[]
  `` → `Epic`, `` carryover[] `` → `Carryover`.
- **Pipe tables, not JSON template fences:** the doc's `tracks[].blocks[]` template fence omits
  `note`/`description` entirely, so a fence-driven check would have been green throughout the
  live bug. Only the `## Block vocabulary (shared, authored)` pipe table declares the full
  14-field set, so that is the sole source the check parses; the fences are ignored.
  `tests/support/schema_parse.rs` skips fenced code blocks while walking the doc.
- **Authored/derived exemption:** `docs/state/state-schema.md`'s `## Authored vs derived`
  section names fields the engine computes (e.g. `tasks`, `focus`) rather than a human writes. A
  documented field with no struct field is only a defect when it is authored — a derived field
  missing from the struct is expected and exempt (`derived_fields_are_exempt` in
  `tests/schema_conformance.rs` is the regression test for this rule).
- **Field-name floor (`OK.ticket.conformance-field-name-floor`):** the section asserts began as
  `!fields.is_empty()` — a floor of *one*. That catches a parser returning nothing; it does not
  catch a parser returning *some*. Because the table match at `tests/support/schema_parse.rs:98` is
  exact string equality on `| Field | Shape | Meaning |`, a doc reformat can drop most of a
  section's rows while one still parses: the assert passed, `check_struct` dutifully verified the
  survivors, and the gate reported green over a silently narrowed scope — the same
  stops-checking-quietly failure this gate exists to eliminate, recurring one layer down. A
  per-section *count* floor replaced those asserts, and a per-section **name** floor has now
  replaced the counts.

  `check_field_name_floor` compares each section's parsed field names against the expected name
  list in `tests/support/schema_floor.rs` (`expected_field_names`), asserting the expected list is
  a **subset** of what parsed. Missing names accumulate into the same `violations` vector
  `check_struct` uses, so one run reports every short section, and each message names the section,
  every missing field, and the floor's origin — a reformatted doc and a deliberately removed field
  have opposite fixes.
  - It is a **floor, not an equality**: a section may document *more* than the list without
    failing. An exact match would turn every honest schema addition red and train people to edit
    the list without reading it.
  - **Legitimate growth needs no edit.** This is why names replaced counts. Under the count floor,
    growth left the number stale and tightening it was a manual four-site edit — `carryover[]`
    growing 9 → 12 cost its own task (`c99d21d`), and this very document went on claiming a floor
    of 9 until the name floor landed. Under the name floor the list changes only on deliberate
    *removal*.
  - **Now caught, which counts missed:** an edit that drops one documented field while adding two
    others. The section's row count *rises*, so a count floor stays green while a real field
    silently stops being checked. `floor_catches_a_drop_masked_by_a_larger_addition` pins this;
    the ticket's Amendment Log records the same fixture passing (exit 0) against the count floor
    and failing (exit 101) against the name floor. A plain row deletion is likewise caught, where
    before it removed the field from the parse entirely and left `check_struct` nothing to iterate.
  - **What it still cannot catch:** a field renamed in the doc, the struct, and the floor list in
    one edit — nothing short of review catches that, and it is visible in the diff, not silent.
    Separately, the floor covers exactly the four sections `section_matches` maps to a struct, so a
    *fifth* authored section appearing in the doc is still unnoticed by any gate.
- **Doc location:** `tests/support/schema_doc.rs` ascends parent directories from
  `CARGO_MANIFEST_DIR` looking for `docs/state/state-schema.md`, so it resolves correctly from
  both the main tree and a `core/okf-core/trees/<name>/` worktree with no hardcoded depth. Set
  `OKF_STATE_SCHEMA_DOC` to an absolute path to override the search for non-standard checkouts
  (e.g. a standalone clone outside the `agentic-portfolio` monorepo workspace); if the doc is
  found by neither the override nor the search, the test panics with an actionable message
  rather than silently skipping. Fixture-driven tests take `read_schema_doc_at(path)` instead of
  setting that env var — env vars are process-global and `cargo test` runs tests as threads in one
  process, so a fixture that set it would race every sibling test.
- All doc-reading I/O lives in `tests/`, never in `src/` — okf-core stays a pure, no-I/O leaf
  library; this gate adds no new entry to `[dependencies]`.

## Whole-object state preservation (authored vs. derived)

The six **authored** `src/state.rs` structs — `StateFile`, `Track`, `TrackBlock`, `Backlog`,
`Epic`, `Carryover` — each carry an unknown-field capture map:

```rust
#[serde(flatten, default)]
pub extra: serde_json::Map<String, serde_json::Value>,
```

This gives them the **whole-object property**: any key present in a `state.json` file that the
struct does not (yet) model rides through a deserialize→serialize round-trip unchanged, landing
in `extra` instead of being silently dropped. A stale binary that has never heard of a field —
because it predates that field, or because the field was accidentally deleted from the struct —
can no longer destroy the value; it merely fails to give it a typed accessor. The property holds
**by construction**, not by auditing every call site that mutates or re-serializes a `StateFile`.

This closes three real incidents, all caused by the same silent-drop mechanism: the dropped
`TrackBlock.note` (2026-08-02); mev's `derive_rollup` dropping a repo from two rollups; and the
2026-08-03 recurrence where a stale binary destroyed 29 authored notes in one unattended
`emit-state --write` run via `scripts/routine.sh` on cron.

### The derived views deliberately do NOT get this

`Focus`, `Block`, `RepoRollup`, and `CrossRepoEdge` are regenerated wholesale on every run — they
have no `extra` capture map, and a stray key on one of them is dropped on round-trip rather than
preserved (`derived_views_do_not_preserve_unknowns` in `tests/state_preservation.rs` pins this).
This asymmetry is deliberate, not an oversight: preserving a stale unknown key on a *derived*
view would **resurrect deleted data** the next time the view is regenerated, which is the
opposite of the safety property this block adds. Authored data is protected by never being
silently dropped; derived data is protected by always being freshly computed. Do not "helpfully"
make this uniform — see the reasoning documented alongside `Focus` in `src/state.rs`.

### Interaction with the OK.3.A schema/struct conformance gate

Before this block, `tests/support/struct_probe.rs`'s `struct_has_field` decided a documented
field was present on a struct by checking whether a synthetic value survived a
deserialize→serialize round-trip. Once the `extra` capture map exists, an *unmodeled* field also
survives that round-trip (it rides in `extra`), so the same probe would report every documented
field as present and `schema_struct_conformance` would pass **vacuously** — quiet, but no longer
actually checking anything.

The fix is a test-only `HasExtra` trait implemented for the six authored structs
(`fn extra(&self) -> &serde_json::Map<String, serde_json::Value>`). `struct_has_field` now checks
`!parsed.extra().contains_key(field)` first: if the probed name shows up in `extra`, the struct
has no typed field for it, full stop, regardless of what the round-trip alone would suggest. **A
future reader who removes the `HasExtra` check (e.g. while "simplifying" the probe back down to a
pure round-trip) will silently make the OK.3.A conformance gate vacuous again** — it will keep
passing while no longer detecting a deleted authored field. If you touch `struct_probe.rs`,
re-run the manual mutation check: delete a field such as `TrackBlock::note`, confirm
`schema_struct_conformance` still FAILS with `` documented field `note` ... has no corresponding
field on struct `TrackBlock` ``, then restore it.

### Scope: model only, not the writer

okf-core owns the **model** half of this property only — the guarantee that a `StateFile` value
in memory round-trips without loss. It does not gain file writing or revision history; that stays
in mev's `emit-state --write` as a follow-on block (append-only writes / never-overwrite-in-place),
which keeps this change inside okf-core's no-I/O, no-path-deps invariant. That mev-side writer is
still outstanding.

### The `Default` contract for the six authored structs

The same six authored structs — `StateFile`, `Track`, `TrackBlock`, `Backlog`, `Epic`, `Carryover`
— along with the `CarryoverScope` enum that `Carryover.scope` requires as a prerequisite, each
derive `#[derive(Default)]`. Downstream consumers **MUST** construct them with
`..Default::default()`, naming only the fields they care about, rather than an exhaustive struct
literal:

```rust
let block = TrackBlock {
    id: "OK.9.A".into(),
    title: "t".into(),
    ..Default::default()
};
```

This is what makes a future field addition to any of the six **non-breaking** for consumer code.
An exhaustive literal must be updated at every construction site the moment a field is added;
`..Default::default()` absorbs the new field silently.

This closes the same defect class three separate times: `Epic` gaining `weight` (mev Phase 11)
broke `bastion`'s exhaustive literals; the `extra` capture map field (OK.3.B) broke `mev` at 101
call sites (`de81724`); the same `extra` field broke `bastion` at 31 call sites (31 ×
`E0063` from `cargo test --no-run`). In all three incidents `cargo build` stayed green throughout
— only test code constructs these literals directly, so a consumer's build-only CI reported
all-clear while its entire test suite was uncompilable. `..Default::default()` removes the defect
class at the call site rather than relying on every consumer to notice.

**Deliberate limits on this contract:**

- `StateFile::default()` yields `repo: ""`, `kind: ""`, `updated: ""` — it is **not** a valid state
  document. The derive exists purely for call-site construction ergonomics, not to produce
  something a validator should accept; do not treat a defaulted `StateFile` as a fixture for
  anything beyond the shape of the type.
- The derived views (`Block`, `RepoRollup`, `CrossRepoEdge`, `TierEntry`, `Endpoint`) are outside
  this contract. They are regenerated wholesale on every run (see above), so there is no
  call-site-literal problem for them to solve.
- `#[non_exhaustive]` plus constructor builders was the stronger alternative considered and
  deliberately not chosen here — larger surface change, tracked separately if ever pursued.

`tests/struct_defaults.rs` is the compile-time guard for this contract: it constructs all six
authored structs via `..Default::default()`, asserts the flattened `extra` map defaults to empty,
and asserts each default value round-trips through serialize→deserialize. Removing a `#[derive(Default)]`
from any of the six will fail that file at compile time rather than silently reopening the defect
class described above.

## Typed-with-fallback — how to type a vocabulary without a parse cliff

Several `state.json` fields hold a small controlled vocabulary. The obvious move is a plain enum,
and it is usually wrong here. **An unknown enum variant aborts deserialization of the entire file**,
not just the offending entry — mev pins this in a test (`core/mev/src/brain/state.rs:3411`,
"unknown blocked_by type in full file should produce `StateLoadError::Parse`"). Since
`mev emit-state` regenerates derived surfaces from these files, one stray value written by any
session blacks out every other check on that file.

The opposite failure is just as bad and quieter: `blocks[].model` accepts four values and silently
normalizes anything else to `"sonnet"` (`core/mev/src/brain/state.rs:821`). That coerces data
without telling anyone.

The shape that avoids both is an `#[serde(untagged)]` wrapper over a known enum plus a `String`
fallback:

```rust
#[serde(untagged)]
pub enum CarryoverKind {
    Known(KnownCarryoverKind),   // defect | deferred | drift | env
    Unknown(String),             // anything else, preserved verbatim
}
```

**Declaration order is load-bearing.** Untagged variants are tried in order, so `Known` must come
first; reversed, every known value falls into `Unknown(String)` — and every round-trip test still
passes, because the string survives either way. That is the failure this note exists to prevent.

What it buys: known values match exhaustively at compile time, an unknown value degrades to a
validation error on *that entry* instead of a parse failure on the whole file, and the original
string round-trips byte-identically rather than being coerced or dropped. okf-core still validates
nothing — the known-vocabulary check stays in mev — so the leniency principle above holds.

`ClearsWhen` uses the same pattern. `Carryover.kind` adopted it in
`OK.ticket.carryover-kind-typed-enum`, which is also why the legacy `constraint` / `known_issue`
values — still on ~131 live entries pending migration — keep loading: they land in `Unknown`.
Verified against the real corpus at the time, not only fixtures: all **51** `state.json` files in
the fleet load with zero failures.

`BlockedBy`, by contrast, *is* a bare enum, and that is a deliberate trade — a dangling dependency
edge should be loud. The cost is on record: adding two variants to it broke every exhaustive match
downstream.

## The optional `typeshare` feature — generating `BlockedBy`'s payload types

`BlockedBy` describes why a block cannot start, and its four reasons (`block`, `external`,
`operator`, `approval`) have to reach the cockpit's TypeScript. **typeshare cannot generate the
enum itself.** It rejects internally-tagged data-carrying enums outright — against typeshare-cli
1.13.4, annotating `BlockedBy` fails with `Serde content attribute needs to be specified for
algebraic enum BlockedBy`. Satisfying it would mean adding `content = "content"`, which rewrites
every dependency entry in every `state.json` in the fleet. That trade was considered and rejected.

So each variant's body is a **named payload struct** the variant wraps as a newtype:

```rust
pub enum BlockedBy {
    Block(BlockDep),
    External(ExternalDep),
    Operator(OperatorDep),
    Approval(ApprovalDep),
}
```

The four structs carry the annotation; the enum stays bare. serde writes a newtype-of-struct
variant **flat** under internal tagging, so **the on-disk and on-the-wire JSON is unchanged** —
pinned by one byte-identity test per variant in `tests/state_preservation.rs`, plus
`clean_file_is_byte_identical`, which must keep passing untouched.

**What downstream gets is four interfaces, not a union.** typeshare emits `BlockDep`,
`ExternalDep`, `OperatorDep`, and `ApprovalDep` — never a type named `BlockedBy`. Consumers still
hand-write the discriminated union, but as a short union over generated interfaces rather than a
full transcription of every field. Anything expecting to export the name `BlockedBy` will find
nothing.

**The dependency is optional and never linked by default.** `Cargo.toml` declares
`typeshare = { version = "1", optional = true }` behind a `typeshare = ["dep:typeshare"]` feature,
applied as `#[cfg_attr(feature = "typeshare", typeshare::typeshare)]`. typeshare-cli parses source
syntactically and never links the crate, so generation works without the feature ever being
enabled: `cargo tree` on a default build shows no `typeshare`. That is why AGENT.md standing rule 3
("no I/O dependencies") holds unchanged.

Regenerate and check with:

```bash
scripts/check-typeshare.sh
```

It regenerates the TypeScript and asserts all four interfaces are present. It is a **developer
convenience, not a gate** — with the CLI absent it prints an install hint and exits 0, so it can
never hard-block a machine that lacks it. It also probes `~/.cargo/bin` before concluding the CLI
is missing, since `cargo install` puts it there and that directory is not on every machine's PATH.

## See also

- [`../README.md`](../README.md) — crate overview, consumers, dependencies
- `business/docs/opportunities/index.md` — the live contract `Opportunity` targets
