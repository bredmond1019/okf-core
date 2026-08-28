# okf-core

A pure Rust library that defines, once, the shape of a documentation-metadata and work-tracking
contract — so that several independent tools (a validator, a state-graph builder, a document
generator) can read and write the same files without their models silently drifting apart from
each other.

## What this is for

If more than one program needs to parse the same file format, and each program keeps its own copy
of "what that format looks like," the copies eventually disagree — one adds a field the other
doesn't know about, one accepts a value the other rejects. `okf-core` exists so there is exactly
one Rust definition of three related contracts, and everything else depends on it instead of
re-implementing it:

1. **OKF frontmatter** — a YAML metadata block (`type`, `title`, `description`, plus optional
   fields) at the top of a Markdown file. "OKF" stands for **Open Knowledge Format**: a
   lightweight convention for tagging a document with structured metadata that a search or
   validation tool can read without parsing the whole file.
2. **A structural link graph** — nodes are documents, edges are the `related:` references one
   document's frontmatter makes to another. This is how a corpus of Markdown files becomes a
   navigable graph instead of a pile of unlinked text.
3. **A work-tracking graph (`state.json`)** — a JSON file describing units of work ("blocks"),
   their dependencies on each other, durable notes ("carryover" items), and cross-repo links,
   used to drive an automated or semi-automated project-tracking system.

A fourth, smaller layer sits on top: typed models for specific kinds of Markdown documents (a
business lead, a learning summary, a proposal) that round-trip through nested frontmatter and a
generated-content body.

`okf-core` does no file I/O, opens no network connection, and enforces no business policy (it
will happily model a value it considers structurally valid but semantically wrong) — parsing,
writing, validating, and policy decisions all live in consumer binaries. This crate is the shape
those consumers agree on.

## Quickstart

`okf-core` is not published to crates.io — it's consumed as a workspace path dependency or
directly from GitHub:

```toml
[dependencies]
okf-core = { git = "https://github.com/bredmond1019/okf-core" }
```

A minimal round-trip — parse an OKF frontmatter block, then regenerate one:

```rust
use okf_core::{OkfFrontmatter, ParseResult, extract_frontmatter, serialize_frontmatter};

fn main() {
    let doc = "---\ntype: Guide\ntitle: Hello\ndescription: A test doc.\n---\n# Hello\n";

    // Read: pull the frontmatter fields out of a Markdown file's source text.
    match extract_frontmatter(doc) {
        ParseResult::Ok(fm) => {
            println!("title: {}", fm.fields["title"].0);
        }
        other => panic!("unexpected parse result: {other:?}"),
    }

    // Write: build the struct and serialize it back to a canonical `---` block.
    let fm = OkfFrontmatter {
        type_: Some("Guide".into()),
        title: Some("Hello".into()),
        description: Some("A test doc.".into()),
        keywords: vec!["example".into()],
        ..Default::default()
    };
    print!("{}", serialize_frontmatter(&fm));
}
```

Run the crate's own test suite the same way any consumer's CI does:

```bash
cargo test
```

### Prerequisites

