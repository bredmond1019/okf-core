---
type: Guideline
title: okf-core checks and scripts
description: Catalogue of every runnable check in okf-core — the gated harness suite, the consumer compile gate, the typeshare guard, the block-record validator, and the git hooks — with how to invoke each one.
doc_id: okf-core-checks
layer: [factory]
project: okf-core
status: active
keywords: [okf-core, gates, consumer compile gate, typeshare, pre-push hook, cargo]
related: [core:okf-core, okf-core-architecture]
---

# okf-core checks and scripts

Everything in this repo you can *run*, and when to run it. For the crate's API surface see
[`../README.md`](../README.md) § Public API; for how the code is laid out see
[`architecture.md`](architecture.md).

## What this page is for

`cargo test` is not the whole gate. okf-core is the root of a dependency fan-out — three sibling
crates compile against it — so "my tests pass" and "I did not break anyone" are different
questions, answered by different commands. This page lists every command, says which ones block a
push, and says which ones are slow or reach outside this repo.

## Quickstart

All of these are **shell** commands, run from the repo root. Nothing here is a Claude Code slash
command.

```bash
cargo fmt --check                            # format
cargo clippy --all-targets -- -D warnings    # lint, test targets included
cargo test                                   # unit + integration tests
cargo build --release                        # release build
```

That is the fast inner loop. Before you push a change to a shared type, also run the slow one:

```bash
scripts/check_consumers.sh --list            # safe: just names the consumers, compiles nothing
scripts/check_consumers.sh                   # slow: compiles three sibling repos' test targets
```

