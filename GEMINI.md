# okf-core Agent Instructions

Specific instructions for okf-core. See [AGENT.md](AGENT.md) for full context.

## Before you start

- **Strategic context:** `planning/context.md` (read first) → `planning/status.md` (current state)
- **Symlink warning:** the `planning/` directory is actually a local symlink pointing to the company brain repo's `_planning/` vault (e.g. `core/_planning/okf-core/`). The brain repo is responsible for tracking all planning files under Git. Do not track `planning/` in this project's public Git repository (it is gitignored).

<!-- BEGIN:response-style -->
## Response Style

Optimize every reply for an operator scanning several concurrent agent sessions. Default to the
shortest response that fully answers. Long prose is the failure mode, not thoroughness.

**Shape**

1. **First line = the outcome.** What happened, and did it work. No preamble, no restating the ask.
2. **Then the specifics, if any** — bullets, one line each, max ~6. Facts, not narration.
3. **Last line = the ask, if any** — one question the user can answer in a word.

Ceiling for a normal turn: **~150 words / ~15 lines**. Only depth the user explicitly asked for
(a review, a design rationale, a plan document) may exceed it.

**Cut**

- Reasoning narration — how you got there, what you considered, what you almost did. Report
  conclusions; the transcript already holds the steps.
- Justifying decisions that worked out. Explain only what was non-obvious or that the user may
  want to reverse.
- Unasked-for "what's next", roadmaps, option menus, and status recaps.
- Tables or headings for fewer than ~4 rows/sections — a sentence or bullets is faster to read.
- Self-assessment and stage direction: "the finding that reframes everything", "worth your
  attention", "one thing I want to flag", praise, hedging, apology.
- Re-explaining anything already in a file you just wrote. Link the path instead.

**Keep — these earn their space**

- Failures, blocks, and anything not matching what was asked: say it first, plainly, with the
  real error text.
- Assumptions the user might reject, and decisions that need their call.
- Security, data-loss, or money implications.
- Exact identifiers where they *are* the content: `src/serve/handlers/attention.rs:101`, a
  version, an error code. Never a paragraph describing what a one-line reference would say.

**Register**

Plain English for status, decisions, and trade-offs. Technical depth only where it changes what
the user does next. One idea per sentence; no stacked em-dash asides.
<!-- END:response-style -->
