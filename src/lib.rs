//! `okf-core` — the single-source OKF frontmatter contract.
//!
//! This crate owns the OKF (Open Knowledge Format) frontmatter model, its YAML
//! serializer, and its parser. Consumers (e.g. `bastion`) depend on this crate
//! rather than maintaining their own copies of the model/serializer/parser, so
//! the frontmatter contract has exactly one source of truth across the
//! workspace.

mod coord;
mod doc;
mod frontmatter;
mod graph;
mod graph_emit;
mod parse;
mod state;

pub use coord::{
    COORD_STALE_TTL_SECONDS, Coord, Disposal, DisposalBreadth, DisposalFile, DisposalRoute,
    DisposalRow, DrainLog, DrainLogEntry, DurableHomeChannel, Escalation, EscalationDurableHome,
    EscalationRecord, HeartbeatRecord, HeartbeatValue, Lease, LeaseKind, LeaseRecord, LeaseScope,
    Message, MessageDurableHome, MessageKind, MessageRecord, MessageSender, MessageSubject,
    PidSource, Registry, RegistryClaim, Slot, SlotRecord, SweepDiscovery, SweepLane,
    SweepRunRecord, SweepRunRecordEntry, SweepSnapshot, SweepSnapshotRecord,
};
pub use doc::{
    Action, BodySection, BodySpec, BrainDocModel, Contact, FrontmatterValue, IndexIntent,
    LearningArtifact, LearningArtifactError, NestedParseError, Opportunity, OpportunityError,
    Proposal, ProposalError, derive_slug, parse_nested_frontmatter, render_document,
    serialize_nested_frontmatter,
};
pub use frontmatter::{OkfFrontmatter, serialize_frontmatter};
pub use graph::{Edge, EdgeKind, EdgeResolution, Graph, GraphArtifact, Node, resolve_edge};
pub use graph_emit::{ExportedEdge, GraphExport, build_graph_export};
pub use parse::{Frontmatter, ParseResult, extract_frontmatter, parse_frontmatter};
pub use state::{
    AmendsRef, ApprovalDep, Backlog, BacklogOrigin, Block, BlockDep, BlockedBy, Carryover,
    CarryoverArchiveRow, CarryoverKind, CarryoverNeeds, CarryoverScope, ClearsWhen,
    ClearsWhenPredicate, CrossRepoEdge, DisposalReason, Endpoint, Epic, ExternalDep,
    FleetCorrectness, Focus, KnownCarryoverKind, KnownCarryoverNeeds, KnownFleetCorrectness,
    OperatorDep, Origin, Reference, RepoRollup, StateEdge, StateEdgeKind, StateFile, StateGraph,
    StateLoadError, StateNode, StateSource, TierEntry, Track, TrackBlock, W_STATE_OP_SLUG_STUTTER,
    build_state_graph, load_state, normalize_op_slug, op_id, op_slug_stutters,
};
