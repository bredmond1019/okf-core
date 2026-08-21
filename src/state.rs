//! `state.json` serde schema + block-dependency graph model.
//!
//! Ported (pure model + primitives only) from `mev`'s `brain/state.rs` — the
//! serde structs mirroring `planning/state-schema.md`, plus a loader and a
//! block-dependency graph builder. mev's validation/derivation logic
//! (`check_*`/`derive_*`, `discover_state_files`, `build_graph`, `check_graph`)
//! depends on mev's `Corpus`/`BrainConfig`/`Diagnostic` types and stays in mev;
//! it consumes these shared types instead of duplicating them.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when loading a `planning/state.json` file.
#[derive(Debug, Error)]
pub enum StateLoadError {
    /// The file could not be read from disk.
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file contents are not valid JSON (or do not match [`StateFile`]'s shape).
    #[error("could not parse {path} as JSON: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

// ---------------------------------------------------------------------------
// BlockedBy — internally tagged enum on `type`
// ---------------------------------------------------------------------------

/// Payload for [`BlockedBy::Block`] — a dependency on another block (may be
/// cross-repo).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub struct BlockDep {
    /// Slug of the owning repo.
    pub repo: String,
    /// Canonical block ID (e.g. `BA.11.C`).
    pub id: String,
    /// Optional gloss explaining the dependency.
    #[serde(default)]
    pub what: Option<String>,
}

/// Payload for [`BlockedBy::External`] — an environmental / external
/// dependency (not a tracked block).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub struct ExternalDep {
    /// Human description of the external dependency.
    pub what: String,
}

/// Payload for [`BlockedBy::Operator`] — an operator working session that
/// gates this block.
///
/// Targetless — no node, skipped by dangling/cycle/topo logic — but
/// identified: a `slug` can be shared across several blocks so tooling can
/// find and clear the gate they are all waiting on. Named `operator` rather
/// than `session` to avoid colliding with `bastion sessions` (the tmux CLI);
/// this is a `depends_on` edge discriminant only.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub struct OperatorDep {
    /// Kebab-case identifier. Shared across every block this gate
    /// covers, giving otherwise-unrelated blocks a join key.
    pub slug: String,
    /// The artifact whose existence ends the session — the thing you
    /// can point at afterwards, not a description of the work.
    pub exit: String,
    /// The command that starts the session, paste-ready.
    pub start: String,
    /// Optional gloss: why this particular block is gated on it.
    #[serde(default)]
    pub what: Option<String>,
}

/// Payload for [`BlockedBy::Approval`] — a gated action awaiting a single
/// operator decision.
///
/// Targetless like `Operator` — no node, skipped by dangling/cycle/topo
/// logic — but identified via `slug` so tooling can find and clear it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub struct ApprovalDep {
    /// Kebab-case identifier. Shared across every block this gate
    /// covers.
    pub slug: String,
    /// One line stating the decision (not the context) — this is what
    /// gets rendered next to the approve control.
    pub what: String,
    /// Digest of the exact payload reviewed. Binds the approval to what
    /// was seen: if the payload changes, the approval is void and
    /// re-queues.
    pub digest: String,
}

impl OperatorDep {
    /// D76 display identity for this edge's slug — see [`op_id`].
    pub fn op_id(&self) -> String {
        op_id(&self.slug)
    }

    /// Whether this edge's slug carries a redundant `operator-` prefix —
    /// see [`op_slug_stutters`]. Delegates rather than reimplementing the
    /// rule: two implementations of one predicate is exactly the drift this
    /// ticket exists to prevent.
    pub fn slug_stutters(&self) -> bool {
        op_slug_stutters(&self.slug)
    }
}

impl ApprovalDep {
    /// D76 display identity for this edge's slug — see [`op_id`].
    ///
    /// `ApprovalDep` gets this only because it shares the slug shape with
    /// `OperatorDep` (D76); its `digest` binding is untouched by this
    /// ticket.
    pub fn op_id(&self) -> String {
        op_id(&self.slug)
    }

    /// Whether this edge's slug carries a redundant `operator-` prefix —
    /// see [`op_slug_stutters`]. Delegates rather than reimplementing the
    /// rule, for the same reason as [`OperatorDep::slug_stutters`].
    pub fn slug_stutters(&self) -> bool {
        op_slug_stutters(&self.slug)
    }
}

/// Diagnostic code for a `slug` that carries a redundant `operator-` prefix
/// (see [`op_slug_stutters`]). Exported as a `const` so okf-core and mev
/// share one literal instead of two independently-typed strings that could
/// drift.
// Not yet re-exported from the crate root (that's task 3 of
// OK.ticket.op-slug-stutter-warning) nor used by the inherent methods
// (task 2) or tests (tasks 4/5), so clippy sees this as unreachable dead
// code until those land. Remove this `allow` once they do.
#[allow(dead_code)]
pub const W_STATE_OP_SLUG_STUTTER: &str = "W_STATE_OP_SLUG_STUTTER";

/// Renders the D76 display identity for an operator/approval edge slug as
/// `OP.<slug>`.
///
/// Deliberately faithful: no normalization is applied here, so a slug that
/// [`op_slug_stutters`] (e.g. `operator-mac-mini-visit`) renders as the
/// stuttering `OP.operator-mac-mini-visit` rather than being silently
/// cleaned up. The stutter must stay visible until the slug itself is
/// renamed by the sweep (`MV.ticket.op-slug-rendering-and-sweep`).
#[allow(dead_code)]
pub fn op_id(slug: &str) -> String {
    format!("OP.{slug}")
}

/// True exactly when `slug` carries a redundant `operator-` prefix under
/// D76 — i.e. it starts with the literal `operator-` AND the remainder
/// after that prefix is non-empty.
///
/// `operator` and `operator-` are both `false` (no remainder to strip);
/// `operators-guild` is `false` (its prefix is `operators-`, not the exact
/// literal `operator-`).
#[allow(dead_code)]
pub fn op_slug_stutters(slug: &str) -> bool {
    slug.strip_prefix("operator-")
        .is_some_and(|rest| !rest.is_empty())
}

/// Strips exactly one redundant `operator-` prefix from `slug`, when and
/// only when [`op_slug_stutters`] is true; otherwise returns `slug`
/// unchanged.
///
/// Never returns an empty string for any input: the empty-remainder case is
/// exactly what `op_slug_stutters` excludes, so a doubled prefix like
/// `operator-operator-x` normalizes to `operator-x` (which still stutters
/// and is reported again) rather than being fully cleaned in one pass.
#[allow(dead_code)]
pub fn normalize_op_slug(slug: &str) -> &str {
    if op_slug_stutters(slug) {
        &slug["operator-".len()..]
    } else {
        slug
    }
}

/// A single entry in a `blocked_by[]` / `depends_on[]` / `related[]` array.
///
/// Tagged by the `"type"` field. Unknown `type` values are rejected by serde
/// (no `#[serde(other)]`), surfaced as `StateLoadError::Parse`.
///
/// Each variant is a newtype over a named payload struct rather than an
/// inline body. serde serializes a newtype-of-struct variant flat under
/// internal tagging, so the wire shape is unchanged; the payload structs
/// carry the typeshare annotation instead of the enum, because typeshare
/// rejects internally-tagged algebraic enums outright.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockedBy {
    /// A dependency on another block (may be cross-repo).
    Block(BlockDep),
    /// An environmental / external dependency (not a tracked block).
    External(ExternalDep),
    /// An operator working session that gates this block.
    Operator(OperatorDep),
    /// A gated action awaiting a single operator decision.
    Approval(ApprovalDep),
}

// ---------------------------------------------------------------------------
// ClearsWhen / ClearsWhenPredicate — carryover clearing conditions
// ---------------------------------------------------------------------------

/// A carryover clearing condition: either free prose (the legacy form, and
/// still the only form available for genuinely subjective conditions) or a
/// machine-checkable typed predicate.
///
/// Untagged: `Prose(String)` MUST stay the first variant. `#[serde(untagged)]`
/// tries variants in declaration order, and Prose-first is what guarantees
/// every existing prose `clears_when` string in the live corpus keeps
/// deserializing to exactly the string it is today rather than being coerced
/// into (and failing to parse as) a `Predicate`.
///
/// okf-core defines the SHAPE only and never evaluates it (AGENT.md rule 3);
/// evaluation lives in mev.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ClearsWhen {
    /// Free-form prose describing what clears this carryover. The legacy
    /// form, and still the only form for subjective conditions.
    Prose(String),
    /// A machine-checkable typed predicate.
    Predicate(ClearsWhenPredicate),
}

