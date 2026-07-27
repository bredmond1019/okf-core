// `Proposal` — a sketched `BrainDocModel` over the engine-rs
// `PROPOSAL_GENERATOR` deliverable, `AutomationRoadmap`
// `{situation, candidates, top_profiles, recommendation}` (see
// `engine-rs/crates/engine-core/src/workflows/proposal_generator/schema.rs`).
// This is one of the two "sketch" models (`LearningArtifact` is the other)
// task `OK.2.A`/task 5 adds to prove the `BrainDocModel` abstraction
// generalizes beyond `Opportunity`. Sketched means it compiles, implements
// the trait, and round-trips a fixture — the roadmap payload is carried
// verbatim as a `serde_json::Value` body section rather than mirroring
// `RankedCandidate`/`WorkflowProfile`/`FirstEngagement` in full.

use serde_json::Value as JsonValue;

use super::frontmatter_value::FrontmatterValue;
use super::model::{BodySection, BodySpec, BrainDocModel, IndexIntent};
use super::slug::derive_slug;

/// The `index.md` this model's [`BrainDocModel::index_intent`] registers
/// into. Sketch-level: mirrors `business/docs/opportunities/index.md`'s
/// sibling-directory convention for the proposals corpus.
const PROPOSALS_INDEX: &str = "business/docs/proposals/index.md";

/// The sentinel marker the roadmap body section renders under.
const ROADMAP_MARKER: &str = "roadmap";

/// An error recovered while reconstructing a [`Proposal`] from parsed
/// frontmatter fields.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProposalError {
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
}

/// A generated automation-roadmap proposal for a prospective client. The
/// document-level identity (`company_name`, `title`) is typed frontmatter;
/// the roadmap's four sections (`situation`, `candidates`, `top_profiles`,
/// `recommendation`) are carried verbatim as JSON in the body, not mirrored
/// as nested Rust structs — see the module comment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Proposal {
    pub company_name: String,
    pub title: String,
    /// The raw `AutomationRoadmap` JSON, embedded verbatim in the body.
    /// `Value::Null` (the default) renders an empty roadmap section.
    pub roadmap: JsonValue,
}

impl Proposal {
    /// Build a `Proposal` from a company name and its raw `AutomationRoadmap`
    /// JSON value, consumed as given — nothing in the roadmap shape is
    /// re-derived or reshaped.
    pub fn from_automation_roadmap(company_name: &str, roadmap: &JsonValue) -> Self {
        Self {
            company_name: company_name.to_string(),
            title: format!("{company_name} — Automation Roadmap"),
            roadmap: roadmap.clone(),
        }
    }

    /// Reconstruct a `Proposal`'s frontmatter-borne fields from a parsed
    /// nested-frontmatter field list. `roadmap` lives in the body, not the
    /// frontmatter, so it is left at its default (`Value::Null`) here.
    pub fn from_frontmatter(fields: &[(String, FrontmatterValue)]) -> Result<Self, ProposalError> {
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        let scalar = |key: &str| match get(key) {
            Some(FrontmatterValue::Scalar(s)) => Some(s.clone()),
            _ => None,
        };

        let title = scalar("title").ok_or(ProposalError::MissingField("title"))?;
        let company_name = scalar("company_name").unwrap_or_default();

        Ok(Self {
            company_name,
            title,
            roadmap: JsonValue::Null,
        })
    }
}

impl BrainDocModel for Proposal {
    fn frontmatter(&self) -> Vec<(String, FrontmatterValue)> {
        vec![
            (
                "type".to_string(),
                FrontmatterValue::Scalar("Proposal".to_string()),
            ),
            (
                "title".to_string(),
                FrontmatterValue::Scalar(self.title.clone()),
            ),
            (
                "company_name".to_string(),
                FrontmatterValue::Scalar(self.company_name.clone()),
            ),
        ]
    }

    fn body(&self) -> BodySpec {
        let content = if self.roadmap.is_null() {
            String::new()
        } else {
            let json =
                serde_json::to_string_pretty(&self.roadmap).unwrap_or_else(|_| "{}".to_string());
            format!("```json\n{json}\n```")
        };
        BodySpec::new(vec![BodySection::Generated {
            marker: ROADMAP_MARKER.to_string(),
            content,
        }])
    }

    fn slug(&self) -> String {
        derive_slug(&self.title)
    }

    fn index_intent(&self) -> IndexIntent {
        IndexIntent::new(
            PROPOSALS_INDEX,
            format!("{}.md", self.slug()),
            vec![self.company_name.clone(), self.title.clone()],
        )
    }

    fn doc_type(&self) -> &'static str {
        "proposal"
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse_nested::parse_nested_frontmatter;
    use crate::doc::render_document;

    fn full() -> Proposal {
        Proposal {
            company_name: "Loja da Ana".to_string(),
            title: "Loja da Ana — Automation Roadmap".to_string(),
            roadmap: serde_json::json!({
                "situation": {"company_name": "Loja da Ana"},
                "candidates": [],
                "top_profiles": [],
                "recommendation": null,
            }),
        }
    }

    #[test]
    fn renders_a_well_formed_document() {
        let proposal = full();
        let rendered = render_document(&proposal);

        assert_eq!(rendered.matches("---\n").count(), 2);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("<!-- BEGIN generated:roadmap -->"));
        assert!(rendered.contains("<!-- END generated:roadmap -->"));
        assert!(rendered.contains("\"company_name\": \"Loja da Ana\""));
    }

    #[test]
    fn render_parse_render_is_idempotent() {
        let proposal = full();
        let rendered_once = render_document(&proposal);

        let fields = parse_nested_frontmatter(&rendered_once).expect("must parse");
        let recovered = Proposal::from_frontmatter(&fields).expect("must reconstruct");
        // `roadmap` lives in the body, not the frontmatter — restore it from
        // the original before re-rendering, mirroring how a real
        // materializer would read the body's `roadmap` sentinel separately.
        let mut recovered = recovered;
        recovered.roadmap = proposal.roadmap.clone();

        let rendered_twice = render_document(&recovered);
        assert_eq!(rendered_once, rendered_twice);
    }

    #[test]
    fn from_automation_roadmap_sets_company_name_title_and_roadmap() {
        let roadmap = serde_json::json!({
            "situation": {"company_name": "Acme Corp"},
            "candidates": [],
            "top_profiles": [],
            "recommendation": null,
        });
        let proposal = Proposal::from_automation_roadmap("Acme Corp", &roadmap);
        assert_eq!(proposal.company_name, "Acme Corp");
        assert_eq!(proposal.title, "Acme Corp — Automation Roadmap");
        assert_eq!(proposal.roadmap, roadmap);
    }

    #[test]
    fn null_roadmap_renders_empty_generated_section() {
        let proposal = Proposal {
            company_name: "Test Co".to_string(),
            title: "Test Co — Automation Roadmap".to_string(),
            roadmap: JsonValue::Null,
        };
        let rendered = render_document(&proposal);
        assert!(
            rendered.contains("<!-- BEGIN generated:roadmap -->\n<!-- END generated:roadmap -->")
        );
    }
}
