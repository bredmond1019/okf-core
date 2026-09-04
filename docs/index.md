---
type: Index
title: okf-core docs
description: Index of project-facing reference docs for the okf-core crate.
doc_id: okf-core-docs-index
layer: [brain, factory]
project: okf-core
status: active
keywords: [okf-core, docs index, architecture, checks, git hooks]
related: [core:okf-core, okf-core-architecture, okf-core-checks, okf-core-type-contract]
---

# okf-core docs

Start at [`../README.md`](../README.md) — what the crate is, its public API, and a copy-pasteable
round-trip.

## Changing the code

| Doc | One line |
|---|---|
| [`architecture.md`](architecture.md) | Module map, data flow, and the mechanisms a change has to respect. |
| [`type-contract.md`](type-contract.md) | The load-bearing Rust types linked into mev/bastion/engine-rs, what breaks each, and the same-wave sequencing rule. |

## Running the checks

| Doc | One line |
|---|---|
| [`checks.md`](checks.md) | Every runnable check and script, and which ones gate a push. |
| [`../hooks/README.md`](../hooks/README.md) | The tracked git hooks and what each one blocks on. |