/// A machine-checkable carryover clearing predicate.
///
/// Tagged by the `"type"` field. Unknown `type` values are rejected by serde
/// (no `#[serde(other)]`), surfaced as a parse error rather than silently
/// falling back to prose.
///
/// okf-core defines the SHAPE only and never evaluates it (AGENT.md rule 3);
/// evaluation lives in mev.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClearsWhenPredicate {
    /// Clears when the named block in the named repo is closed.
    BlockClosed {
        /// Slug of the owning repo.
        repo: String,
        /// Canonical block ID (e.g. `BA.11.C`).
        id: String,
        /// Optional gloss explaining the condition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// Clears when the given path exists on disk.
    FileExists {
        /// Path to check for existence.
        path: String,
        /// Optional gloss explaining the condition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// Clears when the given path exists and contains a pattern match.
    FileContains {
        /// Path to check.
        path: String,
        /// Pattern to search for within the file.
        pattern: String,
        /// Optional gloss explaining the condition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// Clears when running the given command exits zero.
    CommandExitsZero {
        /// Shell command to run.
        command: String,
        /// Optional gloss explaining the condition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Block — lenient superset across now/next/blocked variants
// ---------------------------------------------------------------------------

/// One entry in a `focus.now`, `focus.next`, `focus.blocked`, or `focus.deferred` array.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Block {
    /// Canonical block ID. `#[serde(alias)]` keeps v1 `"block"`-keyed files readable.
    #[serde(alias = "block")]
    pub id: String,
    /// Brief human description.
    pub title: String,
    /// Lifecycle status (present on `now`, `blocked`, and `deferred` entries).
    #[serde(default)]
    pub status: Option<String>,
    /// Optional in-flight context note.
    #[serde(default)]
    pub note: Option<String>,
    /// Cross-repo source repo slug (used in brain `focus` entries).
    #[serde(default)]
    pub repo: Option<String>,
    /// What this block is waiting on (present on `blocked` entries).
    #[serde(default)]
    pub blocked_by: Vec<BlockedBy>,
    /// Execution priority (e.g. 1, 2, 3).
    #[serde(default)]
    pub priority: Option<u8>,
    /// Target due date or timing string (e.g. "2026-07-15").
    #[serde(default)]
    pub due: Option<String>,
    /// Cross-repo epic membership (slugs into the HQ `epics[]` registry).
    ///
    /// Carried on focus/rollup entries so a derived brain focus keeps membership
    /// without a second join back to `tracks[]`. Skipped when empty so the
    /// overwhelming majority of blocks (which belong to no epic) do not gain an
    /// `"epics": []` key on the next `emit-state --write`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub epics: Vec<String>,
}

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

/// The `focus` object — what's now, next, blocked, and deferred in a repo.
///
/// `Focus` and its sibling derived views (`Block`, `RepoRollup`, `CrossRepoEdge`)
/// deliberately do **not** carry an unknown-field capture map, unlike the six
/// authored structs (`StateFile`, `Track`, `TrackBlock`, `Backlog`, `Epic`,
/// `Carryover`). Derived views are regenerated wholesale on every
/// `mev emit-state --write` run from the authored data, not hand-edited —
/// preserving a stale unknown key here would *resurrect* data a prior run
/// intentionally dropped, rather than protect authored data from being lost.
/// The asymmetry is deliberate: capture belongs only on the structs a human
/// actually writes into `state.json` by hand.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Focus {
    /// Blocks currently in progress.
    #[serde(default)]
    pub now: Vec<Block>,
    /// Blocks queued for next (ordered).
    #[serde(default)]
    pub next: Vec<Block>,
    /// Blocks waiting on something.
    #[serde(default)]
    pub blocked: Vec<Block>,
    /// Blocks deliberately parked on the back burner (authored `status: "deferred"`).
    ///
    /// Deferred blocks are real roadmap work that is *not* being surfaced as next.
    /// They are excluded from ready-order (so they can never reach `next`) and do
    /// not enter `blocked` even when they carry unmet deps — `deferred` is a
    /// terminal lane assignment, exactly like `now`.
    ///
    /// Skipped when empty so the overwhelming majority of repos (which defer
    /// nothing) do not gain a `"deferred": []` key on the next `emit-state
    /// --write`. Dropping `skip_serializing_if` would churn every state.json in
    /// the portfolio, because `plan_state_json` diffs pretty-printed JSON.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred: Vec<Block>,
}

// ---------------------------------------------------------------------------
// Track / TrackBlock — leaf roadmap catalog
// ---------------------------------------------------------------------------

/// One block entry inside a `tracks[]` phase/wave.
///
/// Derives `Default` so downstream consumers construct this with
/// `..Default::default()`, naming only the fields they care about — a future
/// field addition here is then non-breaking for every such call site.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TrackBlock {
    /// Canonical block ID.
    pub id: String,
    /// Brief human description.
    pub title: String,
    /// Lifecycle status (authored: `open`/`in_progress`/`deferred`/`closed`).
    ///
    /// `deferred` parks the block on the back burner: still real roadmap work,
    /// still counted, but structurally unable to reach `focus.next`. It is
    /// manual and sticky — there is no expiry date; edit it back to `open` to
    /// resume. `blocked` remains forbidden as an authored value (it is derived).
    #[serde(default)]
    pub status: Option<String>,
    /// The block's full dependency edges (the authoritative DAG).
    #[serde(default)]
    pub depends_on: Vec<BlockedBy>,
    /// Execution-order rank for "what's next" (orthogonal to track grouping).
    #[serde(default)]
    pub wave: Option<i64>,
    /// Backlog-promotion provenance, when this block came from a backlog item.
    #[serde(default)]
    pub origin: Option<Origin>,
    /// Free-form authored annotation on the block — why it is in this state, what
    /// evidence set its status, or a caveat a future reader needs.
    ///
    /// `docs/state/state-schema.md` has always listed `note?` as an authored field on
    /// track blocks, but this struct omitted it, so every `mev emit-state --write` that
    /// re-serialized a `state.json` silently deleted every note in it. That is a
    /// destructive round-trip on a field the schema documents as authored, and
    /// `scripts/routine.sh` runs `emit-state --write` unattended on cron — so notes were
    /// being erased with no one watching. Found 2026-08-03 while backfilling 38 historical
    /// blocks whose whole value was the provenance recorded in exactly this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Longer human-facing description of the block, for surfaces that have room to
    /// render more than `title` — bastion-web's board being the first consumer.
    ///
    /// Distinct from `note`: `note` is an operator annotation about *state* ("closed
    /// because X shipped"), `description` is a fuller statement of *what the block is*.
    /// Both are authored and optional; neither is derived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Execution priority (e.g. 1, 2, 3).
    #[serde(default)]
    pub priority: Option<u8>,
    /// Target due date or timing string (e.g. "2026-07-15").
    #[serde(default)]
    pub due: Option<String>,
    /// Automated execution workflow (e.g. "sdlc-run").
    #[serde(default)]
    pub sdlc_workflow: Option<String>,
    /// Preferred model for automation (e.g. "opus").
    #[serde(default)]
    pub model: Option<String>,
    /// Cross-repo epic membership — zero or more slugs into the HQ `epics[]`
    /// registry. Multi-valued because a block can genuinely serve two
    /// initiatives at once (e.g. an API endpoint used by two Surfaces).
    ///
    /// Authored. Validated against the registry by mev
    /// (`E_STATE_UNKNOWN_EPIC`). Skipped when empty so untagged blocks stay
    /// byte-identical across an `emit-state --write`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub epics: Vec<String>,
    /// Authoring date (`YYYY-MM-DD`) — when this block record was written.
    ///
    /// Authored, not derived. D65 makes the block record the authored
    /// definition of a piece of work, and `created` is the date that
    /// authorship happened. Skipped when absent so blocks without it stay
    /// byte-identical across an `emit-state --write`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Status-transition date (`YYYY-MM-DD`) — when this block moved to
    /// `closed`.
    ///
    /// Authored. Paired with `commit` so a closed block records both *when*
    /// and *by what change*. D65's tombstone shape for a closed block is
    /// `{id, title, status, closed, commit}` — this is the `closed` half of
    /// that pair. Skipped when absent for the same round-trip-safety reason
    /// as every other optional field on this struct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
    /// Git hash of the commit that closed this block, for fast later
    /// retrieval — the `commit` half of D65's `{id, title, status, closed,
    /// commit}` tombstone shape for a closed block.
    ///
    /// Authored (or written by the closing automation), not derived by this
    /// crate. Skipped when absent so blocks without it stay byte-identical
    /// across an `emit-state --write`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Unmodeled fields, captured whole so an authored key this struct does
    /// not (yet) know about survives a deserialize→serialize round-trip
    /// instead of being silently dropped.
    ///
    /// This is the fix for the concrete incident where `TrackBlock` omitted
    /// `note` (which `docs/state/state-schema.md` has always documented as an
    /// authored field): every `mev emit-state --write` re-serialization
    /// silently deleted every note in the file, unattended, because
    /// `scripts/routine.sh` runs that command on cron. With this capture map,
    /// a stale binary that has never heard of a field cannot delete it —
    /// nothing ever tries to parse the field away, it just rides along in
    /// `extra`. Same whole-object property as qm's `durable-map.ts`, which
    /// stores each record as a whole JSONB object and reads it back as `T`
    /// with no field-by-field deserialization (see
    /// `agentic-portfolio/planning/qm-comparison-findings/notes.md` §7(g)).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// One phase/wave entry in a leaf repo's `tracks[]`.
///
/// Derives `Default` so downstream consumers construct this with
/// `..Default::default()`, naming only the fields they care about — a future
/// field addition here is then non-breaking for every such call site.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Track {
    /// Phase or wave name.
    pub title: String,
    /// Ordered blocks in this phase.
    #[serde(default)]
    pub blocks: Vec<TrackBlock>,
    /// Unmodeled fields, captured whole (see [`TrackBlock::extra`]).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// RepoRollup — brain `repos[]` child headline cache
// ---------------------------------------------------------------------------

/// One child repo's cached headline in a brain `repos[]` entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoRollup {
    /// Child repo slug.
    pub repo: String,
    /// Tier classification (e.g. `"core"`, `"portfolio"`).
    #[serde(default)]
    pub tier: Option<String>,
    /// Cached `focus.now` from the child.
    #[serde(default)]
    pub now: Vec<Block>,
    /// Cached `focus.next` from the child.
    #[serde(default)]
    pub next: Vec<Block>,
    /// Cached `focus.blocked` from the child.
    #[serde(default)]
    pub blocked: Vec<Block>,
    /// Cached `focus.deferred` from the child.
    ///
    /// Skipped when empty for the same no-churn reason as [`Focus::deferred`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred: Vec<Block>,
}

// ---------------------------------------------------------------------------
// CrossRepoEdge / Endpoint — brain `cross_repo[]`
// ---------------------------------------------------------------------------

/// One endpoint of a cross-repo dependency edge.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Endpoint {
    /// Repo slug.
    pub repo: String,
    /// Canonical block ID.
    #[serde(alias = "block")]
    pub id: String,
}

