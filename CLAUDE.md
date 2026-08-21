# okf-core Agent Instructions

Specific instructions for okf-core. See [AGENT.md](AGENT.md) for full context.

## Before you start

- **Strategic context:** `planning/context.md` (read first) → `planning/status.md` (current state)
- **Symlink warning:** the `planning/` directory is actually a local symlink pointing to the company brain repo's `_planning/` vault (e.g. `core/_planning/okf-core/`). The brain repo is responsible for tracking all planning files under Git. Do not track `planning/` in this project's public Git repository (it is gitignored).
- **Symlink traps:** `rg`/`grep`/`find` are symlink-blind by default — a search that must include `planning/` content needs `-L`/`--follow`. `git mv` fails through the symlink face ("source directory is empty") — move planning files via the real vault path (`.../_planning/<slug>/...`), never via `planning/...`. Planning changes are committed in the brain repo (`agentic-portfolio`) with an explicit pathspec, never in this repo.

<!-- BEGIN:response-style -->
## Response Style

You are read by an operator scanning several concurrent agent sessions. Long prose is the failure
mode, not thoroughness.

1. **First line = the outcome** — what happened, and whether it needs them.
2. **Then the specifics** — bullets, one line each, max ~6. Facts, not narration.
3. **Last line = the ask**, if there is one. One question, answerable in a word.

**Ceiling: 10 lines for a normal turn, 20 for an end-of-run report.** Only depth the operator
explicitly asked for may exceed it.

Durable detail goes to disk — the commands already require that. **Link the path; do not restate
the file.** Lead with failures, blocks, and anything that did not match the ask, in plain words with
the real error text. Cut reasoning narration, unasked-for next steps, and self-assessment.

Full rationale, the complete cut-list, and worked before/after examples: the
**`report-to-the-operator`** skill.
<!-- END:response-style -->

<!-- BEGIN:session-continuity -->
## Stopping, continuing, and handing off

Decide in this order. Only the third question is about tokens, and most of the time you never reach
it. Raise this proactively when it applies — do not wait to be asked.

1. **Is there a correctness reason to restart?** This overrides everything and holds at any context
   size. An engine, command file, installed binary (`mev`, `bastion`), hook or `settings.json`
   changed this session; or the operator edited a `CLAUDE.md` you already read. The running session
   is a launch-time snapshot (standing rule 10), so it keeps producing pre-change results that read
   as an unreliable agent rather than a stale snapshot. **Name the trigger; do not present it as a
   cost decision.**
2. **Does the next chunk of work have a written entry point?** The gate is the artifact, not the
   number. If the next agent can start from `status.md`, `handoff.md`, a spec's `tasks.json`, or an
   orchestration-run `notes.md`, clearing is nearly free. If not, **suggest writing that artifact
   first, then clearing** — and never clear mid-debug, mid-block, or mid-decision, where the
   valuable context is the part that cannot be written down. If clearing feels expensive, that is a
   signal the handoff is thin, not a reason to stay.
3. **Only then, the context size.** The real signal is what fraction is finished tool output rather
   than active understanding. Rough bands: under ~100k don't raise it · 100–200k keep going ·
   200–300k finish the unit in flight then suggest clearing, and don't start a new one · over ~300k
   suggest clearing at the next boundary. These prompt you to *raise* it, never to abandon work in
   flight. **In an orchestration lane the rule is structural: clear at block boundaries, never
   mid-block** — budget ~20–40k of context per block.

Full rationale, the correctness-trigger table, and what to actually say: the **`stop-or-continue`**
skill.
<!-- END:session-continuity -->
