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

/// A single enriched contact channel for an opportunity. `contacts` starts
/// empty and is populated by the `RESEARCH_AGENT` merge-contacts step
/// (engine-rs `EN.4.E`, `mev::doc::opportunity::plan_merge_contacts`) — see
/// the contract in `business/docs/opportunities/index.md`.
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
    /// Domain-specific claims the model could not tie to a fetched source
    /// (engine-rs `EN.4.G`, `research_agent/schema.rs`'s
    /// `needs_further_research`). Always emitted, empty when nothing is
    /// flagged — a flagged claim is kept, not deleted, and an empty list is
    /// the correct answer for a fully-grounded brief. `validation_required`
    /// is deliberately not a stored field here; see
    /// [`Opportunity::validation_required`].
    pub needs_further_research: Vec<String>,
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

    /// Whether a human still needs to validate at least one flagged claim —
    /// always derived from `needs_further_research`, never independently
    /// settable, so a document cannot contradict its own list.
    pub fn validation_required(&self) -> bool {
        !self.needs_further_research.is_empty()
    }

    /// Build an `Opportunity` from an engine-rs `CompanyBrief` JSON value
    /// (`{company_name, summary, recent_developments[], pain_points[],
    /// outreach_hooks[], sources[]}`, `research_agent/schema.rs`). The raw
    /// JSON is embedded verbatim as the research brief; nothing in the shape
    /// is re-derived or reshaped.
    pub fn from_company_brief(brief: &JsonValue) -> Self {
        let title = json_str(brief, "company_name");
        let description = json_str(brief, "summary");
        let url = json_str_opt(brief, "company_url");
        let links = json_str_array_deduped(brief, "sources");
        let needs_further_research = json_str_array_deduped(brief, "needs_further_research");
        Self {
            title,
            description,
            kind: Some("company".to_string()),
            stage: Some("identified".to_string()),
            layer: vec!["business".to_string()],
            url,
            links,
            needs_further_research,
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
        let links = json_str_array_deduped(result, "sources");
        let needs_further_research = needs_further_research_union(result);
        Self {
            title,
            description,
            kind: Some("prospecting-sweep".to_string()),
            stage: Some("identified".to_string()),
            layer: vec!["business".to_string()],
            links,
            needs_further_research,
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
        // `validation_required` (if present in the source) is intentionally
        // never read here — it is always re-derived from
        // `needs_further_research` (see `Opportunity::validation_required`),
        // so a stale value in the frontmatter cannot leak into the model.
        let needs_further_research = match get("needs_further_research") {
            Some(FrontmatterValue::BlockList(items)) => items.clone(),
            Some(FrontmatterValue::InlineList(items)) => items.clone(),
            _ => Vec::new(),
        };
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
            needs_further_research,
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
            "needs_further_research".to_string(),
            FrontmatterValue::BlockList(self.needs_further_research.clone()),
        ));
        fields.push((
            "validation_required".to_string(),
            FrontmatterValue::Scalar(self.validation_required().to_string()),
        ));
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

/// Read a string field, returning `None` when absent or blank (after
/// trimming) rather than an empty string.
fn json_str_opt(v: &JsonValue, key: &str) -> Option<String> {
    v.get(key).and_then(JsonValue::as_str).and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Read an array field of strings, skipping non-string/blank entries and
/// deduping while preserving first-seen order.
fn json_str_array_deduped(v: &JsonValue, key: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.get(key)
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(JsonValue::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .filter(|s| seen.insert(s.to_string()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve the sweep-level `needs_further_research` for a `ProspectingResult`
/// value: prefer a top-level `needs_further_research` key when the result
/// already carries one (engine-rs `EN.4.G` stamps the order-stable, deduped
/// union there), otherwise compute the same union across `prospects[]`
/// ourselves so a bare `ProspectingResult` still round-trips correctly.
fn needs_further_research_union(result: &JsonValue) -> Vec<String> {
    if result.get("needs_further_research").is_some() {
        return json_str_array_deduped(result, "needs_further_research");
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Some(prospects) = result.get("prospects").and_then(JsonValue::as_array) {
        for prospect in prospects {
            for claim in json_str_array_deduped(prospect, "needs_further_research") {
                if seen.insert(claim.clone()) {
                    out.push(claim);
                }
            }
        }
    }
    out
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
            needs_further_research: vec![],
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
needs_further_research: []
validation_required: \"false\"
contacts: []
actions:
  - { at: 2026-07-25, kind: research, note: Generated brief }
---
";
        assert_eq!(out, expected);
    }

    #[test]
    fn populated_needs_further_research_renders_as_block_list_with_validation_required_true() {
        let mut o = full();
        o.needs_further_research = vec![
            "FAR/DFARS compliance regime claimed but not sourced".to_string(),
            "Brazilian local-LLM data-residency claim unverified".to_string(),
        ];
        let out = super::super::frontmatter_value::serialize_nested_frontmatter(&o.frontmatter());
        assert!(out.contains(
            "needs_further_research:\n  - FAR/DFARS compliance regime claimed but not sourced\n  - Brazilian local-LLM data-residency claim unverified\n"
        ));
        assert!(out.contains("validation_required: \"true\"\n"));
    }

    #[test]
    fn empty_needs_further_research_renders_present_empty_list_with_validation_required_false() {
        let o = full();
        let out = super::super::frontmatter_value::serialize_nested_frontmatter(&o.frontmatter());
        assert!(out.contains("needs_further_research: []\n"));
        assert!(out.contains("validation_required: \"false\"\n"));
    }

    #[test]
    fn validation_required_is_derived_never_independently_set() {
        let mut o = full();
        assert!(!o.validation_required());
        o.needs_further_research = vec!["claim".to_string()];
        assert!(o.validation_required());
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
    fn from_company_brief_lifts_company_url_and_sources() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "Widget manufacturer expanding into SaaS.",
            "company_url": "https://acme.example.com",
            "sources": [
                "https://acme.example.com/about",
                "https://news.example.com/acme-raises-b",
                "https://acme.example.com/about",
            ],
        });
        let o = Opportunity::from_company_brief(&brief);
        assert_eq!(o.url.as_deref(), Some("https://acme.example.com"));
        assert_eq!(
            o.links,
            vec![
                "https://acme.example.com/about".to_string(),
                "https://news.example.com/acme-raises-b".to_string(),
            ]
        );
    }

    #[test]
    fn from_company_brief_absent_company_url_and_sources_leave_defaults() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "Widget manufacturer expanding into SaaS.",
        });
        let o = Opportunity::from_company_brief(&brief);
        assert_eq!(o.url, None);
        assert!(o.links.is_empty());
    }

    #[test]
    fn from_company_brief_blank_company_url_stays_none() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "D",
            "company_url": "   ",
        });
        let o = Opportunity::from_company_brief(&brief);
        assert_eq!(o.url, None);
    }

    #[test]
    fn from_company_brief_non_string_and_blank_source_entries_skipped() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "D",
            "sources": ["https://acme.example.com", 42, "", "   ", null, "https://acme.example.com"],
        });
        let o = Opportunity::from_company_brief(&brief);
        assert_eq!(o.links, vec!["https://acme.example.com".to_string()]);
    }

    #[test]
    fn from_company_brief_maps_needs_further_research_deduped() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "D",
            "needs_further_research": [
                "FAR/DFARS compliance regime claimed but not sourced",
                "FAR/DFARS compliance regime claimed but not sourced",
                "Brazilian local-LLM data-residency claim unverified",
            ],
        });
        let o = Opportunity::from_company_brief(&brief);
        assert_eq!(
            o.needs_further_research,
            vec![
                "FAR/DFARS compliance regime claimed but not sourced".to_string(),
                "Brazilian local-LLM data-residency claim unverified".to_string(),
            ]
        );
        assert!(o.validation_required());
        // Existing fields stay unchanged.
        assert_eq!(o.title, "Acme Corp");
        assert_eq!(o.kind.as_deref(), Some("company"));
    }

    #[test]
    fn from_company_brief_absent_needs_further_research_is_empty() {
        let brief = serde_json::json!({
            "company_name": "Acme Corp",
            "summary": "D",
        });
        let o = Opportunity::from_company_brief(&brief);
        assert!(o.needs_further_research.is_empty());
        assert!(!o.validation_required());
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

    #[test]
    fn from_prospecting_result_lifts_sources_into_links_deduped() {
        let result = serde_json::json!({
            "vertical": "legal-tech",
            "prospects": [],
            "sources": [
                "https://legaltech.example.com/directory",
                "https://legaltech.example.com/directory",
                "https://news.example.com/legal-tech-report",
            ],
        });
        let o = Opportunity::from_prospecting_result(&result);
        assert_eq!(
            o.links,
            vec![
                "https://legaltech.example.com/directory".to_string(),
                "https://news.example.com/legal-tech-report".to_string(),
            ]
        );
    }

    #[test]
    fn from_prospecting_result_prefers_stamped_top_level_union() {
        let result = serde_json::json!({
            "vertical": "legal-tech",
            "prospects": [
                {"needs_further_research": ["stale, per-lead only"]},
            ],
            "needs_further_research": [
                "FAR/DFARS compliance regime claimed but not sourced",
                "Brazilian local-LLM data-residency claim unverified",
            ],
        });
        let o = Opportunity::from_prospecting_result(&result);
        assert_eq!(
            o.needs_further_research,
            vec![
                "FAR/DFARS compliance regime claimed but not sourced".to_string(),
                "Brazilian local-LLM data-residency claim unverified".to_string(),
            ]
        );
        assert!(o.validation_required());
    }

    #[test]
    fn from_prospecting_result_computes_union_across_prospects_deduped_when_unstamped() {
        let result = serde_json::json!({
            "vertical": "legal-tech",
            "prospects": [
                {"needs_further_research": ["FAR/DFARS compliance regime claimed but not sourced"]},
                {"needs_further_research": [
                    "FAR/DFARS compliance regime claimed but not sourced",
                    "Brazilian local-LLM data-residency claim unverified",
                ]},
                {"needs_further_research": []},
            ],
        });
        let o = Opportunity::from_prospecting_result(&result);
        assert_eq!(
            o.needs_further_research,
            vec![
                "FAR/DFARS compliance regime claimed but not sourced".to_string(),
                "Brazilian local-LLM data-residency claim unverified".to_string(),
            ]
        );
        assert!(o.validation_required());
    }

    #[test]
    fn from_prospecting_result_no_flags_anywhere_is_empty() {
        let result = serde_json::json!({
            "vertical": "legal-tech",
            "prospects": [
                {"needs_further_research": []},
                {},
            ],
        });
        let o = Opportunity::from_prospecting_result(&result);
        assert!(o.needs_further_research.is_empty());
        assert!(!o.validation_required());
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
    fn from_frontmatter_of_parsed_render_equals_original_with_populated_needs_further_research() {
        let mut o = full();
        o.needs_further_research = vec![
            "FAR/DFARS compliance regime claimed but not sourced".to_string(),
            "Brazilian local-LLM data-residency claim unverified".to_string(),
        ];
        let rendered =
            super::super::frontmatter_value::serialize_nested_frontmatter(&o.frontmatter());
        let parsed = parse_nested_frontmatter(&rendered).expect("must parse");
        let recovered = Opportunity::from_frontmatter(&parsed).expect("must reconstruct");

        let mut expected = o;
        expected.doc_id = Some(expected.effective_doc_id());
        assert_eq!(recovered, expected);
        assert!(recovered.validation_required());
    }

    #[test]
    fn stale_validation_required_true_alongside_empty_list_reads_back_derived_false() {
        // A document whose frontmatter carries a stale `validation_required:
        // true` next to an empty `needs_further_research:` must read back
        // derived-false — the stored scalar is never trusted, only the list.
        let fields = vec![
            (
                "title".to_string(),
                FrontmatterValue::Scalar("Test Co".to_string()),
            ),
            (
                "description".to_string(),
                FrontmatterValue::Scalar("D".to_string()),
            ),
            (
                "needs_further_research".to_string(),
                FrontmatterValue::BlockList(vec![]),
            ),
            (
                "validation_required".to_string(),
                FrontmatterValue::Scalar("true".to_string()),
            ),
        ];
        let recovered = Opportunity::from_frontmatter(&fields).expect("must reconstruct");
        assert!(recovered.needs_further_research.is_empty());
        assert!(!recovered.validation_required());
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
