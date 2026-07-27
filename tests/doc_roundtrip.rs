// Fixture-based round-trip integration test suite for the `okf-core` brain
// document layer (`OK.2.A` task 6).
//
// Each fixture is loaded via `include_str!` — compile-time inclusion, so this
// suite adds no I/O dependency and no dev-dependency (AGENT.md rule 3). Per
// fixture we assert the block's eval slice:
//   (a) parse succeeds and the typed model recovers every contract field;
//   (b) parse -> serialize -> parse yields an equal model;
//   (c) serialize(parse(serialize(m))) is byte-identical to serialize(m)
//       (idempotence — the property mev's re-splice depends on);
//   (d) emitted body sentinels match mev's `<!-- BEGIN generated:{marker} -->`
//       form exactly.
//
// A final guard section asserts the pre-existing flat surfaces
// (`serialize_frontmatter` / `extract_frontmatter`) are unchanged by this
// block's additions.
//
// No `src/` file is touched by this suite — it is verification-only, layered
// over tasks 1-5.

use okf_core::{
    BrainDocModel, FrontmatterValue, LearningArtifact, OkfFrontmatter, Opportunity, ParseResult,
    Proposal, extract_frontmatter, parse_nested_frontmatter, serialize_frontmatter,
    serialize_nested_frontmatter,
};

const OPPORTUNITY_FIXTURE: &str = include_str!("fixtures/opportunity-anthropic.md");
const LEARNING_ARTIFACT_FIXTURE: &str = include_str!("fixtures/learning-artifact.md");
const PROPOSAL_FIXTURE: &str = include_str!("fixtures/proposal.md");

/// Assert that every `BodySection::Generated` marker in `model`'s body
/// renders under mev's exact sentinel form, `<!-- BEGIN generated:{marker}
/// -->` / `<!-- END generated:{marker} -->` (mev `src/brain/emit.rs`,
/// `splice_generated`).
fn assert_sentinels_mev_compatible(rendered_body: &str, markers: &[&str]) {
    for marker in markers {
        let begin = format!("<!-- BEGIN generated:{marker} -->");
        let end = format!("<!-- END generated:{marker} -->");
        assert!(
            rendered_body.contains(&begin),
            "expected body to contain {begin:?}, got:\n{rendered_body}"
        );
        assert!(
            rendered_body.contains(&end),
            "expected body to contain {end:?}, got:\n{rendered_body}"
        );
        // The BEGIN sentinel must precede the END sentinel for the same marker.
        assert!(
            rendered_body.find(&begin).unwrap() < rendered_body.find(&end).unwrap(),
            "BEGIN sentinel must precede END sentinel for marker {marker:?}"
        );
    }
}

// ── Opportunity — full contract fixture ─────────────────────────────────────

mod opportunity_fixture {
    use super::*;

    fn parse_fixture() -> Opportunity {
        let fields = parse_nested_frontmatter(OPPORTUNITY_FIXTURE).expect("fixture must parse");
        Opportunity::from_frontmatter(&fields).expect("model must reconstruct")
    }

    #[test]
    fn a_recovers_every_contract_field() {
        let o = parse_fixture();
        assert_eq!(o.title, "Anthropic");
        assert_eq!(
            o.description,
            "RESEARCH_AGENT company brief — frontier AI lab in hypergrowth; \
             internal-knowledge / onboarding-automation angle."
        );
        assert_eq!(o.doc_id.as_deref(), Some("opportunity-anthropic"));
        assert_eq!(o.layer, vec!["business".to_string()]);
        assert_eq!(o.status.as_deref(), Some("active"));
        assert_eq!(o.kind.as_deref(), Some("company"));
        assert_eq!(o.stage.as_deref(), Some("identified"));
        assert_eq!(
            o.source.as_deref(),
            Some("RESEARCH_AGENT test run (company mode)")
        );
        assert_eq!(o.url.as_deref(), Some("https://www.anthropic.com"));
        assert_eq!(o.links, vec!["https://www.anthropic.com".to_string()]);
        assert_eq!(o.research_ref.as_deref(), Some("engine-rs-research-runs"));
        assert!(o.contacts.is_empty());
        assert_eq!(o.actions.len(), 1);
        assert_eq!(o.actions[0].at, "2026-07-25");
        assert_eq!(o.actions[0].kind, "research");
        assert_eq!(
            o.actions[0].note,
            "Generated CompanyBrief via RESEARCH_AGENT (company mode)"
        );
    }

    #[test]
    fn b_parse_serialize_parse_yields_equal_model() {
        let m1 = parse_fixture();
        let s1 = serialize_nested_frontmatter(&m1.frontmatter());
        let fields2 = parse_nested_frontmatter(&s1).expect("re-serialized block must parse");
        let m2 = Opportunity::from_frontmatter(&fields2).expect("model must reconstruct");
        assert_eq!(m1, m2);
    }

