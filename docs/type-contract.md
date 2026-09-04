---
type: Reference
title: okf-core type contract
description: What breaks a consumer when an okf-core Rust type changes, per load-bearing type, and the sequencing rule that keeps a breaking change from shipping unadapted.
doc_id: okf-core-type-contract
layer: [brain, factory]
project: okf-core
status: active
keywords: [type contract, breaking change, non_exhaustive, serde other, consumer compile gate, coord]
related: [core:okf-core, okf-core-architecture, okf-core-checks, brain:fleet-contract-map]
---

# okf-core type contract

The other seven contract docs in this fleet (`engine-rs`/`bastion`/`synapse` data-contracts, the
workspace contract, the carryover contract, the serve-api contract — see
[`brain:fleet-contract-map`](../../../docs/fleet-contract-map.md)) all describe a **wire/JSON
payload crossing a process boundary**: the contract is a serialized shape, and the failure mode is
a runtime parse error on a stale pin. This doc is shaped differently on purpose, because okf-core's
exposure is different in kind, not degree.

## What this contract actually is

okf-core exports a **linked Rust surface**. `mev`, `bastion` and `engine-rs` depend on it directly
as a Cargo path dependency (`engine-rs` via `[workspace.dependencies]`) — no version number, no
pin-and-consumer doc pair. This is exactly the shape `fleet-contract-map.md` already documents for
a different repo, in its ["The unversioned case: bella-engine → bastion"](../../../docs/fleet-contract-map.md#the-unversioned-case-bella-engine--bastion)
section: per bella's `D3-bella-engine-shared-with-bastion.md`, *"treat `lib.rs` as a cross-repo
contract"* — a breaking change to the public surface breaks the consumer's build immediately, which
is a **stronger** guarantee than a stale doc pin (it cannot silently drift, only hard-fail), but it
means the usual "diff the version line" check does not apply. okf-core is that same shape, only
larger: three consumers instead of one, and already demonstrated four times in practice (OK.3.B,
D58, OK.4.B, and `b06f1f0`).

**Do not add a version number or a pin-and-consumer doc pair here.** The bella reasoning applies
verbatim: a path dependency hard-fails at compile time and cannot silently drift, so a version line
would be a second thing to keep in sync that buys nothing a compile error doesn't already give you
for free.

Who actually depends on okf-core today is not a hardcoded list in this doc — it is discovered live
by [`scripts/check_consumers.sh`](../scripts/check_consumers.sh), which walks every `[[repos]]`
entry in `brain.toml` for a `path` dependency resolving here:

```bash
scripts/check_consumers.sh --list
```

## The two orthogonal contracts

A change to an okf-core type can break a consumer along **two independent axes**, and a type can
need protection against one, both, or neither:

- **`#[non_exhaustive]`** binds **Rust match sites at compile time**. It stops a new enum variant
  (or struct field, via `#[non_exhaustive]` on the struct) from silently falling through a
  consumer's wildcard arm — the fix is forcing a compile error instead.
- **`#[serde(other)]` / an `Unknown(_)` fallback** binds **deserialization of data already on
  disk**. It stops an unrecognized value already written to `state.json` from failing to parse at
  all — the fix is degrading gracefully instead of erroring.

Concrete example of each:

- [`CarryoverKind`](../src/state.rs) (`#[serde(untagged)]`, `Known(KnownCarryoverKind)` /
  `Unknown(String)`) needs the **deserialization** contract only: its own design *is* the
  runtime-degradation mechanism for an unrecognized `kind` value already on disk. It derives
  `Serialize`/`Deserialize` but is exhaustive at the Rust-match level (only two variants, one of
  which is the catch-all) — `#[non_exhaustive]` here would add a second, redundant degradation
  mechanism on an axis that already has one.
- [`StateEdgeKind`](../src/state.rs) needs the **compile-time** contract's failure mode considered,
  but the verdict (below) is still not to apply `#[non_exhaustive]` — it derives `Serialize` only,
  with no path back from disk, so there is no deserialization to degrade in the first place. The
  `OK.4.B` break that motivated writing this doc was purely a Rust match-site break in consumers,
  making `#[non_exhaustive]` the only *applicable* tool for that failure — and OK.5.B's verdict is
  still not to use it, for reasons specific to the type (below).

## Two break classes, not one

**Enum variant additions** break an exhaustive `match` with no wildcard arm at the consumer's
compile step. **Struct field additions break exhaustive struct *literals*** in a consumer, even
when the struct derives `Default` — a literal that names every field has no way to receive a new
one, `Default` or not. Two of the five load-bearing types below are structs, so this is half the
doc, not a footnote: measured today, `b06f1f0` added two `Option` fields to `Backlog` (which does
derive `Default`) and broke `mev` at two production sites that named every field instead of
spreading —`src/brain/block_create.rs:1016` and `src/brain/conformance/backlog.rs:172`. The
contract's own guidance to a consumer hitting this: construct with `..Default::default()` rather
than naming every field, so a future added field is absorbed rather than a hard break.

## Load-bearing types

Every type below is exhaustive today — **OK.5.B recorded 28 written verdicts and applied
`#[non_exhaustive]` to zero of them.** Where this doc's summary and the type's own doc comment in
`src/state.rs` disagree, the doc comment (the code) is authoritative and the disagreement is a
defect in this doc, not the code.

| Type | Kind | A real consumer call site | Breaking-change verdict (OK.5.B) |
|---|---|---|---|
| [`StateEdgeKind`](../src/state.rs) | enum | `core/mev/src/brain/state.rs:1734` (exhaustive match; the `OK.4.B` break site) | No `#[non_exhaustive]`. It derives `Serialize` only (never deserialized), so `#[serde(other)]`/`Unknown(_)` were never applicable — there is no disk-read path to degrade. `#[non_exhaustive]` is the only tool that was ever on the table for its one failure mode (a Rust match-site break), and the verdict is still not to apply it: consumers are expected to write an explicit arm per variant on purpose, and silently routing a new variant through a catch-all is strictly worse than the loud `OK.4.B` break, because that break was at least visible. |
| [`CarryoverKind`](../src/state.rs) | enum (`#[serde(untagged)]`) | mev's carryover-kind handling (`known-vocabulary` check; okf-core defines the shape only, per AGENTS.md rule 3) | No `#[non_exhaustive]`. Already exhaustive at the Rust-match level (`Known`/`Unknown`, and `Unknown` already exists as the catch-all) — its whole design is the deserialization-degradation contract this axis needs. A third top-level variant is not anticipated; the fixed vocabulary itself lives one layer down in `KnownCarryoverKind`, where a new value is handled by `Unknown(String)`, not by adding a variant here. |
| [`BlockedBy`](../src/state.rs) | enum (`#[serde(tag = "type")]`) | mev: 78 measured match/wildcard arms across `state.rs`, `carryover.rs`, `emit.rs` | No `#[non_exhaustive]`. mev's own convention on the sibling `StateEdgeKind` is to write an explicit arm rather than a catch-all, precisely so a new variant forces that decision; `#[non_exhaustive]` here would silently defeat that convention wherever it is followed, which is not a cost taken on without a positive reason — none of the four variants (block/external/operator/approval dependency) is expected to gain a new one casually. |
| [`CarryoverScope`](../src/state.rs) | **struct** | `Carryover.scope` (non-`Option`, requires `CarryoverScope: Default`) across mev/bastion carryover handling | Break class is **struct field addition**, not enum variant addition. `CarryoverScope` derives `Default` specifically so a consumer can construct it with `..Default::default()`; a literal naming all three fields (`repo`, `tier`, `cross_repo`) would still break on a new field the way `Backlog` did in `b06f1f0` — the contract's guidance is the same spread-construction fix given above. |
| [`CarryoverArchiveRow`](../src/state.rs) | **struct** | `planning/carryover-archive.jsonl` row shape (okf-core defines the shape only; it performs no file I/O) | Break class is **struct field addition** over a nested-flatten shape (`entry: Carryover` flattens, and `Carryover` itself ends in a `#[serde(flatten)] extra` catch-all). A new field added here is a normal struct-literal break for the same reason as `CarryoverScope`. **Deliberately not `#[typeshare]`-annotated** — its flattened `entry` field is unsupported by typeshare and aborts codegen for every consumer scanning this crate; a consumer needing this shape on the wire declares a local mirror instead (as `bastion/src/serve/dto.rs` already does for `StateEdgeKind`/`BlockLane`). |

### `coord` — zero consumers today

`src/coord/` (`disposal`, `drain_log`, `escalation`, `heartbeat`, `lease`, `message`, `registry`,
`slot`, `sweep_snapshot`) is a second family of cross-repo shared types, added for the
coordination-layer port. **It has zero consumers today** — nine consumer blocks are queued behind
it across `engine-rs`, `mev` and `bastion`, but none of them exist yet in the compiled graph
`scripts/check_consumers.sh --list` reports. Listing `coord`'s types here as if they were already
load-bearing, pinned-verdict types would be premature: no OK.5.B-style verdict has been recorded
for them, and none should be invented in this doc. This section exists so a planner scheduling the
first `coord` consumer block knows to come back and add its type(s) to the table above alongside a
real OK.5.B-style verdict — not to pre-empt that verdict now.

## The sequencing rule

**The block that adds a variant or a field owns the consumer adaptation in the same wave.** A
planner who schedules a type change without also scheduling every discovered consumer's adaptation
in the same wave has left unscheduled work that a green board will not show.

The precedent this doc exists to stop repeating: the autonomous-foundation roadmap sequenced mev's
*feature* work behind `OK.4.B` (`MV.16.A`, `MV.16.C`) but never scheduled mev's *compile* work
against `OK.4.B`'s new `StateEdgeKind::CarryoverBlocks` variant — and that unscheduled adaptation is
exactly what broke `mev` and `bastion`'s installs. The type change and the consumer fix are one
unit of work; splitting them across waves is how the break gets discovered by a `cargo install`
failure instead of by a gate.

**Enforcement:** [`scripts/check_consumers.sh`](../scripts/check_consumers.sh) is the mechanism
that makes a violation of this rule visible instead of silent. It compiles every discovered
consumer's **test** targets (`cargo nextest run --no-run --locked`), because all three prior
invisible breaks (`OK.3.B`, `D58`, `OK.4.B`) lived in test-only code a plain `cargo build` walks
past. It reports a `compiled N of M discovered` summary and is **strict by default**: an incomplete
run (`compiled < discovered`) fails the gate on its own, in addition to an unwaived `broken`
consumer or a stale waiver. `planning/harness.json`'s `consumer-compile-gate` entry runs it with
`--allow-incomplete`, which opts out only of the incompleteness half — a sibling lane's dirty tree
is the fleet's normal four-lane-concurrency case, not evidence okf-core broke something — while a
genuinely `broken` consumer or a stale waiver still fails the gate even with that flag.

The one sanctioned way to keep okf-core pushable while a consumer is knowingly broken by something
outside this repo is a row in `scripts/consumer-gate-waivers.txt` (`<slug> | <owning-block-id> |
<reason>`) — **a waiver row must name an owning block**, never `gates: false`. A waiver for a
consumer that now compiles clean fails the gate as a *stale waiver*, which is what stops the file
from rotting into permanent debt; the fix is to delete the row, not keep it.

See [`docs/architecture.md`](architecture.md#consumer-compile-gate) and
[`docs/checks.md`](checks.md) for the full mechanics; this doc states the contract those pages
enforce, not a restatement of okf-core's architecture.