/// A directed cross-repo dependency edge in a brain `cross_repo[]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrossRepoEdge {
    /// Source endpoint (the dependent block).
    pub from: Endpoint,
    /// Target endpoint (the dependency).
    pub to: Endpoint,
    /// Optional explanation of why this edge exists.
    #[serde(default)]
    pub note: Option<String>,
}

// ---------------------------------------------------------------------------
// TierEntry — HQ `tiers[]`
// ---------------------------------------------------------------------------

/// One tier pointer in the HQ brain `tiers[]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TierEntry {
    /// Tier name (e.g. `"core"`).
    pub tier: String,
    /// Path or slug to the tier sub-brain, or `null`.
    #[serde(default)]
    pub rollup: Option<String>,
    /// One-line summary of the tier's current state.
    #[serde(default)]
    pub summary: Option<String>,
}

// ---------------------------------------------------------------------------
// Epic — HQ `epics[]` cross-repo initiative registry
// ---------------------------------------------------------------------------

/// One entry in the HQ brain's `epics[]` registry — a cross-repo initiative
/// that groups blocks spanning several repos.
///
/// The registry is **HQ-only** (same precedent as `backlog[]`, D2): it is the
/// closed vocabulary a block's `epics[]` membership is validated against, so a
/// typo is an error rather than a silently-empty board. Membership itself is
/// authored on the blocks, not listed here — `repos` is only a human hint.
///
/// Epic-to-epic relationships are **derived** from the block `depends_on`
/// graph, never authored here: an epic-level `depends_on` would duplicate truth
/// the block graph already holds.
///
/// Derives `Default` so downstream consumers construct this with
/// `..Default::default()`, naming only the fields they care about — a future
/// field addition here is then non-breaking for every such call site.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct Epic {
    /// Stable kebab-case key — the value blocks reference in their `epics[]`.
    pub slug: String,
    /// Human-readable name (e.g. `"Bastion OS"`).
    pub title: String,
    /// One-line description of what the initiative covers.
    #[serde(default)]
    pub description: Option<String>,
    /// Lifecycle: `active` · `focused` · `paused` · `complete`.
    #[serde(default)]
    pub status: Option<String>,
    /// Authored importance, `0..=100` — consumed by bastion-web's what-next
    /// ranking as one term of its score.
    ///
    /// Range validation lives in mev's `check_epics` (`E_STATE_EPIC_BAD_WEIGHT`),
    /// not here: okf-core holds data structs, not policy. `u8` therefore permits
    /// values above 100 at the type level on purpose.
    ///
    /// Absent means "consumer default" (bastion-web currently falls back to 60) —
    /// deliberately **not** `Default`-valued, so absent stays distinguishable
    /// from an authored `0`. `skip_serializing_if` keeps untagged epics
    /// byte-identical across a re-emit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u8>,
    /// Repo-relative path to the owning master-plan / plan doc, when one exists.
    #[serde(default)]
    pub plan: Option<String>,
    /// Repos the initiative is expected to touch. An authored hint for readers —
    /// **not** the source of truth (that's the blocks' own `epics[]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
    /// Unmodeled fields, captured whole (see [`TrackBlock::extra`]).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Origin — backlog→block promotion provenance (v2)
// ---------------------------------------------------------------------------

/// Provenance pointer on a block that was promoted from a backlog item, or
/// filed to resolve a carryover.
///
/// `kind` is a plain `String` rather than a Rust enum so that forward-compat
/// values round-trip losslessly through this struct — but the two values
/// producers write today are `"backlog"` (promoted from `backlog[]`) and
/// `"carryover"` (filed to resolve a `carryover[]` entry). Both take the
/// same `{type, slug}` shape: `slug` is the originating node's stable key
/// in the respective array.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Origin {
    /// Origin kind — `"backlog"` or `"carryover"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The originating backlog or carryover node's stable `slug` key.
    pub slug: String,
}

/// Provenance pointer on a **backlog node** — where the node itself came from.
///
/// Distinct from [`Origin`], which lives on a *block* and records which backlog
/// idea the block was promoted from (block→backlog). `BacklogOrigin` points the
/// other direction: it records whether a backlog node was a hand-authored
/// `/backlog-ticket` (`"backlog"`) or a promoted `/capture` note (`"capture"`).
/// `kind == "capture"` is the lane classifier for the Attention board's
/// "orphaned captures" sub-lane.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BacklogOrigin {
    /// Origin kind — `"capture"` or `"backlog"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Path to the pre-plan `notes.md`, when `kind == "capture"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// Backlog — HQ queued-ideas graph node (v2)
// ---------------------------------------------------------------------------

/// One entry in the HQ brain `backlog[]` — a queued idea as a graph node.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Backlog {
    /// Stable node key (the notes-dir slug).
    pub slug: String,
    /// Human description.
    pub title: String,
    /// Repo the item will land in when promoted (or `"cross-repo"`).
    pub repo: String,
    /// Item kind (`improvement` / `feature` / `chore` / `decision` / …).
    #[serde(rename = "type")]
    pub kind: String,
    /// Lifecycle status: `idea` / `ready` / `promoted`.
    pub status: String,
    /// What the idea is gated on — same edge forms as a block's `depends_on`.
    #[serde(default)]
    pub depends_on: Vec<BlockedBy>,
    /// Set only when `status == "promoted"`: the ID of the block it became.
    #[serde(default)]
    pub block: Option<String>,
    /// Path to the pre-plan notes doc.
    #[serde(default)]
    pub notes: Option<String>,
    /// Where this node came from — a hand-authored ticket or a `/capture` note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<BacklogOrigin>,
    /// Date recorded (`YYYY-MM-DD`). The staleness clock's anchor — a node with
    /// no `created` cannot age (kept `Option` for back-compat; backfilled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Last "keep / re-affirm" disposition date — a full staleness-clock reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<String>,
    /// Short-term "hide until" date, written by `/snooze`. Suppressed from the
    /// Attention board + warnings while `today < snoozed_until`, regardless of age.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<String>,
    /// Unmodeled fields, captured whole (see [`TrackBlock::extra`]).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Carryover — durable caveats / follow-ons (v3)
// ---------------------------------------------------------------------------

/// The scope of a `carryover[]` entry.
///
/// Derives `Default` as a prerequisite for [`Carryover`], whose `scope` field
/// is a non-`Option` `CarryoverScope`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct CarryoverScope {
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub cross_repo: Option<bool>,
}

/// The fixed, known `Carryover.kind` vocabulary (HQ D72).
///
/// `constraint` and `known_issue` are deliberately absent: they are leaving
/// the vocabulary and are handled by the `Unknown(String)` fallback on
/// [`CarryoverKind`] until Block G migrates the ~131 live entries still
/// using them.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KnownCarryoverKind {
    Defect,
    Deferred,
    Drift,
    Env,
}

/// `Carryover.kind`, typed with an `Unknown(String)` fallback.
///
/// Untagged: `Known` MUST stay the first variant. `#[serde(untagged)]` tries
/// variants in declaration order, and the reverse would swallow every known
/// value into `Unknown(String)`, silently defeating the whole point of the
/// enum while every round-trip test still passes.
///
/// Modelled on [`ClearsWhen`]'s untagged pattern: an unknown value degrades
/// to `Unknown(String)` rather than aborting deserialization of the entire
/// `state.json` file the way a bare (non-untagged) enum would — see
/// `core/mev/src/brain/state.rs:3411`, which pins that consequence for
/// `BlockedBy`. The original string is preserved rather than dropped, so a
/// round-trip is byte-identical even for values outside the known
/// vocabulary (e.g. the legacy `constraint` / `known_issue` values still on
/// live entries).
///
/// okf-core defines the SHAPE only and never validates or evaluates it
/// (AGENT.md rule 3); the known-vocabulary check lives in mev.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum CarryoverKind {
    /// One of the fixed, known kind values.
    Known(KnownCarryoverKind),
    /// Anything else — preserved verbatim rather than rejected or coerced.
    Unknown(String),
}

impl Default for CarryoverKind {
    /// Preserves today's default of an empty string.
    fn default() -> Self {
        CarryoverKind::Unknown(String::new())
    }
}