    #[test]
    fn c_serialize_parse_serialize_is_idempotent() {
        let m1 = parse_fixture();
        let s1 = serialize_nested_frontmatter(&m1.frontmatter());
        let fields2 = parse_nested_frontmatter(&s1).expect("re-serialized block must parse");
        let m2 = Opportunity::from_frontmatter(&fields2).expect("model must reconstruct");
        let s2 = serialize_nested_frontmatter(&m2.frontmatter());
        assert_eq!(
            s1, s2,
            "serialize(parse(serialize(m))) must equal serialize(m)"
        );
    }

    #[test]
    fn d_body_has_no_stray_generated_markers() {
        // Opportunity's body is prose + a fenced research-brief JSON block —
        // it never emits mev generated sentinels, so none should appear.
        let m = parse_fixture();
        let rendered = m.body().render();
        assert!(!rendered.contains("<!-- BEGIN generated:"));
        assert!(!rendered.contains("<!-- END generated:"));
    }

    #[test]
    fn fixture_body_carries_research_brief_as_first_json_fence() {
        let json_fence_pos = OPPORTUNITY_FIXTURE
            .find("```json")
            .expect("fixture must contain a fenced json block");
        let heading_pos = OPPORTUNITY_FIXTURE
            .find("## Research Brief")
            .expect("fixture must contain the Research Brief heading");
        assert!(heading_pos < json_fence_pos);
    }
}

// ── LearningArtifact — sketch fixture ───────────────────────────────────────

mod learning_artifact_fixture {
    use super::*;

    fn parse_fixture() -> LearningArtifact {
        let fields =
            parse_nested_frontmatter(LEARNING_ARTIFACT_FIXTURE).expect("fixture must parse");
        LearningArtifact::from_frontmatter(&fields).expect("model must reconstruct")
    }

    /// The fixture's `digest_markdown` body content, recovered by hand (a
    /// real materializer reads this from the parsed body's `digest`
    /// sentinel section separately — see `LearningArtifact::from_frontmatter`'s
    /// doc comment).
    const DIGEST_MARKDOWN: &str = "# Digest\n\nA concise summary of the fixture article, expanded into digest form for testing.";

    #[test]
    fn a_recovers_every_contract_field() {
        let a = parse_fixture();
        assert_eq!(a.artifact_id, "fixture-artifact-1");
        assert_eq!(a.channel_type, "web_article");
        assert_eq!(a.source_ref, "https://example.com/articles/fixture");
        assert_eq!(a.summary, "A concise summary of the fixture article.");
        assert_eq!(
            a.entities,
            vec!["Acme Corp".to_string(), "Widget Co".to_string()]
        );
        assert_eq!(a.language, "en");
    }

    #[test]
    fn b_parse_serialize_parse_yields_equal_model() {
        let m1 = parse_fixture();
        let s1 = serialize_nested_frontmatter(&m1.frontmatter());
        let fields2 = parse_nested_frontmatter(&s1).expect("re-serialized block must parse");
        let m2 = LearningArtifact::from_frontmatter(&fields2).expect("model must reconstruct");
        assert_eq!(m1, m2);
    }

    #[test]
    fn c_serialize_parse_serialize_is_idempotent() {
        let m1 = parse_fixture();
        let s1 = serialize_nested_frontmatter(&m1.frontmatter());
        let fields2 = parse_nested_frontmatter(&s1).expect("re-serialized block must parse");
        let m2 = LearningArtifact::from_frontmatter(&fields2).expect("model must reconstruct");
        let s2 = serialize_nested_frontmatter(&m2.frontmatter());
        assert_eq!(
            s1, s2,
            "serialize(parse(serialize(m))) must equal serialize(m)"
        );
    }

    #[test]
    fn d_body_sentinels_match_mev_form() {
        let mut m = parse_fixture();
        m.digest_markdown = DIGEST_MARKDOWN.to_string();
        let rendered_body = m.body().render();
        assert_sentinels_mev_compatible(&rendered_body, &["digest"]);
        assert!(rendered_body.contains(DIGEST_MARKDOWN));
    }
}

// ── Proposal — sketch fixture ────────────────────────────────────────────────

mod proposal_fixture {
    use super::*;

    fn parse_fixture() -> Proposal {
        let fields = parse_nested_frontmatter(PROPOSAL_FIXTURE).expect("fixture must parse");
        Proposal::from_frontmatter(&fields).expect("model must reconstruct")
    }

    #[test]
    fn a_recovers_every_contract_field() {
        let p = parse_fixture();
        assert_eq!(p.title, "Acme Corp — Automation Roadmap");
        assert_eq!(p.company_name, "Acme Corp");
    }

    #[test]
    fn b_parse_serialize_parse_yields_equal_model() {
        let m1 = parse_fixture();
        let s1 = serialize_nested_frontmatter(&m1.frontmatter());
        let fields2 = parse_nested_frontmatter(&s1).expect("re-serialized block must parse");
        let m2 = Proposal::from_frontmatter(&fields2).expect("model must reconstruct");
        assert_eq!(m1, m2);
    }

