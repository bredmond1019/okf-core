# AGENT.md — okf-core

The single-source OKF frontmatter / brain-graph / state-json contract, as a standalone Rust crate.

## Before you start

- **Strategic context:** `planning/context.md` (read first) → `planning/status.md` (current state)

## Standing rules

1. **Every new function, module, or behaviour change ships with tests.** No exceptions.
2. **OKF frontmatter is required on every new `.md` file under `docs/` and `planning/`.**
3. **No I/O dependencies** — okf-core is a pure leaf library. Do not add filesystem, network, or heavy dependencies. Stick to `serde`, `serde_json`, and `thiserror`.
4. **Coverage bar — separate pure logic from I/O, test the logic exhaustively.**
5. **Decisions are append-only**.

## Build / test / run

```bash
cargo fmt --check          # format gate
cargo clippy -- -D warnings  # lint gate
cargo test                 # test suite
cargo build --release      # release build
```

## Directory map

```
okf-core/
├── planning/           ← context, status, state.json
├── src/                ← source code
└── Cargo.toml          ← crate manifest
```