/// A durable caveat, known issue, environmental note, or deferred follow-on.
///
/// Derives `Default` so downstream consumers construct this with
/// `..Default::default()`, naming only the fields they care about — a future
/// field addition here is then non-breaking for every such call site.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Carryover {
    /// Stable node key.
    pub slug: String,
    /// Where it applies.
    pub scope: CarryoverScope,
    /// Item kind (`constraint`, `known_issue`, `env`, `deferred`).
    pub kind: CarryoverKind,
    /// The caveat / follow-on text.
    pub text: String,
    /// Optional related edges (same forms as blocked_by).
    #[serde(default)]
    pub related: Vec<BlockedBy>,
    /// Authored value-if-resolved priority, same 0..=3 money/deadline rubric as
    /// blocks and HQ backlog. NOT a statement about what this entry blocks —
    /// blocking-ness is DERIVED from `blocks[]` and is never authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    /// Edges to the work this carryover blocks. Same forms as `blocked_by` —
    /// `External { what }` covers "blocks every ticket run fleet-wide", which
    /// has no node target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<BlockedBy>,
    /// Free-form shared identity string linking the same finding filed in
    /// several repos. No registry, many-to-one by construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding_id: Option<String>,
    /// Condition under which this entry should be deleted: either free
    /// prose (the legacy and still-valid form for subjective conditions) or
    /// a typed predicate object whose `type` is one of `block_closed`,
    /// `file_exists`, `file_contains`, `command_exits_zero`.
    #[serde(default)]
    pub clears_when: Option<ClearsWhen>,
    /// Date recorded (`YYYY-MM-DD` or full RFC3339).
    pub created: String,
    /// Last "keep / re-affirm" disposition date — a full staleness-clock reset.
    /// Staleness age is measured from `max(created, reviewed)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<String>,
    /// Short-term "hide until" date, written by `/snooze`. Suppressed from the
    /// Attention board + warnings while `today < snoozed_until`, regardless of age.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<String>,
    /// Unmodeled fields, captured whole (see [`TrackBlock::extra`]).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A permanently-true reference entry: a trap, measured design invariant,
/// distilled lesson, or deliberate-choice marker that can never be "done".
///
/// Deliberately carries no `clears_when`, no `priority`, and no `blocks[]` —
/// those are precisely the fields a triage surface sorts and ranks on, so
/// omitting them makes "this is not work" structurally true rather than a
/// convention someone has to remember. See
/// `planning/ticket-reference-container-fields/tasks.md`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Reference {
    /// Stable node key.
    pub slug: String,
    /// Where it applies.
    pub scope: CarryoverScope,
    /// Reference kind (`trap`, `invariant`, `lesson`, `deliberate`).
    pub class: String,
    /// The reference text.
    pub text: String,
    /// Date recorded (`YYYY-MM-DD` or full RFC3339).
    pub created: String,
    /// Optional related edges (same forms as blocked_by).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<BlockedBy>,
    /// Last "keep / re-affirm" disposition date — a full freshness-clock reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<String>,
}

// ---------------------------------------------------------------------------
// StateFile — top-level structure
// ---------------------------------------------------------------------------

/// The deserialized contents of a `planning/state.json` file.
///
/// Both leaf (`kind:"project"`) and brain (`kind:"brain"`) variants are covered.
/// All optional collections default to empty; extra unknown fields are
/// tolerated (no `deny_unknown_fields`).
///
/// Derives `Default` so downstream consumers construct this with
/// `..Default::default()`, naming only the fields they care about — a future
/// field addition here is then non-breaking for every such call site.
/// `StateFile::default()` yields `repo: ""`, `kind: ""`, `updated: ""` — not a
/// meaningful state document; the derive exists for call-site ergonomics, not
/// to produce a valid document.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StateFile {
    /// Repo slug identifying this file's owner.
    pub repo: String,
    /// File variant: `"project"` or `"brain"`.
    pub kind: String,
    /// Freshness date string (presence only).
    pub updated: String,
    /// Current work status snapshot.
    #[serde(default)]
    pub focus: Focus,
    /// Roadmap catalog (leaf repos).
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// Child-repo headline cache (brain files).
    #[serde(default)]
    pub repos: Vec<RepoRollup>,
    /// Directed cross-repo dependency edges (brain files).
    #[serde(default)]
    pub cross_repo: Vec<CrossRepoEdge>,
    /// Tier pointers (HQ brain only).
    #[serde(default)]
    pub tiers: Vec<TierEntry>,
    /// Cross-repo initiative registry (HQ brain only; empty elsewhere).
    ///
    /// Skipped when empty so every non-HQ file stays byte-identical across an
    /// `emit-state --write`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub epics: Vec<Epic>,
    /// Optional top-level annotation note (seen in HQ state.json).
    #[serde(default)]
    pub note: Option<String>,
    /// HQ queued-ideas graph (brain HQ only; empty elsewhere).
    #[serde(default)]
    pub backlog: Vec<Backlog>,
    /// Durable caveats and follow-ons.
    #[serde(default)]
    pub carryover: Vec<Carryover>,
    /// Permanently-true reference material (traps, invariants, lessons,
    /// deliberate-choice markers) — structurally un-triageable by design. A
    /// `state.json` predating this field must not gain a stray `"reference": []`
    /// on write, hence `skip_serializing_if` here (unlike the legacy `carryover`
    /// field above, which this ticket does not touch).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference: Vec<Reference>,
    /// Unmodeled fields, captured whole (see [`TrackBlock::extra`]).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Read `path` and deserialize it as a [`StateFile`].
///
/// Returns [`StateLoadError::Io`] if the file cannot be read, or
/// [`StateLoadError::Parse`] if the contents are not valid JSON or do not
/// match the [`StateFile`] schema.
pub fn load_state(path: &Path) -> Result<StateFile, StateLoadError> {
    let contents = std::fs::read_to_string(path).map_err(|e| StateLoadError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    serde_json::from_str(&contents).map_err(|e| StateLoadError::Parse {
        path: path.to_path_buf(),
        source: e,
    })
}

// ---------------------------------------------------------------------------
// StateSource — discovery record (pure data; discovery itself stays in mev)
// ---------------------------------------------------------------------------

/// Metadata about a discovered `planning/state.json` file.
///
/// mev's `discover_state_files` (which *produces* these, walking the
/// filesystem against `BrainConfig`) stays in mev; this is just the pure
/// record that [`build_state_graph`] consumes.
#[derive(Debug, Clone)]
pub struct StateSource {
    /// Identifying slug for this source (e.g. `"hq"`, `"core"`, `"mev"`).
    pub repo_slug: String,
    /// Absolute path to the `planning/state.json` file.
    pub abs_path: PathBuf,
    /// Expected `kind` field value: `"brain"`, `"project"`, or `"portfolio"`.
    pub expected_kind: &'static str,
}

// ---------------------------------------------------------------------------
// State graph model (D4 serializable, emittable artifact)
// ---------------------------------------------------------------------------

/// The kind of a directed edge in the state block graph.
///
/// `BlockedBy` edges come from `tracks[].blocks[].depends_on[]{type:"block"}`
/// entries. `CrossRepo` edges come from brain-file `cross_repo[]` arrays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateEdgeKind {
    /// A `blocked_by` dependency (a block is waiting on another block).
    BlockedBy,
    /// An explicit cross-repo dependency declared in a brain file's `cross_repo[]`.
    CrossRepo,
}

/// A directed edge in the state block graph.
///
/// `from` and `to_ref` are canonical `"repo:id"` keys. `source_path` is kept
/// for diagnostic generation but **skipped** in serialization.
#[derive(Debug, Clone, Serialize)]
pub struct StateEdge {
    /// `"repo:id"` key of the source block (the dependent / blocked block).
    pub from: String,
    /// `"repo:id"` key of the target block (the dependency / blocker).
    pub to_ref: String,
    /// Edge discriminant.
    pub kind: StateEdgeKind,
    /// Absolute path of the file that authored this edge (skipped in JSON).
    #[serde(skip)]
    pub source_path: PathBuf,
}

/// A graph node — a block registered in a repo's `tracks[]`.
#[derive(Debug, Clone, Serialize)]
pub struct StateNode {
    /// Canonical key: `"repo:id"`.
    pub key: String,
    /// Repo slug that owns this block.
    pub repo: String,
    /// Canonical block ID (e.g. `"MV.3.P"`).
    pub id: String,
    /// Brief human description.
    pub title: String,
    /// Cross-repo epic membership, copied from the authoring `TrackBlock`, so
    /// graph consumers (and the emitted graph artifact) can filter by
    /// initiative without re-reading every `state.json`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub epics: Vec<String>,
    /// Absolute path of the file that registered this block (skipped in JSON).
    #[serde(skip)]
    pub source_path: PathBuf,
}

/// The serializable, emittable state block graph.
///
/// Produced by [`build_state_graph`]. The graph is authored-only — no node or
/// edge is inferred.
#[derive(Debug, Default, Serialize)]
pub struct StateGraph {
    /// All blocks registered in any repo's `tracks[]`.
    pub nodes: Vec<StateNode>,
    /// All `blocked_by` block edges and brain `cross_repo[]` edges.
    pub edges: Vec<StateEdge>,
}