    #[test]
    fn c_serialize_parse_serialize_is_idempotent() {
        let m1 = parse_fixture();
        let s1 = serialize_nested_frontmatter(&m1.frontmatter());
        let fields2 = parse_nested_frontmatter(&s1).expect("re-serialized block must parse");
        let m2 = Proposal::from_frontmatter(&fields2).expect("model must reconstruct");
        let s2 = serialize_nested_frontmatter(&m2.frontmatter());
        assert_eq!(
            s1, s2,
            "serialize(parse(serialize(m))) must equal serialize(m)"
        );
    }

    #[test]
    fn d_body_sentinels_match_mev_form() {
        let mut m = parse_fixture();
        m.roadmap = serde_json::json!({"situation": {"company_name": "Acme Corp"}});
        let rendered_body = m.body().render();
        assert_sentinels_mev_compatible(&rendered_body, &["roadmap"]);
    }
}

// ── Guard: pre-existing flat surfaces are unchanged ─────────────────────────

mod flat_surface_guard {
    use super::*;

    fn full_flat() -> OkfFrontmatter {
        OkfFrontmatter {
            type_: Some("Guideline".into()),
            title: Some("My Title".into()),
            description: Some("A one-line summary.".into()),
            doc_id: Some("my-title".into()),
            layer: vec!["brain".into(), "console".into()],
            project: Some("bastion".into()),
            status: Some("active".into()),
            keywords: vec!["okf".into(), "frontmatter".into(), "scaffold".into()],
            related: vec!["okf-core".into()],
            synced_from: None,
        }
    }

    #[test]
    fn serialize_frontmatter_exact_output_unchanged() {
        let out = serialize_frontmatter(&full_flat());
        let expected = "\
---
type: Guideline
title: My Title
description: A one-line summary.
doc_id: my-title
layer: [brain, console]
project: bastion
status: active
keywords: [okf, frontmatter, scaffold]
related: [okf-core]
---
";
        assert_eq!(out, expected);
    }

    #[test]
    fn extract_frontmatter_ok_variant_unchanged() {
        let content = "---\ntype: Doc\ntitle: Hello\ndescription: A test.\n---\n# Body\n";
        match extract_frontmatter(content) {
            ParseResult::Ok(fm) => {
                assert_eq!(fm.open_line, 1);
                assert_eq!(fm.close_line, 5);
                assert_eq!(fm.fields["type"].0, "Doc");
                assert_eq!(fm.fields["title"].0, "Hello");
                assert_eq!(fm.fields["description"].0, "A test.");
            }
            other => panic!("expected ParseResult::Ok, got {other:?}"),
        }
    }

    #[test]
    fn extract_frontmatter_unterminated_fence_variant_unchanged() {
        let content = "---\ntype: Doc\ntitle: Hello\n";
        assert_eq!(
            extract_frontmatter(content),
            ParseResult::UnterminatedFence { open_line: 1 }
        );
    }

    #[test]
    fn extract_frontmatter_malformed_line_variant_unchanged() {
        let content = "---\ntype: Doc\nthis is not kv\n---\n";
        assert_eq!(
            extract_frontmatter(content),
            ParseResult::MalformedLine { source_line: 3 }
        );
    }

    #[test]
    fn extract_frontmatter_no_frontmatter_variant_unchanged() {
        let content = "# Just a heading\n\nNo frontmatter here.\n";
        assert_eq!(extract_frontmatter(content), ParseResult::NoFrontmatter);
    }

    #[test]
    fn nested_serializer_still_matches_flat_serializer_byte_for_byte() {
        // Sanity check that this suite's addition of nested fixtures did not
        // regress the flat/nested parity `frontmatter_value` already asserts
        // in-module — a flat field set serializes identically either way.
        let flat_out = serialize_frontmatter(&full_flat());
        let nested_out = serialize_nested_frontmatter(&[
            (
                "type".to_string(),
                FrontmatterValue::Scalar("Guideline".into()),
            ),
            (
                "title".to_string(),
                FrontmatterValue::Scalar("My Title".into()),
            ),
            (
                "description".to_string(),
                FrontmatterValue::Scalar("A one-line summary.".into()),
            ),
            (
                "doc_id".to_string(),
                FrontmatterValue::Scalar("my-title".into()),
            ),
            (
                "layer".to_string(),
                FrontmatterValue::InlineList(vec!["brain".into(), "console".into()]),
            ),
            (
                "project".to_string(),
                FrontmatterValue::Scalar("bastion".into()),
            ),
            (
                "status".to_string(),
                FrontmatterValue::Scalar("active".into()),
            ),
            (
                "keywords".to_string(),
                FrontmatterValue::InlineList(vec![
                    "okf".into(),
                    "frontmatter".into(),
                    "scaffold".into(),
                ]),
            ),
            (
                "related".to_string(),
                FrontmatterValue::InlineList(vec!["okf-core".into()]),
            ),
        ]);
        assert_eq!(flat_out, nested_out);
    }
}
