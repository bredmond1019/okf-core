# okf-core Agent Instructions

Specific instructions for okf-core. See [AGENT.md](AGENT.md) for full context.

## Workflow engine telemetry

**After invoking `Workflow({name: 'sdlc-task'|'sdlc-flow', ...})`, load the `stamp-workflow-run-id`
skill.** The engine script can't read its own Workflow run id back — the Workflow script API has no
`runId` global and no filesystem access — so joining a run's `sdlc-task-state.json`/
`sdlc-flow-state.json` to the exact Claude Code session transcript for cost telemetry relies on the
*invoking* agent patching the id in after the call returns. Skip this and `workflow_run_id` simply
stays `null` — a normal, expected state, never a defect to chase.

## Before you start

- **Strategic context:** `planning/context.md` (read first) → `planning/status.md` (current state)
- **Symlink warning:** the `planning/` directory is actually a local symlink pointing to the company brain repo's `_planning/` vault (e.g. `core/_planning/okf-core/`). The brain repo is responsible for tracking all planning files under Git. Do not track `planning/` in this project's public Git repository (it is gitignored).
- **Symlink traps:** `rg`/`grep`/`find` are symlink-blind by default — a search that must include `planning/` content needs `-L`/`--follow`. `git mv` fails through the symlink face ("source directory is empty") — move planning files via the real vault path (`.../_planning/<slug>/...`), never via `planning/...`. Planning changes are committed in the brain repo (`agentic-portfolio`) with an explicit pathspec, never in this repo.

## Standing rules

1. **Never `git push` this repo directly from inside it.** `mev` and `bastion` both path-depend
   on `okf-core`, and every Rust repo's CI clones its sibling path-deps at their unpinned
   default branch — pushing out of order breaks a sibling's CI on code that was actually fine
   (the 2026-08-18 mev/bastion outage is the canonical example of this failure mode). Route
   every push through the company-brain's `agentic-portfolio/scripts/git_push.sh --all`, which
   pushes the whole fleet in dependency order and skips a repo flagged `ci-blocked` (a Cargo
   dependency is red on GitHub with nothing queued to fix it). Branching, committing, and
   opening/reviewing/merging PRs to `main` locally are all fine from inside this repo — only the
   final `git push` of `main` to `origin` must go through that script.

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

**Run to completion. Never stop, clear, or hand off because context is getting large.** There is no
token band, no percentage, and no "the next block would be cleaner in a fresh session." A chain runs
every block it was given; a lane that stops after one block and waits to be relaunched by hand
defeats the entire point of the run and puts the operator back in the loop after every block. If
context genuinely runs out, the harness summarizes and you keep going — that is its job, not yours.

There is exactly **one** reason to end a session early, and it is about correctness, not cost:
**something the running session depends on changed underneath it** — an engine, command file,
installed binary (`mev`, `bastion`), hook or `settings.json` edited this session, or a `CLAUDE.md`
you already read. The running session is a launch-time snapshot (base-template standing rule 10), so
it keeps producing pre-change results, which read as an unreliable agent rather than a stale
snapshot. **Name the trigger, finish the unit of work in flight, and say plainly that a fresh
session is needed.** Do not present it as a context-budget decision, and do not go looking for the
trigger as an excuse to stop.

Whenever you do hand off, write the entry point first — `status.md`, `handoff.md`, a spec's
`tasks.json`, or an orchestration-run `notes.md` — so the next agent starts from an artifact instead
of from your memory.
<!-- END:session-continuity -->
