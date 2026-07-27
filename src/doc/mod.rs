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
//! Later tasks in this spec register the generic `BrainDocModel` trait and the
//! concrete `Opportunity` / `LearningArtifact` / `Proposal` models here as sibling
//! sub-modules; this file is the module wiring + re-export surface they attach to.

mod frontmatter_value;
mod parse_nested;

pub use frontmatter_value::{FrontmatterValue, serialize_nested_frontmatter};
pub use parse_nested::{NestedParseError, parse_nested_frontmatter};
