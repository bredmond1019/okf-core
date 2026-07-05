---
type: Index
title: okf-core
description: The single-source OKF frontmatter / brain-graph / state-json contract, as a standalone Rust crate.
doc_id: okf-core
layer: [brain, factory]
project: okf-core
status: active
keywords: [okf-core, OKF frontmatter, brain graph, state.json, contract crate, Rust]
related: [context, status]
---

# okf-core

A pure Rust leaf library that owns the **single source of truth** for three brain contracts. It provides a unified implementation for use across the core-tier Rust projects.

## What It Is

It provides the data structures and logic for:
1. **OKF frontmatter** — `OkfFrontmatter`, `serialize_frontmatter`, `extract_frontmatter`, `parse_frontmatter`
2. **Brain graph** — `Edge`, `EdgeKind`, `Graph`, `Node`, `resolve_edge`, plus `GraphExport`
3. **State graph** — `StateFile`, `StateGraph`, `load_state`, `build_state_graph`

## Consumers

- **bastion** — Uses this crate for `bastion validate` and `bastion brain` to leverage the frontmatter and graph models.
- **mev** — Uses this crate for `mev validate-brain`, `emit-state`, and `emit-graph` to interact with frontmatter, graph, and state models.

## Dependencies

Its only dependencies are `serde`, `serde_json`, and `thiserror` — no I/O, no path deps. It is a member of the unified `core/` Cargo workspace.

## Tests

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```

## Directory map

```
okf-core/
├── planning/       ← context, status, state.json
├── src/            ← frontmatter, graph, state logic
└── Cargo.toml      ← crate manifest
```
