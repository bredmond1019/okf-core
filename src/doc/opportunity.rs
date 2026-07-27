// `Opportunity` — the full `BrainDocModel` for the brain
// `business/docs/opportunities/index.md` contract: a candidate business
// opportunity captured from a `RESEARCH_AGENT` run (engine-rs `EN.4.A`),
// before promotion to a real pipeline lead.
//
// `kind` and `stage` are deliberately kept `Option<String>`, NOT typed enums.
// `okf-core` is the lenient data model in this stack — it never rejects a
// document for carrying a value outside some allowed set. Enforcing the
// allowed set (`kind: company | prospecting-sweep | job-posting`,
// `stage: identified | researching | … | closed-lost`) is mev's job, the same
// call `OK.1.A` made for the routing/frontmatter fields it validates. Keeping
// the constraint out of the model here means a new `kind`/`stage` value never
// requires an `okf-core` release before mev's allowed-set policy can accept
// it (or reject it, with a real error message, at the validation layer where
// policy belongs).

use serde_json::Value as JsonValue;

use super::frontmatter_value::FrontmatterValue;
use super::model::{BodySection, BodySpec, BrainDocModel, IndexIntent};
use super::slug::derive_slug;

/// A single enriched contact channel for an opportunity. Briefs from
/// `RESEARCH_AGENT` carry no contact data — `contacts` starts empty and is
/// enriched separately (see the contract in
/// `business/docs/opportunities/index.md`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub role: String,
    pub emails: Vec<String>,
    pub whatsapp: Vec<String>,
    pub phones: Vec<String>,
    pub links: Vec<String>,
    pub note: String,
}

/// One append-only history entry recording an action taken on an opportunity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Action {
    pub at: String,
    pub kind: String,
    pub note: String,
}

/// An error recovered while reconstructing an [`Opportunity`] (or its
/// [`Contact`]/[`Action`] entries) from parsed frontmatter fields.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpportunityError {
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
}

/// A candidate business opportunity — the full `okf-core` model for the
/// `business/docs/opportunities/index.md` contract.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opportunity {
    pub title: String,
    pub description: String,
    /// Defaults to `opportunity-<slug>` (see [`Opportunity::frontmatter`])
    /// when absent.
    pub doc_id: Option<String>,
    pub layer: Vec<String>,
    pub status: Option<String>,
    /// `company | prospecting-sweep | job-posting` — lenient, see the module
    /// comment.
    pub kind: Option<String>,
    /// `identified | researching | contacted | conversation | proposal-sent |
    /// closed-won | closed-lost` — lenient, see the module comment.
    pub stage: Option<String>,
    pub source: Option<String>,
    pub url: Option<String>,
    pub links: Vec<String>,
    pub last_contact: Option<String>,
    pub next_action: Option<String>,
    pub research_ref: Option<String>,
    pub contacts: Vec<Contact>,
    pub actions: Vec<Action>,
    /// Free-form body prose rendered before the `## Research Brief` heading
    /// (e.g. `# Title\n\nCandidate opportunity captured …`). `None` renders a
    /// bare `# {title}` heading.
    pub body_prose: Option<String>,
    /// The raw `CompanyBrief`/`ProspectingResult` JSON, embedded verbatim as
    /// the first fenced `json` block in the body. `Value::Null` (the
    /// default) omits the `## Research Brief` section entirely.
    pub research_brief: JsonValue,
}

impl Opportunity {
    /// The document's effective `doc_id`: `self.doc_id` if set, otherwise
    /// `opportunity-<slug>`.
    fn effective_doc_id(&self) -> String {
        self.doc_id
            .clone()
            .unwrap_or_else(|| format!("opportunity-{}", self.slug()))
    }

