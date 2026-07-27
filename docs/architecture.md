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
| `state` | `src/state.rs` | `StateFile`, `StateGraph`, `Epic`, `Focus`, `Block`, `Track`, `Backlog`, `Carryover`, `load_state`, `build_state_graph` — the `planning/state.json` schema and its derived graph |
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
cargo clippy -- -D warnings
cargo test
cargo build --release
```

## See also

- [`../README.md`](../README.md) — crate overview, consumers, dependencies
- `business/docs/opportunities/index.md` — the live contract `Opportunity` targets