/// Build a [`StateGraph`] from the loaded state files.
///
/// # Nodes
/// One [`StateNode`] per `tracks[].blocks[]` entry across all files (keyed
/// `"repo:id"`).
///
/// # Edges
/// - One [`StateEdge`] with `kind: BlockedBy` per `{type:"block"}` entry in
///   any file's `tracks[].blocks[].depends_on[]`. External entries are
///   skipped — they are leaf constraints, not graph edges.
/// - One [`StateEdge`] with `kind: CrossRepo` per brain-file `cross_repo[]`
///   entry.
pub fn build_state_graph(files: &[(StateSource, StateFile)]) -> StateGraph {
    let mut nodes: Vec<StateNode> = Vec::new();
    let mut edges: Vec<StateEdge> = Vec::new();

    for (src, file) in files {
        let path = &src.abs_path;

        // --- Nodes + BlockedBy edges: from tracks[].blocks[] ---
        for track in &file.tracks {
            for block in &track.blocks {
                let from_key = format!("{}:{}", src.repo_slug, block.id);

                nodes.push(StateNode {
                    key: from_key.clone(),
                    repo: src.repo_slug.clone(),
                    id: block.id.clone(),
                    title: block.title.clone(),
                    epics: block.epics.clone(),
                    source_path: path.clone(),
                });

                // BlockedBy edges: one per {type:block} depends_on entry.
                // External entries are leaf constraints, not graph edges — skip.
                for dep in &block.depends_on {
                    if let BlockedBy::Block(BlockDep { repo, id, .. }) = dep {
                        edges.push(StateEdge {
                            from: from_key.clone(),
                            to_ref: format!("{repo}:{id}"),
                            kind: StateEdgeKind::BlockedBy,
                            source_path: path.clone(),
                        });
                    }
                }
            }
        }

        // --- CrossRepo edges: from brain cross_repo[] ---
        for edge in &file.cross_repo {
            edges.push(StateEdge {
                from: format!("{}:{}", edge.from.repo, edge.from.id),
                to_ref: format!("{}:{}", edge.to.repo, edge.to.id),
                kind: StateEdgeKind::CrossRepo,
                source_path: path.clone(),
            });
        }
    }

    StateGraph { nodes, edges }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Both fixtures below are written *complete*: every field the structs
    // model is present explicitly (as a value, `null`, or `[]`), matching
    // exactly what serializing a parsed `StateFile` back out would produce.
    // This is deliberate, not incidental — `original == round` (see the
    // round-trip tests below) can only hold for fields *without*
    // `skip_serializing_if` if the original already carries them, because
    // those fields always serialize (Rust's `Option<T>` emits `null` for
    // `None`, `Vec<T>` emits `[]` for empty, absent `skip_serializing_if`).
    // Only the small set of fields the structs annotate with
    // `skip_serializing_if` (`epics`, `note`/`description` on `TrackBlock`,
    // `reviewed`, `snoozed_until`, `weight`, `deferred`, …) are genuinely
    // omittable — those are left out here on purpose, to also exercise that
    // they stay absent.
    fn project_fixture() -> &'static str {
        r#"{
            "repo": "bastion",
            "kind": "project",
            "updated": "2026-07-01",
            "note": null,
            "focus": {
                "now": [
                    {
                        "id": "BA.15.12", "title": "okf-core convergence",
                        "status": "in_progress", "note": null, "repo": null,
                        "blocked_by": [], "priority": null, "due": null
                    }
                ],
                "next": [
                    {
                        "id": "BA.15.13", "title": "next thing",
                        "status": null, "note": null, "repo": null,
                        "blocked_by": [], "priority": null, "due": null
                    }
                ],
                "blocked": [
                    {
                        "id": "BA.16.A", "title": "blocked thing",
                        "status": null, "note": null, "repo": null, "priority": null, "due": null,
                        "blocked_by": [
                            {"type": "block", "repo": "mev", "id": "MV.3.P", "what": null},
                            {"type": "external", "what": "waiting on infra"}
                        ]
                    }
                ]
            },
            "tracks": [
                {
                    "title": "Phase 15",
                    "blocks": [
                        {
                            "id": "BA.15.12",
                            "title": "okf-core convergence",
                            "status": "in_progress",
                            "depends_on": [
                                {"type": "block", "repo": "mev", "id": "MV.3.P", "what": null},
                                {"type": "external", "what": "waiting on infra"}
                            ],
                            "wave": 1,
                            "origin": null,
                            "priority": null,
                            "due": null,
                            "sdlc_workflow": null,
                            "model": null
                        }
                    ]
                }
            ],
            "repos": [],
            "cross_repo": [],
            "tiers": [],
            "backlog": [],
            "carryover": [
                {
                    "slug": "ba15-12-mev-context-seed",
                    "scope": {"repo": "bastion", "tier": null, "cross_repo": null},
                    "kind": "deferred",
                    "text": "seed mev context",
                    "related": [],
                    "clears_when": null,
                    "created": "2026-06-20"
                }
            ]
        }"#
    }

    fn brain_fixture() -> &'static str {
        r#"{
            "repo": "hq",
            "kind": "brain",
            "updated": "2026-07-01",
            "note": null,
            "focus": {"now": [], "next": [], "blocked": []},
            "tracks": [],
            "backlog": [],
            "repos": [
                {"repo": "bastion", "tier": "core", "now": [], "next": [], "blocked": []},
                {"repo": "mev", "tier": "core", "now": [], "next": [], "blocked": []}
            ],
            "cross_repo": [
                {
                    "from": {"repo": "bastion", "id": "BA.15.12"},
                    "to": {"repo": "mev", "id": "MV.3.P"},
                    "note": "okf-core convergence"
                }
            ],
            "tiers": [
                {"tier": "core", "rollup": "core/status.md", "summary": "on track"}
            ],
            "carryover": []
        }"#
    }

    #[test]
    fn load_state_roundtrip_real_fixture() {
        let file: StateFile = serde_json::from_str(project_fixture()).unwrap();
        let round: serde_json::Value = serde_json::to_value(&file).unwrap();
        let original: serde_json::Value = serde_json::from_str(project_fixture()).unwrap();

        // The assertion that matters: the model's re-serialization against the
        // JSON as it was actually parsed from disk. Comparing `round` to
        // `reserialized` (model vs. model) is a blind spot — any field the
        // struct does not model is dropped identically on both sides, so that
        // comparison can never detect a dropped field. Comparing `serde_json::Value`
        // (not strings) is deliberate: key order and whitespace are not part of
        // the contract, and `Value`'s map equality is order-independent.
        assert_eq!(original, round);

        assert_eq!(file.repo, "bastion");
        assert_eq!(file.kind, "project");
        assert_eq!(file.focus.now.len(), 1);
        assert_eq!(file.focus.now[0].id, "BA.15.12");
        assert_eq!(file.tracks[0].blocks[0].depends_on.len(), 2);
        assert_eq!(file.carryover[0].slug, "ba15-12-mev-context-seed");
    }

    #[test]
    fn load_state_brain_fixture() {
        let file: StateFile = serde_json::from_str(brain_fixture()).unwrap();
        let round: serde_json::Value = serde_json::to_value(&file).unwrap();
        let original: serde_json::Value = serde_json::from_str(brain_fixture()).unwrap();

        // Same original-vs-round coverage as the project fixture above, so the
        // brain variant (repos/cross_repo/tiers shape) is also protected against
        // a silently-dropped field.
        assert_eq!(original, round);

        assert_eq!(file.repos.len(), 2);
        assert_eq!(file.cross_repo.len(), 1);
        assert_eq!(file.tiers.len(), 1);
    }

    #[test]
    fn blocked_by_unknown_type_is_rejected() {
        let bad = r#"{
            "repo": "bastion",
            "kind": "project",
            "updated": "2026-07-01",
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {
                            "id": "X",
                            "title": "x",
                            "depends_on": [{"type": "bogus", "repo": "a", "id": "b"}]
                        }
                    ]
                }
            ]
        }"#;
        let result: Result<StateFile, _> = serde_json::from_str(bad);
        assert!(result.is_err());
    }

    #[test]
    fn old_session_type_name_is_rejected() {
        // The `session` -> `operator` rename (2026-08-11) must not leave the old
        // discriminant silently parseable as anything, including a no-op edge.
        let json = r#"{"type":"session","slug":"session-mac-mini","exit":"planning/handoff.md","start":"/begin-session mac-mini"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn operator_edge_missing_type_is_rejected() {
        let json = r#"{"type":"operator"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn operator_edge_missing_slug_is_rejected() {
        let json =
            r#"{"type":"operator","exit":"planning/handoff.md","start":"/begin-session mac-mini"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn operator_edge_missing_exit_is_rejected() {
        let json =
            r#"{"type":"operator","slug":"session-mac-mini","start":"/begin-session mac-mini"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn operator_edge_missing_start_is_rejected() {
        let json = r#"{"type":"operator","slug":"session-mac-mini","exit":"planning/handoff.md"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn approval_edge_missing_type_is_rejected() {
        let json = r#"{"type":"approval"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn approval_edge_missing_slug_is_rejected() {
        let json = r#"{"type":"approval","what":"publish the four Dev.to tag edits","digest":"sha256:abcd1234"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn approval_edge_missing_what_is_rejected() {
        let json = r#"{"type":"approval","slug":"approve-devto-sweep","digest":"sha256:abcd1234"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn approval_edge_missing_digest_is_rejected() {
        let json = r#"{"type":"approval","slug":"approve-devto-sweep","what":"publish the four Dev.to tag edits"}"#;
        let result: Result<BlockedBy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn operator_edge_without_what_roundtrips() {
        let json = r#"{"type":"operator","slug":"session-mac-mini","exit":"planning/handoff.md","start":"/begin-session mac-mini"}"#;
        let edge: BlockedBy = serde_json::from_str(json).unwrap();
        let expected = BlockedBy::Operator(OperatorDep {
            slug: "session-mac-mini".to_string(),
            exit: "planning/handoff.md".to_string(),
            start: "/begin-session mac-mini".to_string(),
            what: None,
        });
        assert_eq!(edge, expected);

        let serialized = serde_json::to_string(&edge).unwrap();
        assert!(serialized.contains("\"type\":\"operator\""));

        let reparsed: BlockedBy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed, edge);
    }

    #[test]
    fn operator_edge_with_what_roundtrips() {
        let json = r#"{"type":"operator","slug":"session-mac-mini","exit":"planning/handoff.md","start":"/begin-session mac-mini","what":"gates the Mini restart"}"#;
        let edge: BlockedBy = serde_json::from_str(json).unwrap();
        let expected = BlockedBy::Operator(OperatorDep {
            slug: "session-mac-mini".to_string(),
            exit: "planning/handoff.md".to_string(),
            start: "/begin-session mac-mini".to_string(),
            what: Some("gates the Mini restart".to_string()),
        });
        assert_eq!(edge, expected);

        let serialized = serde_json::to_string(&edge).unwrap();
        assert!(serialized.contains("\"type\":\"operator\""));

        let reparsed: BlockedBy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed, edge);
    }

    #[test]
    fn approval_edge_roundtrips() {
        let json = r#"{"type":"approval","slug":"approve-devto-sweep","what":"publish the four Dev.to tag edits","digest":"sha256:abcd1234"}"#;
        let edge: BlockedBy = serde_json::from_str(json).unwrap();
        let expected = BlockedBy::Approval(ApprovalDep {
            slug: "approve-devto-sweep".to_string(),
            what: "publish the four Dev.to tag edits".to_string(),
            digest: "sha256:abcd1234".to_string(),
        });
        assert_eq!(edge, expected);

        let serialized = serde_json::to_string(&edge).unwrap();
        assert!(serialized.contains("\"type\":\"approval\""));

        let reparsed: BlockedBy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed, edge);
    }

    #[test]
    fn block_id_alias_reads_v1_key() {
        let v1 = r#"{"block": "BA.1.A", "title": "legacy"}"#;
        let block: Block = serde_json::from_str(v1).unwrap();
        assert_eq!(block.id, "BA.1.A");
    }

    #[test]
    fn load_state_missing_file_is_io_error() {
        let err = load_state(Path::new("/nonexistent/path/state.json")).unwrap_err();
        match err {
            StateLoadError::Io { .. } => {}
            other => panic!("expected Io error, got {other:?}"),
        }
    }

    #[test]
    fn load_state_malformed_json_is_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "{ not json").unwrap();

        let err = load_state(&path).unwrap_err();
        match err {
            StateLoadError::Parse { .. } => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    fn state_source(repo_slug: &str, path: &Path) -> StateSource {
        StateSource {
            repo_slug: repo_slug.to_string(),
            abs_path: path.to_path_buf(),
            expected_kind: "project",
        }
    }

    #[test]
    fn build_state_graph_nodes_and_edges() {
        let repo_a_json = r#"{
            "repo": "a",
            "kind": "project",
            "updated": "2026-07-01",
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {
                            "id": "X",
                            "title": "x",
                            "depends_on": [
                                {"type": "block", "repo": "b", "id": "Y"},
                                {"type": "external", "what": "infra"}
                            ]
                        }
                    ]
                }
            ]
        }"#;
        let repo_b_json = r#"{
            "repo": "b",
            "kind": "project",
            "updated": "2026-07-01",
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {"id": "Y", "title": "y"}
                    ]
                }
            ]
        }"#;

        let path_a = PathBuf::from("/repos/a/planning/state.json");
        let path_b = PathBuf::from("/repos/b/planning/state.json");
        let file_a: StateFile = serde_json::from_str(repo_a_json).unwrap();
        let file_b: StateFile = serde_json::from_str(repo_b_json).unwrap();

        let files = vec![
            (state_source("a", &path_a), file_a),
            (state_source("b", &path_b), file_b),
        ];

        let graph = build_state_graph(&files);

        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.iter().any(|n| n.key == "a:X"));
        assert!(graph.nodes.iter().any(|n| n.key == "b:Y"));

        let blocked_by_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == StateEdgeKind::BlockedBy)
            .collect();
        assert_eq!(blocked_by_edges.len(), 1);
        assert_eq!(blocked_by_edges[0].from, "a:X");
        assert_eq!(blocked_by_edges[0].to_ref, "b:Y");
    }

    #[test]
    fn build_state_graph_skips_operator_and_approval_edges() {
        // Regression pin for the graph builder at build_state_graph: a block
        // whose depends_on[] carries an operator edge and an approval edge
        // must produce zero graph edges and zero nodes for those targetless
        // entries — exactly as {type:external} is skipped today. If either
        // new variant ever starts emitting an edge, it would point at a
        // non-existent node and surface as a dangling reference.
        let repo_a_json = r#"{
            "repo": "a",
            "kind": "project",
            "updated": "2026-07-01",
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {
                            "id": "X",
                            "title": "x",
                            "depends_on": [
                                {
                                    "type": "operator",
                                    "slug": "close-session-decision",
                                    "exit": "planning/decision.md",
                                    "start": "/begin-session close-session-decision"
                                },
                                {
                                    "type": "approval",
                                    "slug": "ship-it",
                                    "what": "Ship the change",
                                    "digest": "sha256:abc123"
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let path_a = PathBuf::from("/repos/a/planning/state.json");
        let file_a: StateFile = serde_json::from_str(repo_a_json).unwrap();
        let files = vec![(state_source("a", &path_a), file_a)];

        let graph = build_state_graph(&files);

        // The one authored block is still a node...
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].key, "a:X");
        // ...but neither the operator edge nor the approval edge produced a
        // graph edge, so there is nothing that could point at a dangling
        // "close-session-decision" or "ship-it" node.
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn build_state_graph_cross_repo_edge() {
        let brain_json = r#"{
            "repo": "hq",
            "kind": "brain",
            "updated": "2026-07-01",
            "cross_repo": [
                {
                    "from": {"repo": "bastion", "id": "BA.15.12"},
                    "to": {"repo": "mev", "id": "MV.3.P"}
                }
            ]
        }"#;
        let path = PathBuf::from("/repos/hq/planning/state.json");
        let file: StateFile = serde_json::from_str(brain_json).unwrap();
        let files = vec![(state_source("hq", &path), file)];

        let graph = build_state_graph(&files);

        assert_eq!(graph.nodes.len(), 0);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].kind, StateEdgeKind::CrossRepo);
        assert_eq!(graph.edges[0].from, "bastion:BA.15.12");
        assert_eq!(graph.edges[0].to_ref, "mev:MV.3.P");
    }

    #[test]
    fn block_shape_fields_roundtrip() {
        let json_with_fields = r#"{
            "repo": "okf-core",
            "kind": "project",
            "updated": "2026-07-05",
            "focus": {
                "now": [
                    {
                        "id": "BA.1.A",
                        "title": "with fields",
                        "priority": 1,
                        "due": "2026-07-15"
                    }
                ]
            },
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {
                            "id": "OK.1.A",
                            "title": "block shape",
                            "priority": 2,
                            "due": "Q3",
                            "sdlc_workflow": "sdlc-task",
                            "model": "opus"
                        },
                        {
                            "id": "OK.1.B",
                            "title": "missing fields"
                        }
                    ]
                }
            ]
        }"#;

        let file: StateFile = serde_json::from_str(json_with_fields).unwrap();

        // Assert TrackBlock fields
        let block_a = &file.tracks[0].blocks[0];
        assert_eq!(block_a.priority, Some(2));
        assert_eq!(block_a.due.as_deref(), Some("Q3"));
        assert_eq!(block_a.sdlc_workflow.as_deref(), Some("sdlc-task"));
        assert_eq!(block_a.model.as_deref(), Some("opus"));

        // Assert backwards compatibility (omitted fields default to None)
        let block_b = &file.tracks[0].blocks[1];
        assert_eq!(block_b.priority, None);
        assert_eq!(block_b.due, None);
        assert_eq!(block_b.sdlc_workflow, None);
        assert_eq!(block_b.model, None);

        // Assert Block fields
        let focus_now = &file.focus.now[0];
        assert_eq!(focus_now.priority, Some(1));
        assert_eq!(focus_now.due.as_deref(), Some("2026-07-15"));

        // Roundtrip model fidelity
        let round = serde_json::to_value(&file).unwrap();
        let reparsed: StateFile = serde_json::from_value(round.clone()).unwrap();
        let reserialized = serde_json::to_value(&reparsed).unwrap();
        assert_eq!(round, reserialized);
    }

    #[test]
    fn attention_fields_roundtrip_and_omit_when_absent() {
        // Backlog + carryover carrying the new attention fields parse fully...
        let json = r#"{
            "repo": "hq",
            "kind": "brain",
            "updated": "2026-07-15",
            "backlog": [
                {
                    "slug": "tailscale-rag-db",
                    "title": "Tailscale RAG DB",
                    "repo": "core",
                    "type": "research",
                    "status": "idea",
                    "created": "2026-06-16",
                    "origin": {"type": "capture", "notes": "core/planning/tailscale-rag-db/notes.md"},
                    "reviewed": "2026-07-10",
                    "snoozed_until": "2026-07-20"
                }
            ],
            "carryover": [
                {
                    "slug": "cortex-rename",
                    "scope": {"cross_repo": true},
                    "kind": "deferred",
                    "text": "rename mev to cortex",
                    "created": "2026-07-05T07:10:00-03:00",
                    "reviewed": "2026-07-12"
                }
            ]
        }"#;
        let file: StateFile = serde_json::from_str(json).unwrap();
        let bl = &file.backlog[0];
        assert_eq!(bl.created.as_deref(), Some("2026-06-16"));
        assert_eq!(bl.reviewed.as_deref(), Some("2026-07-10"));
        assert_eq!(bl.snoozed_until.as_deref(), Some("2026-07-20"));
        let origin = bl.origin.as_ref().expect("origin present");
        assert_eq!(origin.kind, "capture");
        assert_eq!(
            origin.notes.as_deref(),
            Some("core/planning/tailscale-rag-db/notes.md")
        );
        assert_eq!(file.carryover[0].reviewed.as_deref(), Some("2026-07-12"));

        // ...and when the new fields are absent they must NOT serialize as `null`
        // (skip_serializing_if), so first `emit-state --write` adds no noise.
        let bare = Backlog {
            slug: "x".to_string(),
            title: "X".to_string(),
            repo: "core".to_string(),
            kind: "feature".to_string(),
            status: "idea".to_string(),
            ..Default::default()
        };
        let v = serde_json::to_value(&bare).unwrap();
        let obj = v.as_object().unwrap();
        assert!(
            !obj.contains_key("origin"),
            "origin must be omitted when None"
        );
        assert!(
            !obj.contains_key("created"),
            "created must be omitted when None"
        );
        assert!(
            !obj.contains_key("reviewed"),
            "reviewed must be omitted when None"
        );
        assert!(
            !obj.contains_key("snoozed_until"),
            "snoozed_until must be omitted when None"
        );
    }

    // -- created / closed / commit / carryover origin -------------------------

    #[test]
    fn track_block_created_closed_commit_roundtrip_when_populated() {
        // Every field the struct models is present explicitly, including the
        // new created/closed/commit trio and a carryover-kind origin — same
        // discipline as `project_fixture()` above, so `original == round` can
        // only hold if nothing gets silently dropped on re-serialization.
        let json = r#"{
            "id": "OK.ticket.block-vocab-extension",
            "title": "Extend TrackBlock",
            "status": "closed",
            "depends_on": [],
            "wave": 1,
            "origin": {"type": "carryover", "slug": "cortex-rename"},
            "note": "closed cleanly",
            "description": "adds created/closed/commit",
            "priority": 1,
            "due": "2026-08-20",
            "sdlc_workflow": "task",
            "model": "sonnet",
            "epics": ["okf-vocab"],
            "created": "2026-08-16",
            "closed": "2026-08-17",
            "commit": "abc1234"
        }"#;
        let block: TrackBlock = serde_json::from_str(json).unwrap();
        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let round: serde_json::Value = serde_json::to_value(&block).unwrap();
        assert_eq!(original, round);

        assert_eq!(block.created.as_deref(), Some("2026-08-16"));
        assert_eq!(block.closed.as_deref(), Some("2026-08-17"));
        assert_eq!(block.commit.as_deref(), Some("abc1234"));
        let origin = block.origin.as_ref().expect("origin present");
        assert_eq!(origin.kind, "carryover");
        assert_eq!(origin.slug, "cortex-rename");
    }

    #[test]
    fn track_block_created_closed_commit_omitted_when_absent() {
        // A block carrying none of the new fields must round-trip
        // byte-identically too — the property the `note` deletion bug
        // violated. Asserting only field presence (as opposed to full
        // `Value` equality against the original JSON) would not have caught
        // that bug, so this compares whole objects, not just key absence.
        let json = r#"{
            "id": "OK.other",
            "title": "Untouched block",
            "status": "open",
            "depends_on": [],
            "wave": null,
            "origin": null,
            "priority": null,
            "due": null,
            "sdlc_workflow": null,
            "model": null
        }"#;
        let block: TrackBlock = serde_json::from_str(json).unwrap();
        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let round: serde_json::Value = serde_json::to_value(&block).unwrap();
        assert_eq!(original, round);

        let obj = round.as_object().unwrap();
        assert!(
            !obj.contains_key("created"),
            "created must be omitted when None"
        );
        assert!(
            !obj.contains_key("closed"),
            "closed must be omitted when None"
        );
        assert!(
            !obj.contains_key("commit"),
            "commit must be omitted when None"
        );
        assert_eq!(block.created, None);
        assert_eq!(block.closed, None);
        assert_eq!(block.commit, None);
    }

    // -- epics ---------------------------------------------------------------

    #[test]
    fn epics_round_trip_on_blocks_and_registry() {
        let json = r#"{
            "repo": "hq",
            "kind": "brain",
            "updated": "2026-07-24",
            "epics": [
                {
                    "slug": "bastion-os",
                    "title": "Bastion OS",
                    "description": "The five-layer practice OS",
                    "status": "active",
                    "plan": "core/planning/master-plan.md",
                    "repos": ["bastion", "mev"]
                }
            ],
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {
                            "id": "HQ.1.A",
                            "title": "shared block",
                            "status": "open",
                            "epics": ["bastion-os", "bastion-web"]
                        }
                    ]
                }
            ]
        }"#;

        let file: StateFile = serde_json::from_str(json).expect("parses");

        assert_eq!(file.epics.len(), 1);
        let epic = &file.epics[0];
        assert_eq!(epic.slug, "bastion-os");
        assert_eq!(epic.title, "Bastion OS");
        assert_eq!(epic.status.as_deref(), Some("active"));
        assert_eq!(epic.plan.as_deref(), Some("core/planning/master-plan.md"));
        assert_eq!(epic.repos, vec!["bastion", "mev"]);

        assert_eq!(
            file.tracks[0].blocks[0].epics,
            vec!["bastion-os", "bastion-web"],
            "multi-valued membership must survive the parse"
        );

        // Re-serialize and re-parse: the authored membership survives the exact
        // round-trip `plan_state_json` performs on every `emit-state --write`.
        let out = serde_json::to_string(&file).expect("serializes");
        let again: StateFile = serde_json::from_str(&out).expect("re-parses");
        assert_eq!(
            again.tracks[0].blocks[0].epics,
            vec!["bastion-os", "bastion-web"]
        );
        assert_eq!(again.epics[0].slug, "bastion-os");
    }

    /// Parse a one-epic HQ registry from an inline epic object body.
    fn epic_from(body: &str) -> (Epic, serde_json::Value) {
        let json =
            format!(r#"{{"repo":"hq","kind":"brain","updated":"2026-08-01","epics":[{body}]}}"#);
        let file: StateFile = serde_json::from_str(&json).expect("parses");
        let out = serde_json::to_value(&file).expect("serializes");
        let epic_json = out["epics"][0].clone();
        (file.epics.into_iter().next().expect("one epic"), epic_json)
    }

    #[test]
    fn epic_without_weight_round_trips_without_the_key() {
        // The no-churn contract: the ~13 existing registry entries author no
        // weight, so `emit-state --write` must not add `"weight"` to any of them.
        let (epic, json) = epic_from(r#"{"slug":"brain-quality","title":"Brain Quality"}"#);
        assert_eq!(epic.weight, None);
        assert!(
            json.get("weight").is_none(),
            "Epic.weight must be omitted when absent, got: {json}"
        );
    }

    #[test]
    fn epic_weight_zero_round_trips_as_zero() {
        // Absent and zero must stay distinguishable — hence no `Default` value.
        let (epic, json) = epic_from(r#"{"slug":"parked","title":"Parked","weight":0}"#);
        assert_eq!(epic.weight, Some(0));
        assert_eq!(json.get("weight"), Some(&serde_json::json!(0)));
    }

    #[test]
    fn epic_weight_round_trips_in_range() {
        let (epic, json) = epic_from(r#"{"slug":"bastion-os","title":"Bastion OS","weight":100}"#);
        assert_eq!(epic.weight, Some(100));
        assert_eq!(json.get("weight"), Some(&serde_json::json!(100)));
    }

    #[test]
    fn epic_weight_above_100_still_deserializes() {
        // Range is mev's `check_epics` (E_STATE_EPIC_BAD_WEIGHT), not serde's:
        // okf-core holds data structs, not policy.
        let (epic, _) = epic_from(r#"{"slug":"loud","title":"Loud","weight":200}"#);
        assert_eq!(epic.weight, Some(200));
    }

    #[test]
    fn absent_epics_do_not_serialize_as_empty_arrays() {
        // The whole corpus (~292 blocks) belongs to no epic today. Round-tripping
        // a state file must not add `"epics": []` to every block / file, or the
        // first `emit-state --write` after this schema change rewrites everything.
        let file: StateFile = serde_json::from_str(project_fixture()).expect("parses");
        let v = serde_json::to_value(&file).expect("serializes");
        let obj = v.as_object().unwrap();

        assert!(
            !obj.contains_key("epics"),
            "StateFile.epics must be omitted when empty"
        );

        let block = &v["tracks"][0]["blocks"][0];
        assert!(
            block.get("epics").is_none(),
            "TrackBlock.epics must be omitted when empty, got: {block}"
        );

        let focus_now = &v["focus"]["now"][0];
        assert!(
            focus_now.get("epics").is_none(),
            "Block.epics must be omitted when empty, got: {focus_now}"
        );
    }

    #[test]
    fn absent_deferred_lane_reads_as_empty() {
        // Every state.json in the portfolio predates the `deferred` lane. They
        // must all still load, with the lane defaulting to empty rather than
        // failing as a missing field.
        let file: StateFile = serde_json::from_str(project_fixture()).expect("parses");
        assert!(file.focus.deferred.is_empty());

        let brain: StateFile = serde_json::from_str(brain_fixture()).expect("parses");
        assert!(brain.focus.deferred.is_empty());
        for rollup in &brain.repos {
            assert!(
                rollup.deferred.is_empty(),
                "RepoRollup.deferred must default to empty for {}",
                rollup.repo
            );
        }
    }

    #[test]
    fn absent_deferred_lane_does_not_serialize_as_empty_array() {
        // Same no-churn contract as `absent_epics_do_not_serialize_as_empty_arrays`:
        // `plan_state_json` diffs pretty-printed JSON, so an empty lane that
        // serializes to `"deferred": []` would rewrite every state.json in the
        // portfolio on the first `emit-state --write` after this change.
        let file: StateFile = serde_json::from_str(project_fixture()).expect("parses");
        let v = serde_json::to_value(&file).expect("serializes");

        let focus = &v["focus"];
        assert!(
            focus.get("deferred").is_none(),
            "Focus.deferred must be omitted when empty, got: {focus}"
        );

        let brain: StateFile = serde_json::from_str(brain_fixture()).expect("parses");
        let bv = serde_json::to_value(&brain).expect("serializes");
        for rollup in bv["repos"].as_array().into_iter().flatten() {
            assert!(
                rollup.get("deferred").is_none(),
                "RepoRollup.deferred must be omitted when empty, got: {rollup}"
            );
        }
    }

    #[test]
    fn deferred_lane_round_trips_when_present() {
        let json = r#"{
            "repo": "mev",
            "kind": "project",
            "updated": "2026-07-26",
            "focus": {
                "deferred": [
                    {"id": "MV.9.A", "title": "back burner thing", "status": "deferred"}
                ]
            },
            "tracks": [
                {
                    "title": "Phase 9",
                    "blocks": [
                        {"id": "MV.9.A", "title": "back burner thing", "status": "deferred"}
                    ]
                }
            ]
        }"#;
        let file: StateFile = serde_json::from_str(json).expect("parses");

        assert_eq!(file.focus.deferred.len(), 1);
        assert_eq!(file.focus.deferred[0].id, "MV.9.A");
        assert_eq!(file.focus.deferred[0].status.as_deref(), Some("deferred"));
        assert_eq!(
            file.tracks[0].blocks[0].status.as_deref(),
            Some("deferred"),
            "`deferred` must survive as an authored track-block status"
        );

        let v = serde_json::to_value(&file).expect("serializes");
        assert_eq!(v["focus"]["deferred"][0]["id"], "MV.9.A");
    }

    #[test]
    fn build_state_graph_carries_epic_membership_onto_nodes() {
        let json = r#"{
            "repo": "bastion",
            "kind": "project",
            "updated": "2026-07-24",
            "tracks": [
                {
                    "title": "Phase 11",
                    "blocks": [
                        {"id": "BA.11.K", "title": "board endpoint", "status": "closed",
                         "epics": ["bastion-os", "bastion-web"]},
                        {"id": "BA.12.A", "title": "untagged", "status": "open"}
                    ]
                }
            ]
        }"#;
        let file: StateFile = serde_json::from_str(json).expect("parses");
        let src = StateSource {
            repo_slug: "bastion".to_string(),
            abs_path: PathBuf::from("/tmp/bastion/planning/state.json"),
            expected_kind: "project",
        };

        let graph = build_state_graph(&[(src, file)]);

        let tagged = graph
            .nodes
            .iter()
            .find(|n| n.id == "BA.11.K")
            .expect("BA.11.K node");
        assert_eq!(tagged.epics, vec!["bastion-os", "bastion-web"]);

        let untagged = graph
            .nodes
            .iter()
            .find(|n| n.id == "BA.12.A")
            .expect("BA.12.A node");
        assert!(untagged.epics.is_empty());
    }

    #[test]
    fn origin_deserializes_backlog_kind() {
        let origin: Origin =
            serde_json::from_str(r#"{"type":"backlog","slug":"ba15-12-mev-context-seed"}"#)
                .expect("backlog origin parses");
        assert_eq!(origin.kind, "backlog");
        assert_eq!(origin.slug, "ba15-12-mev-context-seed");
    }

    #[test]
    fn origin_deserializes_carryover_kind() {
        // A ticket filed to resolve a carryover keeps provenance the same
        // {type, slug} shape as a backlog promotion — see D65.
        let origin: Origin =
            serde_json::from_str(r#"{"type":"carryover","slug":"note-deletion-bug"}"#)
                .expect("carryover origin parses");
        assert_eq!(origin.kind, "carryover");
        assert_eq!(origin.slug, "note-deletion-bug");

        let json = serde_json::to_value(&origin).expect("serializes");
        assert_eq!(
            json,
            serde_json::json!({"type": "carryover", "slug": "note-deletion-bug"})
        );
    }

    // -----------------------------------------------------------------
    // OK.ticket.op-slug-stutter-warning: op_id / op_slug_stutters /
    // normalize_op_slug
    // -----------------------------------------------------------------

    /// One shared case table — (slug, expects_stutter, expected_normalized,
    /// expected_op_id) — asserted against the free functions (this test)
    /// and, in a separate test, against `OperatorDep`/`ApprovalDep`'s
    /// inherent methods, so the two call sites cannot silently drift apart.
    ///
    /// `operator-` and `operators-guild` are the boundary cases a naive
    /// `starts_with("operator")` or a naive strip gets wrong: `operator-`
    /// has no remainder to strip, and `operators-guild`'s prefix is
    /// `operators-`, not the exact literal `operator-`.
    fn op_slug_cases() -> Vec<(&'static str, bool, &'static str, &'static str)> {
        vec![
            (
                "mac-mini-visit",
                false,
                "mac-mini-visit",
                "OP.mac-mini-visit",
            ),
            (
                "operator-mac-mini-visit",
                true,
                "mac-mini-visit",
                "OP.operator-mac-mini-visit",
            ),
            ("operator", false, "operator", "OP.operator"),
            ("operator-", false, "operator-", "OP.operator-"),
            (
                "operators-guild",
                false,
                "operators-guild",
                "OP.operators-guild",
            ),
            (
                "operator-operator-x",
                true,
                "operator-x",
                "OP.operator-operator-x",
            ),
            ("", false, "", "OP."),
        ]
    }

    #[test]
    fn op_slug_free_functions_match_table() {
        for (slug, expects_stutter, expected_normalized, expected_op_id) in op_slug_cases() {
            assert_eq!(
                op_slug_stutters(slug),
                expects_stutter,
                "op_slug_stutters({slug:?})"
            );
            assert_eq!(
                normalize_op_slug(slug),
                expected_normalized,
                "normalize_op_slug({slug:?})"
            );
            assert_eq!(op_id(slug), expected_op_id, "op_id({slug:?})");
        }
    }

    /// Same table as [`op_slug_free_functions_match_table`], walked through
    /// `OperatorDep`/`ApprovalDep`'s inherent methods instead of the free
    /// functions, so the delegation (task 2) cannot silently drift from the
    /// rule it delegates to.
    #[test]
    fn op_slug_inherent_methods_match_table() {
        for (slug, expects_stutter, _expected_normalized, expected_op_id) in op_slug_cases() {
            let operator = OperatorDep {
                slug: slug.to_string(),
                exit: "planning/handoff.md".to_string(),
                start: "/begin-session example".to_string(),
                what: None,
            };
            assert_eq!(
                operator.slug_stutters(),
                expects_stutter,
                "OperatorDep::slug_stutters({slug:?})"
            );
            assert_eq!(
                operator.op_id(),
                expected_op_id,
                "OperatorDep::op_id({slug:?})"
            );

            let approval = ApprovalDep {
                slug: slug.to_string(),
                what: "example decision".to_string(),
                digest: "sha256:example".to_string(),
            };
            assert_eq!(
                approval.slug_stutters(),
                expects_stutter,
                "ApprovalDep::slug_stutters({slug:?})"
            );
            assert_eq!(
                approval.op_id(),
                expected_op_id,
                "ApprovalDep::op_id({slug:?})"
            );
        }
    }

    /// A `state.json` fixture carrying one operator edge and one approval
    /// edge must serialize byte-identically (via `serde_json::Value`
    /// equality — key order/whitespace are not part of the contract, see
    /// `load_state_roundtrip_real_fixture` above) to its input after a
    /// load-then-serialize cycle. Pins that this ticket moved no wire shape:
    /// no field, serde attribute, or typeshare annotation changed on either
    /// struct.
    #[test]
    fn operator_and_approval_edges_roundtrip_byte_identical() {
        let json = r#"{
            "repo": "bastion",
            "kind": "project",
            "updated": "2026-08-20",
            "note": null,
            "focus": {"now": [], "next": [], "blocked": []},
            "tracks": [
                {
                    "title": "Phase 1",
                    "blocks": [
                        {
                            "id": "BA.1.A",
                            "title": "gated block",
                            "status": "open",
                            "depends_on": [
                                {
                                    "type": "operator",
                                    "slug": "operator-mac-mini-visit",
                                    "exit": "planning/handoff.md",
                                    "start": "/begin-session operator-mac-mini-visit",
                                    "what": null
                                },
                                {
                                    "type": "approval",
                                    "slug": "approve-devto-sweep",
                                    "what": "publish the four Dev.to tag edits",
                                    "digest": "sha256:abcd1234"
                                }
                            ],
                            "wave": 1,
                            "origin": null,
                            "priority": null,
                            "due": null,
                            "sdlc_workflow": null,
                            "model": null
                        }
                    ]
                }
            ],
            "repos": [],
            "cross_repo": [],
            "tiers": [],
            "backlog": [],
            "carryover": []
        }"#;

        let file: StateFile = serde_json::from_str(json).unwrap();
        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let round: serde_json::Value = serde_json::to_value(&file).unwrap();
        assert_eq!(original, round);

        let deps = &file.tracks[0].blocks[0].depends_on;
        assert_eq!(deps.len(), 2);
        assert!(
            matches!(&deps[0], BlockedBy::Operator(op) if op.slug == "operator-mac-mini-visit")
        );
        assert!(matches!(&deps[1], BlockedBy::Approval(ap) if ap.slug == "approve-devto-sweep"));
    }
}
