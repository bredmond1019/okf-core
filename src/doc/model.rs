// The generic `BrainDocModel` abstraction — per D53, `okf-core` owns a typed
// frontmatter + sentinel-delimited body specification + slug derivation +
// `index.md` reconcile *intent* (never file IO — that is mev `MV.9.A`'s job).
//
// Everything in this module is deliberately shape-agnostic: nothing here is
// named or typed for `Opportunity` (or any other concrete model). Later tasks
// in this spec implement `BrainDocModel` for three unrelated shapes
// (`Opportunity`, `LearningArtifact`, `Proposal`) to prove the abstraction
// generalizes rather than merely fitting the first (and richest) consumer.

use super::frontmatter_value::{FrontmatterValue, serialize_nested_frontmatter};

/// A generic typed brain document: typed frontmatter, a sentinel-delimited
/// body, a derived slug, and the declarative intent for how it registers into
/// an `index.md`.
///
/// Implementors own their field shape entirely — this trait only asks for the
/// four projections a materializer needs to write and reconcile the document.
pub trait BrainDocModel {
    /// The document's frontmatter fields, in the order they should serialize.
    fn frontmatter(&self) -> Vec<(String, FrontmatterValue)>;

    /// The document's body, as an ordered list of verbatim/generated sections.
    fn body(&self) -> BodySpec;

    /// The document's derived slug (typically via [`super::slug::derive_slug`]
    /// over a title field), used to name the file and/or build `doc_id`.
    fn slug(&self) -> String;

    /// The declarative intent describing how this document registers into an
    /// `index.md` — never executed here; a materializer (mev `MV.9.A`)
    /// consumes this to perform the actual reconcile.
    fn index_intent(&self) -> IndexIntent;

    /// A stable identity string for this document's type (e.g.
    /// `"opportunity"`, `"learning-artifact"`, `"proposal"`), used by a
    /// downstream materializer to dispatch on document kind without matching
    /// on the concrete Rust type.
    fn doc_type(&self) -> &'static str;
}

/// One section of a `BrainDocModel`'s body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodySection {
    /// Free-form prose/markdown, emitted exactly as given.
    Verbatim(String),
    /// A machine-generated section, wrapped in mev-compatible sentinels:
    /// `<!-- BEGIN generated:{marker} -->` / `<!-- END generated:{marker} -->`.
    /// Byte-compatible with mev's `splice_generated` (`src/brain/emit.rs`),
    /// so mev can re-splice a document okf-core wrote.
    Generated { marker: String, content: String },
}

/// An ordered list of body sections, plus the renderer that turns them into
/// the document's full body text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BodySpec {
    pub sections: Vec<BodySection>,
}

impl BodySpec {
    /// Construct a `BodySpec` from an ordered list of sections.
    pub fn new(sections: Vec<BodySection>) -> Self {
        Self { sections }
    }

    /// Render every section into the document's full body text, in order.
    ///
    /// Each section ends with exactly one trailing newline (an already
    /// newline-terminated `Verbatim`/`content` string is not doubled), so
    /// consecutive sections do not run together. `Generated` sections always
    /// wrap `content` in the mev-compatible sentinel pair, even when
    /// `content` is empty.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for section in &self.sections {
            match section {
                BodySection::Verbatim(text) => {
                    out.push_str(text);
                    if !text.is_empty() && !text.ends_with('\n') {
                        out.push('\n');
                    }
                }
                BodySection::Generated { marker, content } => {
                    out.push_str("<!-- BEGIN generated:");
                    out.push_str(marker);
                    out.push_str(" -->\n");
                    if !content.is_empty() {
                        out.push_str(content);
                        if !content.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    out.push_str("<!-- END generated:");
                    out.push_str(marker);
                    out.push_str(" -->\n");
                }
            }
        }
        out
    }
}

/// The declarative intent describing how a document registers into an
/// `index.md` — the file path it belongs under, the relative link target for
/// the doc itself, and the ordered row cells a materializer writes into that
/// index's table. Intent only; no file IO happens here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexIntent {
    /// Path (relative to the brain root) of the `index.md` this document
    /// registers into, e.g. `business/docs/opportunities/index.md`.
    pub index_path: String,
    /// The relative link target for this document from that index, e.g.
    /// `anthropic.md`.
    pub link_target: String,
    /// The ordered cells of this document's row in the index's table.
    pub row_cells: Vec<String>,
}

impl IndexIntent {
    pub fn new(
        index_path: impl Into<String>,
        link_target: impl Into<String>,
        row_cells: Vec<String>,
    ) -> Self {
        Self {
            index_path: index_path.into(),
            link_target: link_target.into(),
            row_cells,
        }
    }
}

