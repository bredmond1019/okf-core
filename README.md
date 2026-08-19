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
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Directory map

```
okf-core/
├── planning/       ← context, status, state.json, harness.json (the gates)
├── src/            ← frontmatter, graph, state logic
└── Cargo.toml      ← crate manifest
```

## Built within the Bastion workspace

This crate is a member of the unified `core/` Cargo workspace and is consumed by sibling
crates (`bastion`, `mev`) via **path dependency** — it isn't published to crates.io and isn't
designed to build standalone outside that workspace. See the
[`bastion-os`](https://github.com/bredmond1019/bastion-os) meta-repo for the full ecosystem.

## Roadmap / Known limitations

No known limitations. This is a stable, leaf-level contract crate — no I/O, no path
dependencies of its own — and its scope is intentionally minimal.

Part of the broader **Bastion** ecosystem — see the [bastion-os](https://github.com/bredmond1019/bastion-os) front door for the full architecture.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) · <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) · <http://opensource.org/licenses/MIT>)

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.

Built for one operator and released because it may be useful to others — there is no support
obligation, no issue-response SLA, and no stability promise. See HQ decisions D40 and D75.