| Needed | Why | If missing |
|---|---|---|
| Rust, edition 2024 | `Cargo.toml` pins `edition = "2024"` | Install/update via [rustup](https://rustup.rs/) |
| `cargo-nextest` | The consumer gate compiles with `cargo nextest run --no-run --locked` | `cargo install cargo-nextest --locked`; without it only the consumer gate is affected |
| The sibling repos checked out | The consumer gate discovers them from the brain's `brain.toml` | Consumer gate only — it reports what it could not find rather than passing silently |
| `typeshare-cli` | Only for `scripts/check-typeshare.sh` | `cargo install typeshare-cli --locked`; the script exits 0 and says so if absent |

## The gated suite

These seven are the checks in `planning/harness.json` — the file the SDLC engines' Test stage and
the `pre-push` hook's stage 2 both read. (That path is in the brain's planning vault, not in this
repo's git history, so there is nothing to link.) **All seven gate**: a failure blocks the verdict.

| Check | Command | What it proves |
|---|---|---|
| `fmt` | `cargo fmt --check` | Source matches rustfmt. |
| `clippy` | `cargo clippy --all-targets -- -D warnings` | No lints, in test targets too. |
| `test` | `cargo test` | The suite passes — **authoritative for the verdict**. |
| `build` | `cargo build --release` | Release profile compiles. Not run per-task. |
| `cargo-audit` | `cargo audit` | No known-vulnerable dependency. Guarded by `command -v` so an uninstalled `cargo-audit` cannot block a push. |
| `consumer-compile-gate` | [`scripts/check_consumers.sh`](../scripts/check_consumers.sh) | Every consumer still compiles against this shape. Not run per-task — it is three real cargo builds. |
| `consumer-gate-tests` | [`scripts/test_check_consumers.sh`](../scripts/test_check_consumers.sh) | The gate script itself still works. `cargo` and `git` are shimmed, so nothing is compiled and no repo outside a `mktemp -d` is touched. |

If this table and `planning/harness.json` disagree, the JSON is the authority.

## The consumer compile gate

**Why it exists:** a breaking change to a shared type often shows up only in a *consumer's test
code* — a struct literal or a match arm that no library build ever constructs. Three such changes
(OK.3.B, D58, OK.4.B, all okf-core decisions) went green here and broke `mev` and `bastion`
downstream. So the gate compiles each consumer's **test targets**, and never runs them.

Consumers are **discovered, not hardcoded**: the script reads the brain's `brain.toml` and walks
each repo's `dependencies`, `dev-dependencies`, `build-dependencies` and `workspace.dependencies`
for a `path` entry resolving to okf-core. Today that finds three — `mev`, `bastion`, `engine-rs`.

| Invocation | What it does | Cost |
|---|---|---|
| `scripts/check_consumers.sh --list` | Prints the discovered consumer slugs and exits 0. Discovery only — **nothing is compiled**. Start here. | Instant |
| `scripts/check_consumers.sh --consumer <slug>` | Runs and reports on exactly one consumer. | One cargo build |
| `scripts/check_consumers.sh` | Runs, classifies and reports on every consumer. | Three cargo builds |
| `scripts/check_consumers.sh --json` | The same run, as compact JSON. | Three cargo builds |

It writes nothing outside its own temp files and touches no consumer's working tree — a consumer
with a dirty tree is reported `skipped-dirty` rather than compiled, because its result would not be
evidence about okf-core either way.

**Only the `broken` verdict fails the gate.** `lockfile-stale`, `skipped-dirty` and `not-evaluable`
are explicitly *not* evidence that okf-core caused a problem — read the reported reason before
assuming a red gate is yours. Verdict table and the waiver mechanism
([`scripts/consumer-gate-waivers.txt`](../scripts/consumer-gate-waivers.txt), one row per knowingly
broken consumer, all three fields mandatory, self-deleting when the consumer goes green):
[`architecture.md`](architecture.md) § "Consumer compile gate".

## The ungated scripts

Useful, but nothing blocks on them.

| Script | Run it when | Notes |
|---|---|---|
| [`scripts/check-typeshare.sh`](../scripts/check-typeshare.sh) | You changed `BlockedBy` or one of its four payload structs and want the TypeScript to still generate | Regenerates into a temp file and asserts all four interfaces appear. **Deliberately never hard-fails**: a machine with no `typeshare` CLI gets a message and exit 0. It also probes `~/.cargo/bin` directly, since that directory is not on every machine's PATH. |
| [`scripts/check_block_records.py`](../scripts/check_block_records.py) | You added or edited a block record | Validates block records against `block.schema.json` (base-template D65) — required fields, the ID grammar, ID/filename/`spec_dir` agreement, enums, date formats, edge shapes. Dependency-free on purpose: `jsonschema` is installed nowhere in this fleet, so a validator that imports it would validate nothing and report success. |

`check_block_records.py` reads `planning/blocks/` by default (`--planning DIR` to point elsewhere,
`--fleet` to walk every repo, `--quiet` for failures only). `planning/` is a symlink into the
brain's vault and is not part of this repo's public tree, so a repo with no `blocks/` directory is
reported as fine, not as a failure.

## Git hooks

[`hooks/`](../hooks/) carries the tracked hooks; [`hooks/README.md`](../hooks/README.md) is the
full reference. Enable them once, from the repo root:

```bash
git config core.hooksPath hooks
git config --get core.hooksPath   # → hooks
```

`pre-push` runs in two stages: stage 1 validates the wider corpus but blocks only on errors new
since this clone's last successful push; stage 2 runs this repo's own `planning/harness.json`
checks. **As of 2026-08-24 the hook file is `chmod -x`'d fleet-wide at the operator's request**, so
it is currently inert — present and maintained, but not executing on your pushes. Do not read a
clean push as a passed gate; run the commands above yourself.

**Never `git push` this repo directly.** `mev`, `bastion` and `engine-rs` path-depend on okf-core
and each clones its path-deps at their unpinned default branch, so pushing out of order breaks a
sibling's CI on code that was fine. Route pushes through the brain's
`agentic-portfolio/scripts/sync/git_push.sh`, which pushes the fleet in dependency order.

## Troubleshooting

| Symptom | Likely cause | What to check |
|---|---|---|
| The consumer gate reports `broken` but `cargo test` is green here | A consumer's *test-only* code constructs a type you changed | The reported `file:line` in that consumer — this is the exact case the gate exists for |
| `lockfile-stale` | The consumer's `Cargo.lock` needs updating; `--locked` refused | Decided by the stderr signature, not the exit code — exit 101 means both things |
| `skipped-dirty` | That consumer's git tree is dirty | Commit or stash there, then re-run |
| `check-typeshare.sh` prints "CLI not found" and exits 0 | `typeshare-cli` is not installed, or `~/.cargo/bin` is off PATH | `cargo install typeshare-cli --locked` — this is never a gate failure |
| A piped check "passes" but the same command alone fails | A pipeline's exit code is the **last** command's | Redirect to a file, then check `$?` |

## See also

- [`../README.md`](../README.md) — what the crate is, its public API, its consumers.
- [`architecture.md`](architecture.md) — module map, data flow, and the full consumer-gate,
  conformance-gate and `typeshare` detail.
- [`../hooks/README.md`](../hooks/README.md) — every hook, what it blocks on, and how it is tested.