/// Render a full document — serialized nested frontmatter followed by the
/// rendered body — for any `BrainDocModel`. This is the single deterministic
/// write path every concrete model shares; no model re-implements it.
pub fn render_document(model: &impl BrainDocModel) -> String {
    let mut out = serialize_nested_frontmatter(&model.frontmatter());
    out.push_str(&model.body().render());
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal in-test model — deliberately not opportunity-shaped — used
    /// only to exercise `render_document` and the trait's shape-agnosticism.
    struct Minimal {
        title: String,
    }

    impl BrainDocModel for Minimal {
        fn frontmatter(&self) -> Vec<(String, FrontmatterValue)> {
            vec![
                (
                    "type".to_string(),
                    FrontmatterValue::Scalar("Minimal".to_string()),
                ),
                (
                    "title".to_string(),
                    FrontmatterValue::Scalar(self.title.clone()),
                ),
            ]
        }

        fn body(&self) -> BodySpec {
            BodySpec::new(vec![
                BodySection::Verbatim(format!("# {}", self.title)),
                BodySection::Generated {
                    marker: "summary".to_string(),
                    content: "Auto-generated summary.".to_string(),
                },
            ])
        }

        fn slug(&self) -> String {
            super::super::slug::derive_slug(&self.title)
        }

        fn index_intent(&self) -> IndexIntent {
            IndexIntent::new(
                "index.md",
                format!("{}.md", self.slug()),
                vec![self.title.clone()],
            )
        }

        fn doc_type(&self) -> &'static str {
            "minimal"
        }
    }

    // ── BodySpec::render ─────────────────────────────────────────────────────

    #[test]
    fn generated_section_emits_mev_compatible_sentinels() {
        let spec = BodySpec::new(vec![BodySection::Generated {
            marker: "wave-table".to_string(),
            content: "| a | b |".to_string(),
        }]);
        assert_eq!(
            spec.render(),
            "<!-- BEGIN generated:wave-table -->\n| a | b |\n<!-- END generated:wave-table -->\n"
        );
    }

    #[test]
    fn generated_section_with_empty_content_still_wraps_sentinels() {
        let spec = BodySpec::new(vec![BodySection::Generated {
            marker: "empty".to_string(),
            content: String::new(),
        }]);
        assert_eq!(
            spec.render(),
            "<!-- BEGIN generated:empty -->\n<!-- END generated:empty -->\n"
        );
    }

    #[test]
    fn verbatim_section_gets_exactly_one_trailing_newline() {
        let spec = BodySpec::new(vec![BodySection::Verbatim("no newline yet".to_string())]);
        assert_eq!(spec.render(), "no newline yet\n");

        let spec_already_terminated =
            BodySpec::new(vec![BodySection::Verbatim("has one\n".to_string())]);
        assert_eq!(spec_already_terminated.render(), "has one\n");
    }

    #[test]
    fn render_is_stable_across_repeated_calls() {
        let spec = BodySpec::new(vec![
            BodySection::Verbatim("Intro.".to_string()),
            BodySection::Generated {
                marker: "brief".to_string(),
                content: "```json\n{}\n```".to_string(),
            },
        ]);
        assert_eq!(spec.render(), spec.render());
    }

    #[test]
    fn multiple_sections_render_in_order() {
        let spec = BodySpec::new(vec![
            BodySection::Verbatim("First.".to_string()),
            BodySection::Verbatim("Second.".to_string()),
        ]);
        assert_eq!(spec.render(), "First.\nSecond.\n");
    }

    // ── render_document ──────────────────────────────────────────────────────

    #[test]
    fn render_document_has_exactly_one_frontmatter_fence_pair() {
        let model = Minimal {
            title: "Test Doc".to_string(),
        };
        let rendered = render_document(&model);

        let fence_count = rendered.matches("---\n").count();
        assert_eq!(fence_count, 2, "expected exactly one `---` fence pair");
        assert!(rendered.starts_with("---\n"));
    }

    #[test]
    fn render_document_is_frontmatter_then_body() {
        let model = Minimal {
            title: "Test Doc".to_string(),
        };
        let rendered = render_document(&model);
        let expected = "---\ntype: Minimal\ntitle: Test Doc\n---\n# Test Doc\n<!-- BEGIN generated:summary -->\nAuto-generated summary.\n<!-- END generated:summary -->\n";
        assert_eq!(rendered, expected);
    }

    #[test]
    fn minimal_model_slug_and_index_intent_are_shape_agnostic() {
        let model = Minimal {
            title: "Hello World".to_string(),
        };
        assert_eq!(model.slug(), "hello-world");
        assert_eq!(model.doc_type(), "minimal");
        let intent = model.index_intent();
        assert_eq!(intent.index_path, "index.md");
        assert_eq!(intent.link_target, "hello-world.md");
        assert_eq!(intent.row_cells, vec!["Hello World".to_string()]);
    }
}