    /// Build an `Opportunity` from an engine-rs `CompanyBrief` JSON value
    /// (`{company_name, summary, recent_developments[], pain_points[],
    /// outreach_hooks[], sources[]}`, `research_agent/schema.rs`). The raw
    /// JSON is embedded verbatim as the research brief; nothing in the shape
    /// is re-derived or reshaped.
    pub fn from_company_brief(brief: &JsonValue) -> Self {
        let title = json_str(brief, "company_name");
        let description = json_str(brief, "summary");
        Self {
            title,
            description,
            kind: Some("company".to_string()),
            stage: Some("identified".to_string()),
            layer: vec!["business".to_string()],
            research_brief: brief.clone(),
            ..Self::default()
        }
    }

    /// Build an `Opportunity` from an engine-rs `ProspectingResult` JSON
    /// value (`{vertical, prospects[], common_pain_points[], sources[]}`,
    /// `research_agent/schema.rs`). The raw JSON is embedded verbatim as the
    /// research brief; nothing in the shape is re-derived or reshaped.
    pub fn from_prospecting_result(result: &JsonValue) -> Self {
        let vertical = json_str(result, "vertical");
        let title = if vertical.is_empty() {
            String::new()
        } else {
            format!("{vertical} — Prospecting Sweep")
        };
        let description = if vertical.is_empty() {
            "RESEARCH_AGENT prospecting sweep.".to_string()
        } else {
            format!("RESEARCH_AGENT prospecting sweep — {vertical}.")
        };
        Self {
            title,
            description,
            kind: Some("prospecting-sweep".to_string()),
            stage: Some("identified".to_string()),
            layer: vec!["business".to_string()],
            research_brief: result.clone(),
            ..Self::default()
        }
    }

    /// Reconstruct an `Opportunity` from a parsed nested-frontmatter field
    /// list (the read half — round-trips with [`BrainDocModel::frontmatter`]).
    /// Only frontmatter fields are recovered; `body_prose` and
    /// `research_brief` (which live in the document body, not the
    /// frontmatter) are left at their defaults (`None` / `Value::Null`).
    pub fn from_frontmatter(
        fields: &[(String, FrontmatterValue)],
    ) -> Result<Self, OpportunityError> {
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);

        let title = match get("title") {
            Some(FrontmatterValue::Scalar(s)) => s.clone(),
            _ => return Err(OpportunityError::MissingField("title")),
        };
        let description = match get("description") {
            Some(FrontmatterValue::Scalar(s)) => s.clone(),
            _ => return Err(OpportunityError::MissingField("description")),
        };
        let doc_id = scalar_opt(get("doc_id"));
        let layer = match get("layer") {
            Some(FrontmatterValue::InlineList(items)) => items.clone(),
            _ => Vec::new(),
        };
        let status = scalar_opt(get("status"));
        let kind = scalar_opt(get("kind"));
        let stage = scalar_opt(get("stage"));
        let source = scalar_opt(get("source"));
        let url = scalar_opt(get("url"));
        let links = match get("links") {
            Some(FrontmatterValue::BlockList(items)) => items.clone(),
            Some(FrontmatterValue::InlineList(items)) => items.clone(),
            _ => Vec::new(),
        };
        let last_contact = scalar_opt(get("last_contact"));
        let next_action = scalar_opt(get("next_action"));
        let research_ref = scalar_opt(get("research_ref"));
        let contacts = match get("contacts") {
            Some(FrontmatterValue::MapList(entries)) => entries
                .iter()
                .map(|e| Contact::from_entry(e))
                .collect::<Result<Vec<_>, _>>()?,
            _ => Vec::new(),
        };
        let actions = match get("actions") {
            Some(FrontmatterValue::MapList(entries)) => entries
                .iter()
                .map(|e| Action::from_entry(e))
                .collect::<Result<Vec<_>, _>>()?,
            _ => Vec::new(),
        };

        Ok(Self {
            title,
            description,
            doc_id,
            layer,
            status,
            kind,
            stage,
            source,
            url,
            links,
            last_contact,
            next_action,
            research_ref,
            contacts,
            actions,
            body_prose: None,
            research_brief: JsonValue::Null,
        })
    }
}

