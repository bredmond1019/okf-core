//! The typed brain-**document** layer, above the frontmatter-only model in
//! `crate::frontmatter` / `crate::parse`.
//!
//! This module is the home for:
//! - `frontmatter_value`: a `FrontmatterValue` model + `serialize_nested_frontmatter`,
//!   extending the flat scalar/inline-list model with block lists and inline-map
//!   lists (`contacts:` / `actions:`), so nested brain-doc frontmatter can round-trip.
//! - `parse_nested`: `parse_nested_frontmatter`, the read half of the round-trip —
//!   recovers all four `FrontmatterValue` shapes from source text, which the flat
//!   `crate::parse` parser cannot represent.
//!
//! - `model`: the generic `BrainDocModel` trait, `BodySpec`/`BodySection`
//!   (sentinel-delimited body rendering, byte-compatible with mev's
//!   `splice_generated`), `IndexIntent`, and `render_document` — the single
//!   deterministic write path every concrete model shares.
//! - `slug`: `derive_slug`, the pure title -> slug function every model uses
//!   for its `BrainDocModel::slug()`.
//! - `opportunity`: `Opportunity` — the full model for the
//!   `business/docs/opportunities/index.md` contract, plus `Contact`/`Action`
//!   and the `CompanyBrief`/`ProspectingResult` constructors.
//! - `learning_artifact`: `LearningArtifact` — a sketched model over the
//!   engine-rs content-pipeline payload (`{artifact_id, channel_type,
//!   source_ref, summary, digest_markdown, entities, language}`).
//! - `proposal`: `Proposal` — a sketched model over the engine-rs
//!   `AutomationRoadmap` deliverable (`{situation, candidates, top_profiles,
//!   recommendation}`), carried as a JSON body section.
//!
//! `LearningArtifact` and `Proposal` are the two additional shapes that prove
//! the `BrainDocModel` abstraction generalizes beyond `Opportunity` — the
//! whole point of `OK.2.A`.

mod frontmatter_value;
mod learning_artifact;
mod model;
mod opportunity;
mod parse_nested;
mod proposal;
mod slug;

pub use frontmatter_value::{FrontmatterValue, serialize_nested_frontmatter};
pub use learning_artifact::{LearningArtifact, LearningArtifactError};
pub use model::{BodySection, BodySpec, BrainDocModel, IndexIntent, render_document};
pub use opportunity::{Action, Contact, Opportunity, OpportunityError};
pub use parse_nested::{NestedParseError, parse_nested_frontmatter};
pub use proposal::{Proposal, ProposalError};
pub use slug::derive_slug;
