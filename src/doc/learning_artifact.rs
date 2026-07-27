// `LearningArtifact` — a sketched `BrainDocModel` over the engine-rs
// content-pipeline payload `{artifact_id, channel_type, source_ref, summary,
// digest_markdown, entities, language}` (see
// `engine-rs/crates/engine-core/src/workflows/content_pipeline/persist_to_brain.rs`,
// `PersistToBrainNode`'s POST body). This is one of the two "sketch" models
// (`Proposal` is the other) task `OK.2.A`/task 5 adds to prove the
// `BrainDocModel` abstraction generalizes beyond `Opportunity` — sketched
// means it compiles, implements the trait, and round-trips a fixture; it is
// not a field-by-field contract the way `Opportunity` is.

use super::frontmatter_value::FrontmatterValue;
use super::model::{BodySection, BodySpec, BrainDocModel, IndexIntent};
use super::slug::derive_slug;

/// The `index.md` this model's [`BrainDocModel::index_intent`] registers
/// into. Sketch-level: the learning corpus index path is not yet a landed
/// contract, so this is a reasonable placeholder a real materializer task
/// can repoint without changing this model's shape.
const LEARNING_CORPUS_INDEX: &str = "docs/content/learning-corpus/index.md";

/// The sentinel marker the digest body section renders under.
const DIGEST_MARKER: &str = "digest";

/// An error recovered while reconstructing a [`LearningArtifact`] from
/// parsed frontmatter fields.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LearningArtifactError {
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
}

/// A digested piece of external content, persisted to the brain by
/// `PersistToBrainNode` (`EN.5.A`). Mirrors the POST payload's field set
/// exactly; `digest_markdown` is carried in the body (as a generated
/// sentinel section) rather than the frontmatter, since it is prose, not a
/// scalar/list value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LearningArtifact {
    pub artifact_id: String,
    pub channel_type: String,
    pub source_ref: String,
    pub summary: String,
    pub entities: Vec<String>,
    pub language: String,
    pub digest_markdown: String,
}

impl LearningArtifact {
    /// Build a `LearningArtifact` from the engine-rs content-pipeline POST
    /// payload JSON (`{artifact_id, channel_type, source_ref, summary,
    /// digest_markdown, entities, language}`). Consumed as given — string
    /// fields default to empty and `entities` to an empty list when absent,
    /// rather than erroring, since this is a sketch model.
    pub fn from_payload(payload: &serde_json::Value) -> Self {
        let str_field = |key: &str| {
            payload
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let entities = payload
            .get("entities")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            artifact_id: str_field("artifact_id"),
            channel_type: str_field("channel_type"),
            source_ref: str_field("source_ref"),
            summary: str_field("summary"),
            entities,
            language: str_field("language"),
            digest_markdown: str_field("digest_markdown"),
        }
    }

    /// Reconstruct a `LearningArtifact`'s frontmatter-borne fields from a
    /// parsed nested-frontmatter field list. `digest_markdown` lives in the
    /// body, not the frontmatter, so it is left at its default (empty
    /// string) here — callers that need it read it from the parsed body's
    /// `digest` sentinel section separately.
    pub fn from_frontmatter(
        fields: &[(String, FrontmatterValue)],
    ) -> Result<Self, LearningArtifactError> {
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        let scalar = |key: &str| match get(key) {
            Some(FrontmatterValue::Scalar(s)) => Some(s.clone()),
            _ => None,
        };

        let artifact_id =
            scalar("artifact_id").ok_or(LearningArtifactError::MissingField("artifact_id"))?;
        let channel_type = scalar("channel_type").unwrap_or_default();
        let source_ref = scalar("source_ref").unwrap_or_default();
        let summary = scalar("summary").unwrap_or_default();
        let language = scalar("language").unwrap_or_default();
        let entities = match get("entities") {
            Some(FrontmatterValue::InlineList(items)) => items.clone(),
            _ => Vec::new(),
        };

        Ok(Self {
            artifact_id,
            channel_type,
            source_ref,
            summary,
            entities,
            language,
            digest_markdown: String::new(),
        })
    }
}