impl BrainDocModel for Opportunity {
    fn frontmatter(&self) -> Vec<(String, FrontmatterValue)> {
        let mut fields = vec![
            (
                "type".to_string(),
                FrontmatterValue::Scalar("Opportunity".to_string()),
            ),
            (
                "title".to_string(),
                FrontmatterValue::Scalar(self.title.clone()),
            ),
            (
                "description".to_string(),
                FrontmatterValue::Scalar(self.description.clone()),
            ),
            (
                "doc_id".to_string(),
                FrontmatterValue::Scalar(self.effective_doc_id()),
            ),
        ];
        if !self.layer.is_empty() {
            fields.push((
                "layer".to_string(),
                FrontmatterValue::InlineList(self.layer.clone()),
            ));
        }
        if let Some(v) = &self.status {
            fields.push(("status".to_string(), FrontmatterValue::Scalar(v.clone())));
        }
        if let Some(v) = &self.kind {
            fields.push(("kind".to_string(), FrontmatterValue::Scalar(v.clone())));
        }
        if let Some(v) = &self.stage {
            fields.push(("stage".to_string(), FrontmatterValue::Scalar(v.clone())));
        }
        if let Some(v) = &self.source {
            fields.push(("source".to_string(), FrontmatterValue::Scalar(v.clone())));
        }
        if let Some(v) = &self.url {
            fields.push(("url".to_string(), FrontmatterValue::Scalar(v.clone())));
        }
        fields.push((
            "links".to_string(),
            FrontmatterValue::BlockList(self.links.clone()),
        ));
        if let Some(v) = &self.last_contact {
            fields.push((
                "last_contact".to_string(),
                FrontmatterValue::Scalar(v.clone()),
            ));
        }
        if let Some(v) = &self.next_action {
            fields.push((
                "next_action".to_string(),
                FrontmatterValue::Scalar(v.clone()),
            ));
        }
        if let Some(v) = &self.research_ref {
            fields.push((
                "research_ref".to_string(),
                FrontmatterValue::Scalar(v.clone()),
            ));
        }
        fields.push((
            "contacts".to_string(),
            FrontmatterValue::MapList(self.contacts.iter().map(Contact::to_entry).collect()),
        ));
        fields.push((
            "actions".to_string(),
            FrontmatterValue::MapList(self.actions.iter().map(Action::to_entry).collect()),
        ));
        fields
    }

    fn body(&self) -> BodySpec {
        let mut sections = vec![BodySection::Verbatim(
            self.body_prose
                .clone()
                .unwrap_or_else(|| format!("# {}", self.title)),
        )];
        if !self.research_brief.is_null() {
            let json = serde_json::to_string_pretty(&self.research_brief)
                .unwrap_or_else(|_| "{}".to_string());
            sections.push(BodySection::Verbatim(format!(
                "## Research Brief\n```json\n{json}\n```"
            )));
        }
        BodySpec::new(sections)
    }

    fn slug(&self) -> String {
        derive_slug(&self.title)
    }

    fn index_intent(&self) -> IndexIntent {
        IndexIntent::new(
            "business/docs/opportunities/index.md",
            format!("{}.md", self.slug()),
            vec![
                self.title.clone(),
                self.kind.clone().unwrap_or_default(),
                self.stage.clone().unwrap_or_default(),
            ],
        )
    }

    fn doc_type(&self) -> &'static str {
        "opportunity"
    }
}

// ── internal helpers ────────────────────────────────────────────────────────

fn json_str(v: &JsonValue, key: &str) -> String {
    v.get(key)
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .to_string()
}