| Needed | Why | If missing |
|---|---|---|
| Rust, edition 2024 | `Cargo.toml` pins `edition = "2024"` | Install/update via [rustup](https://rustup.rs/) |
| Nothing else | No I/O deps — only `serde`, `serde_json`, `thiserror` (see [Dependencies](#dependencies)) | — |

## Public API

Every item below is re-exported from the crate root (`use okf_core::...`) — see [`src/lib.rs`](src/lib.rs).

### Flat frontmatter — read and write

| Type / function | What it models |
|---|---|
| `OkfFrontmatter` | The nine OKF fields: `type`, `title`, `description` (required), `doc_id`, `layer`, `project`, `status`, `keywords`, `related`, `synced_from` (all optional) |
| `serialize_frontmatter` | `&OkfFrontmatter -> String` — writes a canonical `---`-fenced YAML block, in fixed field order |
| `Frontmatter`, `ParseResult` | The parsed frontmatter block (field values + their 1-based source line numbers) and the four possible outcomes of parsing |
| `extract_frontmatter`, `parse_frontmatter` | `&str -> ParseResult` / `&str -> Option<Frontmatter>` — the read path |

### Structural link graph

| Type / function | What it models |
|---|---|
| `Node` | One document in the corpus: its canonical id, owning scope, authored `doc_id`, and relative path |
| `Edge`, `EdgeKind` | A directed link from one document to another, sourced from a `related:` frontmatter entry (`EdgeKind::Related` is the only kind today) |
| `Graph` | The full node/edge set — the serializable graph artifact |
| `GraphArtifact` | A built `Graph` plus the lookup tables (`node_map`, `leaf_keys`) `resolve_edge` needs |
| `EdgeResolution`, `resolve_edge` | Resolves one edge's target: `Resolved` (a real node), `LeafTarget` (a file with no `doc_id`), or `Dangling` (nothing found) |
| `GraphExport`, `ExportedEdge`, `build_graph_export` | A JSON-serializable export envelope (`version` + `root` header, nodes, edges annotated with their resolution) for loading the graph into an external store |

### Work-tracking graph (`state.json`)

| Type / function | What it models |
|---|---|
| `StateFile`, `StateLoadError`, `load_state` | The top-level `state.json` schema, its load error (`Io` or `Parse`), and the file loader |
| `Block`, `TrackBlock`, `Track` | A tracked unit of work (two shapes: a lightweight `focus` entry, and the fuller `tracks[].blocks[]` record) and the track (roadmap) that groups them |
| `Focus` | The `focus` object: what's in progress now, next, blocked, or deferred |
| `BlockedBy`, `BlockDep`, `ExternalDep`, `OperatorDep`, `ApprovalDep` | The four shapes a block can depend on: another block, an external/environmental condition, a human operator session, or a single pending approval |
| `Carryover`, `CarryoverKind`, `KnownCarryoverKind`, `CarryoverScope` | A durable caveat or follow-on note, its kind (with an `Unknown(String)` fallback so an unrecognized kind degrades gracefully instead of failing to parse), and where it applies |
| `ClearsWhen`, `ClearsWhenPredicate` | What retires a `Carryover`: free-form prose, or a machine-checkable predicate (`BlockClosed`, `FileExists`, `FileContains`, `CommandExitsZero`) |
| `CarryoverArchiveRow`, `DisposalReason`, `AmendsRef` | One append-only record of a `Carryover` that was disposed, why (`Cleared`/`Superseded`/`Promoted`/`Withdrawn`), and — for a superseding row — which earlier row it corrects |
| `Reference` | A permanently-true fact (a trap, an invariant, a lesson) that structurally can never be "done" — it carries no dependency or priority fields |
| `Epic`, `Backlog`, `BacklogOrigin`, `Origin` | Cross-repo initiative and queued-idea registries |
| `RepoRollup`, `CrossRepoEdge`, `Endpoint`, `TierEntry` | Cross-repo headline caches, explicit dependency edges between repos, and tier pointers |
| `StateSource` | A discovered `state.json` file's identity (repo slug, path, expected `kind`) — the input to `build_state_graph` |
| `StateGraph`, `StateNode`, `StateEdge`, `StateEdgeKind`, `build_state_graph` | The block-dependency graph built by joining every discovered `StateFile`: nodes are registered blocks, edges are `BlockedBy`/`CrossRepo`/`CarryoverBlocks` links |
| `op_id`, `op_slug_stutters`, `normalize_op_slug`, `W_STATE_OP_SLUG_STUTTER` | Helpers for rendering/validating the display identity of an operator- or approval-gated dependency edge |

### Typed brain documents

A generalization on top of the flat frontmatter model, for Markdown documents with nested
frontmatter (lists of maps, not just scalars/inline lists) and a body made of hand-written and
machine-generated sections.

| Type / function | What it models |
|---|---|
| `BrainDocModel` | The trait a concrete document type implements: its frontmatter fields, its body, its derived slug, and how it registers into an index file |
| `BodySpec`, `BodySection` | A document body as an ordered list of `Verbatim` or `Generated` (sentinel-delimited) sections |
| `IndexIntent` | The declarative intent for registering a document into an index file (path, link target, row cells) — no file I/O happens here |
| `render_document` | `&impl BrainDocModel -> String` — the one deterministic write path every concrete model shares |
| `FrontmatterValue`, `serialize_nested_frontmatter` | The nested frontmatter value model (scalars, inline lists, block lists, inline-map lists) and its serializer |
| `parse_nested_frontmatter`, `NestedParseError` | The read half of the nested-frontmatter round-trip |
| `derive_slug` | `&str -> String` — turns a document title into a filesystem-safe slug |
| `Opportunity`, `Contact`, `Action`, `OpportunityError` | A concrete `BrainDocModel`: a candidate business opportunity, its contact channels, and its action history |
| `LearningArtifact`, `LearningArtifactError` | A concrete `BrainDocModel` for a summarized piece of learned content |
| `Proposal`, `ProposalError` | A concrete `BrainDocModel` for a recommendation/roadmap document |

## Error types

`okf-core` has no fixed numeric or string error-code registry (that lives in the consumers that
validate against this shape). Its own fallible entry points return one of these:

| Error | Returned by | Variants |
|---|---|---|
| `ParseResult` | `extract_frontmatter` | `Ok`, `UnterminatedFence`, `MalformedLine`, `NoFrontmatter` |
| `StateLoadError` | `load_state` | `Io { path, source }`, `Parse { path, source }` |
| `NestedParseError` | `parse_nested_frontmatter` | Unterminated fence, malformed line, malformed inline map, unterminated quoted value |
| `OpportunityError` / `LearningArtifactError` / `ProposalError` | each model's field-recovery constructor | `MissingField(&'static str)` |

`W_STATE_OP_SLUG_STUTTER` is the one shared diagnostic-code constant the crate exports, for a
consumer that wants to report the same warning code `okf-core`'s own `op_slug_stutters` check
describes.

## Design notes

- **Lenient by default.** Fields like `Carryover.kind` and `Block.status` model *shape*, not
  *policy* — an unrecognized value degrades to a typed fallback (e.g. `CarryoverKind::Unknown`)
  rather than rejecting the whole file. Enforcing an allowed value set is a validator's job, not
  this crate's.
- **Byte-stable round-trips.** Serialization is written so that parsing a file and re-serializing
  it unchanged reproduces the original bytes — every optional/empty field uses
  `skip_serializing_if` so a field a document never used doesn't appear on the next write.
- **Unknown-field capture.** The hand-authored structs (`StateFile`, `Track`, `TrackBlock`,
  `Backlog`, `Epic`, `Carryover`) carry a `#[serde(flatten)] extra` map so a field this crate
  doesn't yet model survives a round-trip instead of being silently dropped. Derived/regenerated
  views (`Focus`, `Block`, `RepoRollup`, `CrossRepoEdge`) deliberately do not, since they're
  rebuilt wholesale on every write.

## Cargo features

| Feature | Effect |
|---|---|
| *(default)* | No optional dependencies; `cargo tree` shows only `serde`, `serde_json`, `thiserror` |
| `typeshare` | Adds the optional [`typeshare`](https://docs.rs/typeshare) attribute crate and annotates `BlockedBy`'s four payload structs (`BlockDep`, `ExternalDep`, `OperatorDep`, `ApprovalDep`) plus `DisposalReason`/`AmendsRef`, so a `typeshare-cli` run can generate matching TypeScript interfaces for a consumer's frontend |

Details and the reason the `BlockedBy` enum itself can't carry the annotation: [`docs/architecture.md`](docs/architecture.md) § "The optional `typeshare` feature".

## Dependencies

`serde` (with `derive`), `serde_json`, and `thiserror` — no I/O, no path dependencies. Dev-only:
`tempfile`.

## Development

```bash
cargo fmt --check                            # format gate
cargo clippy --all-targets -- -D warnings    # lint gate (includes test targets)
cargo test                                   # unit + integration tests
cargo build --release                        # release build
```

Integration tests live under [`tests/`](tests/): [`tests/doc_roundtrip.rs`](tests/doc_roundtrip.rs)
(parse/serialize fidelity for the typed document layer, against fixtures in
[`tests/fixtures/`](tests/fixtures/)), [`tests/schema_conformance.rs`](tests/schema_conformance.rs)
(every field a schema doc documents as authored must exist on the matching struct),
[`tests/state_preservation.rs`](tests/state_preservation.rs), and
[`tests/struct_defaults.rs`](tests/struct_defaults.rs).

## Consumers

This crate is a member of a private Cargo workspace and is consumed via path dependency by two
sibling tools in that workspace:

- **`bastion`** — uses it for frontmatter and graph validation (`bastion validate`) and graph
  queries (`bastion brain`).
- **`mev`** — uses it for `mev validate-brain`, `mev emit-state`, and `mev emit-graph`.

Both are part of the broader **Bastion** ecosystem — see the
[bastion-os](https://github.com/bredmond1019/bastion-os) meta-repo for the full architecture.

## See also

- [`docs/index.md`](docs/index.md) — index of this repo's reference docs.
- [`docs/architecture.md`](docs/architecture.md) — module map, key types, data flow, and the
  `typeshare` feature in full detail.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) · <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) · <http://opensource.org/licenses/MIT>)

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.

Built for one operator and released because it may be useful to others — there is no support
obligation, no issue-response SLA, and no stability promise.