impl BrainDocModel for LearningArtifact {
    fn frontmatter(&self) -> Vec<(String, FrontmatterValue)> {
        vec![
            (
                "type".to_string(),
                FrontmatterValue::Scalar("LearningArtifact".to_string()),
            ),
            (
                "artifact_id".to_string(),
                FrontmatterValue::Scalar(self.artifact_id.clone()),
            ),
            (
                "channel_type".to_string(),
                FrontmatterValue::Scalar(self.channel_type.clone()),
            ),
            (
                "source_ref".to_string(),
                FrontmatterValue::Scalar(self.source_ref.clone()),
            ),
            (
                "summary".to_string(),
                FrontmatterValue::Scalar(self.summary.clone()),
            ),
            (
                "entities".to_string(),
                FrontmatterValue::InlineList(self.entities.clone()),
            ),
            (
                "language".to_string(),
                FrontmatterValue::Scalar(self.language.clone()),
            ),
        ]
    }

    fn body(&self) -> BodySpec {
        BodySpec::new(vec![BodySection::Generated {
            marker: DIGEST_MARKER.to_string(),
            content: self.digest_markdown.clone(),
        }])
    }

    fn slug(&self) -> String {
        derive_slug(&self.artifact_id)
    }

    fn index_intent(&self) -> IndexIntent {
        IndexIntent::new(
            LEARNING_CORPUS_INDEX,
            format!("{}.md", self.slug()),
            vec![
                self.artifact_id.clone(),
                self.channel_type.clone(),
                self.language.clone(),
            ],
        )
    }

    fn doc_type(&self) -> &'static str {
        "learning-artifact"
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse_nested::parse_nested_frontmatter;
    use crate::doc::render_document;

    fn full() -> LearningArtifact {
        LearningArtifact {
            artifact_id: "artifact-1".to_string(),
            channel_type: "web_article".to_string(),
            source_ref: "https://example.com/a".to_string(),
            summary: "A concise summary.".to_string(),
            entities: vec!["Acme Corp".to_string()],
            language: "en".to_string(),
            digest_markdown: "# Digest\n\nA concise summary.".to_string(),
        }
    }

    #[test]
    fn renders_a_well_formed_document() {
        let artifact = full();
        let rendered = render_document(&artifact);

        assert_eq!(rendered.matches("---\n").count(), 2);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("<!-- BEGIN generated:digest -->"));
        assert!(rendered.contains("<!-- END generated:digest -->"));
    }

    #[test]
    fn render_parse_render_is_idempotent() {
        let artifact = full();
        let rendered_once = render_document(&artifact);

        let fields = parse_nested_frontmatter(&rendered_once).expect("must parse");
        let recovered = LearningArtifact::from_frontmatter(&fields).expect("must reconstruct");
        // `digest_markdown` lives in the body, not the frontmatter — restore
        // it from the original before re-rendering, mirroring how a real
        // materializer would read the body's `digest` sentinel separately.
        let mut recovered = recovered;
        recovered.digest_markdown = artifact.digest_markdown.clone();

        let rendered_twice = render_document(&recovered);
        assert_eq!(rendered_once, rendered_twice);
    }

    #[test]
    fn from_payload_maps_engine_rs_content_pipeline_shape() {
        let payload = serde_json::json!({
            "artifact_id": "artifact-1",
            "channel_type": "web_article",
            "source_ref": "https://example.com/a",
            "summary": "A concise summary.",
            "digest_markdown": "# Digest\n\nA concise summary.",
            "entities": ["Acme Corp"],
            "language": "en",
        });
        let artifact = LearningArtifact::from_payload(&payload);
        assert_eq!(artifact, full());
    }

    #[test]
    fn empty_entities_renders_as_empty_inline_list() {
        let artifact = LearningArtifact {
            entities: vec![],
            ..full()
        };
        let fields = artifact.frontmatter();
        let entities = fields
            .iter()
            .find(|(k, _)| k == "entities")
            .map(|(_, v)| v.clone());
        assert_eq!(entities, Some(FrontmatterValue::InlineList(vec![])));
    }

    #[test]
    fn slug_derives_from_artifact_id() {
        let artifact = LearningArtifact {
            artifact_id: "Artifact One!".to_string(),
            ..full()
        };
        assert_eq!(artifact.slug(), "artifact-one");
    }
}