fn scalar_opt(v: Option<&FrontmatterValue>) -> Option<String> {
    match v {
        Some(FrontmatterValue::Scalar(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Encode a `Vec<String>` sub-field (e.g. `Contact::emails`) as a single
/// inline-map string value: `"[]"` for an empty list, otherwise `", "`-joined
/// (the crate's existing quoting policy quotes any value containing a comma,
/// so this round-trips through the map-entry splitter unquoted-comma-safe).
fn encode_list_field(items: &[String]) -> String {
    if items.is_empty() {
        "[]".to_string()
    } else {
        items.join(", ")
    }
}

/// Inverse of [`encode_list_field`].
fn decode_list_field(v: &str) -> Vec<String> {
    if v.is_empty() || v == "[]" {
        Vec::new()
    } else {
        v.split(", ").map(|s| s.to_string()).collect()
    }
}

impl Contact {
    fn to_entry(&self) -> Vec<(String, String)> {
        vec![
            ("name".to_string(), self.name.clone()),
            ("role".to_string(), self.role.clone()),
            ("emails".to_string(), encode_list_field(&self.emails)),
            ("whatsapp".to_string(), encode_list_field(&self.whatsapp)),
            ("phones".to_string(), encode_list_field(&self.phones)),
            ("links".to_string(), encode_list_field(&self.links)),
            ("note".to_string(), self.note.clone()),
        ]
    }

    fn from_entry(entry: &[(String, String)]) -> Result<Self, OpportunityError> {
        let get = |key: &str| entry.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
        Ok(Self {
            name: get("name").unwrap_or_default(),
            role: get("role").unwrap_or_default(),
            emails: decode_list_field(&get("emails").unwrap_or_default()),
            whatsapp: decode_list_field(&get("whatsapp").unwrap_or_default()),
            phones: decode_list_field(&get("phones").unwrap_or_default()),
            links: decode_list_field(&get("links").unwrap_or_default()),
            note: get("note").unwrap_or_default(),
        })
    }
}

impl Action {
    fn to_entry(&self) -> Vec<(String, String)> {
        vec![
            ("at".to_string(), self.at.clone()),
            ("kind".to_string(), self.kind.clone()),
            ("note".to_string(), self.note.clone()),
        ]
    }

    fn from_entry(entry: &[(String, String)]) -> Result<Self, OpportunityError> {
        let get = |key: &str| entry.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
        Ok(Self {
            at: get("at").unwrap_or_default(),
            kind: get("kind").unwrap_or_default(),
            note: get("note").unwrap_or_default(),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse_nested::parse_nested_frontmatter;
    use crate::doc::render_document;

    fn full() -> Opportunity {
        Opportunity {
            title: "Anthropic".to_string(),
            description: "RESEARCH_AGENT company brief.".to_string(),
            doc_id: None,
            layer: vec!["business".to_string()],
            status: Some("active".to_string()),
            kind: Some("company".to_string()),
            stage: Some("identified".to_string()),
            source: Some("RESEARCH_AGENT test run (company mode)".to_string()),
            url: Some("https://www.anthropic.com".to_string()),
            links: vec!["https://www.anthropic.com".to_string()],
            last_contact: Some("2026-07-25".to_string()),
            next_action: Some("Send outreach email".to_string()),
            research_ref: Some("engine-rs-research-runs".to_string()),
            contacts: vec![],
            actions: vec![Action {
                at: "2026-07-25".to_string(),
                kind: "research".to_string(),
                note: "Generated brief".to_string(),
            }],
            body_prose: None,
            research_brief: JsonValue::Null,
        }
    }

    // ── Exact frontmatter output ────────────────────────────────────────────

    #[test]
    fn fully_populated_opportunity_renders_exact_expected_frontmatter() {
        let o = full();
        let out = super::super::frontmatter_value::serialize_nested_frontmatter(&o.frontmatter());
        let expected = "\
---
type: Opportunity
title: Anthropic
description: RESEARCH_AGENT company brief.
doc_id: opportunity-anthropic
layer: [business]
status: active
kind: company
stage: identified
source: RESEARCH_AGENT test run (company mode)
url: \"https://www.anthropic.com\"
links:
  - \"https://www.anthropic.com\"
last_contact: 2026-07-25
next_action: Send outreach email
research_ref: engine-rs-research-runs
contacts: []
actions:
  - { at: 2026-07-25, kind: research, note: Generated brief }
---
";
        assert_eq!(out, expected);
    }

    #[test]
    fn empty_contacts_renders_as_empty_list() {
        let o = Opportunity {
            title: "Test Co".to_string(),
            description: "D".to_string(),
            ..Opportunity::default()
        };
        let fields = o.frontmatter();
        let contacts = fields
            .iter()
            .find(|(k, _)| k == "contacts")
            .map(|(_, v)| v.clone());
        assert_eq!(contacts, Some(FrontmatterValue::MapList(vec![])));
    }

    // ── Constructors ─────────────────────────────────────────────────────────

    #[test]
    fn from_company_brief_sets_kind_title_and_brief() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "Widget manufacturer expanding into SaaS.",
            "recent_developments": ["Raised Series B"],
            "pain_points": [],
            "outreach_hooks": [],
            "sources": [],
        });
        let o = Opportunity::from_company_brief(&brief);
        assert_eq!(o.title, "Acme Corp");
        assert_eq!(o.description, "Widget manufacturer expanding into SaaS.");
        assert_eq!(o.kind.as_deref(), Some("company"));
        assert_eq!(o.research_brief, brief);
    }

    #[test]
    fn from_prospecting_result_sets_kind_title_and_brief() {
        let result = serde_json::json!({
            "vertical": "legal-tech",
            "prospects": [],
            "common_pain_points": ["Manual contract review"],
            "sources": [],
        });
        let o = Opportunity::from_prospecting_result(&result);
        assert_eq!(o.title, "legal-tech — Prospecting Sweep");
        assert_eq!(o.kind.as_deref(), Some("prospecting-sweep"));
        assert_eq!(o.research_brief, result);
    }

    // ── Round-trip ───────────────────────────────────────────────────────────

    #[test]
    fn from_frontmatter_of_parsed_render_equals_original() {
        let o = full();
        let rendered =
            super::super::frontmatter_value::serialize_nested_frontmatter(&o.frontmatter());
        let parsed = parse_nested_frontmatter(&rendered).expect("must parse");
        let recovered = Opportunity::from_frontmatter(&parsed).expect("must reconstruct");

        // `body_prose`/`research_brief` live in the body, not the frontmatter,
        // so they are not recoverable from `from_frontmatter` alone — hold
        // `o` to the same defaults before comparing.
        let mut expected = o;
        expected.doc_id = Some(expected.effective_doc_id());
        assert_eq!(recovered, expected);
    }

    #[test]
    fn render_document_contains_research_brief_as_first_json_fence() {
        let mut o = full();
        o.research_brief = serde_json::json!({"company_name": "Anthropic"});
        let rendered = render_document(&o);
        let json_fence_pos = rendered.find("```json").expect("must contain json fence");
        let heading_pos = rendered
            .find("## Research Brief")
            .expect("must contain heading");
        assert!(heading_pos < json_fence_pos);
        assert!(rendered.contains("\"company_name\": \"Anthropic\""));
    }

    #[test]
    fn contact_round_trips_through_entry() {
        let c = Contact {
            name: "Alice".to_string(),
            role: "CTO".to_string(),
            emails: vec!["alice@example.com".to_string(), "a@corp.com".to_string()],
            whatsapp: vec![],
            phones: vec!["+1-555-0100".to_string()],
            links: vec![],
            note: "Met at conference".to_string(),
        };
        let entry = Contact::to_entry(&c);
        let recovered = Contact::from_entry(&entry).expect("must reconstruct");
        assert_eq!(recovered, c);
    }
}
