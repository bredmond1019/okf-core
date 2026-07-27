---
type: Log
title: okf-core Log
description: Chronological log of work and decisions for okf-core.
doc_id: log
layer: [brain, factory]
project: okf-core
status: active
keywords: [log, okf-core]
timestamp: "2026-07-05T16:32:00-03:00"
---

# Log

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
