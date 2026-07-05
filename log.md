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

## [2026-07-05]

### Added priority and due fields to Block and TrackBlock
- **What:** Added `priority: Option<u8>`, `due: Option<String>`, `sdlc_workflow: Option<String>`, and `model: Option<String>` to `TrackBlock` and `priority`/`due` to `Block` in `src/state.rs`. Added a roundtrip serialization test for fidelity and backwards-compatibility.
- **Why:** The upstream `mev` consumer needs these fields present in the state model to build out its scheduling and routing capabilities (`MV.6.A`).
- **Refs:** [OK.1.A](planning/1.1-block-shape-fields/tasks.md)
