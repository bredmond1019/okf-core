// Nested frontmatter value model + serializer.
//
// Extends `crate::frontmatter`'s flat `key: value` / `key: [a, b]` model with the
// two additional shapes real brain docs use: block lists (`links:` + `  - <url>`
// lines) and inline-map lists (`contacts:` / `actions:` + `  - { k: v, … }`
// lines) — e.g. the live `business/docs/opportunities/anthropic.md` shape.
//
// Reuses `crate::frontmatter`'s exact quoting policy (`needs_quote` / `yaml_scalar`,
// widened to `pub(crate)` for this purpose) rather than forking a second one, so a
// value is quoted identically whether it appears in a flat scalar, an inline list,
// or an inline-map value.

use crate::frontmatter::yaml_scalar;

/// A single frontmatter field's value, covering the four shapes real brain docs use.
///
/// Entry order is explicit (`Vec`, never a `HashMap`) throughout, including within
/// each `MapList` entry's field pairs, so serialized output is deterministic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterValue {
    /// `key: value` — or a bare `key:` when the value is empty (the OKF
    /// "present but unfilled" backfill signal used by `serialize_frontmatter`).
    Scalar(String),
    /// `key: [a, b, c]` — e.g. `layer: [brain, console]`.
    InlineList(Vec<String>),
    /// `key:` followed by one `  - <item>` line per entry — e.g. `links:` /
    /// `  - https://example.com`. An empty list still renders `key: []`.
    BlockList(Vec<String>),
    /// `key:` followed by one `  - { k: v, … }` line per entry — e.g. `actions:` /
    /// `  - { at: 2026-07-25, kind: research, note: "…" }`. An empty list still
    /// renders `key: []`. Each entry is an ordered list of `(field, value)` pairs.
    MapList(Vec<Vec<(String, String)>>),
}

/// Serialize a nested-aware frontmatter field list into a canonical `---`-fenced
/// YAML block.
///
/// Fields are emitted in the given order, exactly as given — this function has no
/// notion of "required" or "optional"; the caller decides which fields to include
/// (e.g. by omitting an absent optional field entirely, matching
/// `serialize_frontmatter`'s omission behaviour for the flat model). The returned
/// string includes both fences and a trailing newline, matching
/// `serialize_frontmatter`'s conventions exactly for the `Scalar` / `InlineList`
/// shapes — a flat field set serializes byte-identically either way.
pub fn serialize_nested_frontmatter(fields: &[(String, FrontmatterValue)]) -> String {
    let mut out = String::from("---\n");
    for (key, value) in fields {
        push_value(&mut out, key, value);
    }
    out.push_str("---\n");
    out
}

fn push_value(out: &mut String, key: &str, value: &FrontmatterValue) {
    match value {
        FrontmatterValue::Scalar(v) => push_scalar(out, key, v),
        FrontmatterValue::InlineList(items) => push_inline_list(out, key, items),
        FrontmatterValue::BlockList(items) => push_block_list(out, key, items),
        FrontmatterValue::MapList(entries) => push_map_list(out, key, entries),
    }
}

/// Append `key: value` (or a bare `key:` when `value` is empty) plus a newline.
/// Mirrors `crate::frontmatter::push_scalar` exactly.
fn push_scalar(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    if value.is_empty() {
        out.push(':');
    } else {
        out.push_str(": ");
        out.push_str(&yaml_scalar(value));
    }
    out.push('\n');
}

/// Append `key: [a, b, c]` plus a newline. Mirrors `crate::frontmatter::push_list`
/// exactly, including for an empty list (`key: []`).
fn push_inline_list(out: &mut String, key: &str, items: &[String]) {
    out.push_str(key);
    out.push_str(": [");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&yaml_scalar(item));
    }
    out.push_str("]\n");
}

/// Append `key:` followed by one `  - <item>` line per entry, or `key: []` when
/// `items` is empty.
fn push_block_list(out: &mut String, key: &str, items: &[String]) {
    if items.is_empty() {
        out.push_str(key);
        out.push_str(": []\n");
        return;
    }
    out.push_str(key);
    out.push_str(":\n");
    for item in items {
        out.push_str("  - ");
        out.push_str(&yaml_scalar(item));
        out.push('\n');
    }
}

/// Append `key:` followed by one `  - { k: v, … }` line per entry, or `key: []`
/// when `entries` is empty. Values are quoted by `yaml_scalar`; keys never are —
/// they are trusted field names (`at`, `kind`, `note`, …), not free-form content.
fn push_map_list(out: &mut String, key: &str, entries: &[Vec<(String, String)>]) {
    if entries.is_empty() {
        out.push_str(key);
        out.push_str(": []\n");
        return;
    }
    out.push_str(key);
    out.push_str(":\n");
    for entry in entries {
        out.push_str("  - { ");
        for (i, (k, v)) in entry.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(k);
            out.push_str(": ");
            out.push_str(&yaml_scalar(v));
        }
        out.push_str(" }\n");
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::{OkfFrontmatter, serialize_frontmatter};

    fn field(key: &str, value: FrontmatterValue) -> (String, FrontmatterValue) {
        (key.to_string(), value)
    }

    // ── Exact-output tests, one per shape ───────────────────────────────────────

    #[test]
    fn scalar_exact_output() {
        let out = serialize_nested_frontmatter(&[field(
            "title",
            FrontmatterValue::Scalar("Anthropic".into()),
        )]);
        assert_eq!(out, "---\ntitle: Anthropic\n---\n");
    }

    #[test]
    fn scalar_empty_renders_bare_key() {
        let out = serialize_nested_frontmatter(&[field(
            "title",
            FrontmatterValue::Scalar(String::new()),
        )]);
        assert_eq!(out, "---\ntitle:\n---\n");
    }

    #[test]
    fn inline_list_exact_output() {
        let out = serialize_nested_frontmatter(&[field(
            "layer",
            FrontmatterValue::InlineList(vec!["brain".into(), "business".into()]),
        )]);
        assert_eq!(out, "---\nlayer: [brain, business]\n---\n");
    }

    #[test]
    fn block_list_exact_output() {
        let out = serialize_nested_frontmatter(&[field(
            "keywords",
            FrontmatterValue::BlockList(vec!["okf".into()]),
        )]);
        assert_eq!(out, "---\nkeywords:\n  - okf\n---\n");
    }

    #[test]
    fn block_list_multiple_items_exact_output() {
        let out = serialize_nested_frontmatter(&[field(
            "keywords",
            FrontmatterValue::BlockList(vec!["okf".into(), "frontmatter".into()]),
        )]);
        assert_eq!(out, "---\nkeywords:\n  - okf\n  - frontmatter\n---\n");
    }

    #[test]
    fn block_list_url_item_is_quoted_for_its_colon() {
        // Reuses the same colon-triggers-quote rule as scalars/inline lists — the
        // live `business/docs/opportunities/anthropic.md` file is hand-authored and
        // leaves URLs unquoted, but this crate's one quoting policy (`needs_quote`)
        // is deliberately conservative and quotes any value containing `:`.
        let out = serialize_nested_frontmatter(&[field(
            "links",
            FrontmatterValue::BlockList(vec!["https://www.anthropic.com".into()]),
        )]);
        assert_eq!(out, "---\nlinks:\n  - \"https://www.anthropic.com\"\n---\n");
    }

    #[test]
    fn map_list_exact_output() {
        let out = serialize_nested_frontmatter(&[field(
            "actions",
            FrontmatterValue::MapList(vec![vec![
                ("at".to_string(), "2026-07-25".to_string()),
                ("kind".to_string(), "research".to_string()),
                ("note".to_string(), "Generated brief".to_string()),
            ]]),
        )]);
        assert_eq!(
            out,
            "---\nactions:\n  - { at: 2026-07-25, kind: research, note: Generated brief }\n---\n"
        );
    }

    // ── Empty BlockList / MapList render as `key: []` ───────────────────────────

    #[test]
    fn empty_block_list_renders_bracket_pair() {
        let out =
            serialize_nested_frontmatter(&[field("links", FrontmatterValue::BlockList(vec![]))]);
        assert_eq!(out, "---\nlinks: []\n---\n");
    }

    #[test]
    fn empty_map_list_renders_bracket_pair() {
        // Matches the live `contacts: []` in business/docs/opportunities/anthropic.md.
        let out =
            serialize_nested_frontmatter(&[field("contacts", FrontmatterValue::MapList(vec![]))]);
        assert_eq!(out, "---\ncontacts: []\n---\n");
    }

    // ── Quoting inside inline-map values ────────────────────────────────────────

    #[test]
    fn map_list_value_needing_quote_is_quoted() {
        let out = serialize_nested_frontmatter(&[field(
            "actions",
            FrontmatterValue::MapList(vec![vec![
                ("at".to_string(), "2026-07-25".to_string()),
                ("note".to_string(), "ratio 3:1, done".to_string()),
            ]]),
        )]);
        assert_eq!(
            out,
            "---\nactions:\n  - { at: 2026-07-25, note: \"ratio 3:1, done\" }\n---\n"
        );
    }

    #[test]
    fn map_list_multiple_entries_exact_output() {
        let out = serialize_nested_frontmatter(&[field(
            "contacts",
            FrontmatterValue::MapList(vec![
                vec![("name".to_string(), "Alice".to_string())],
                vec![("name".to_string(), "has: colon".to_string())],
            ]),
        )]);
        assert_eq!(
            out,
            "---\ncontacts:\n  - { name: Alice }\n  - { name: \"has: colon\" }\n---\n"
        );
    }

    // ── Flat field set matches serialize_frontmatter byte-for-byte ─────────────

    #[test]
    fn flat_fields_byte_identical_to_serialize_frontmatter() {
        let fm = OkfFrontmatter {
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
        };
        let flat_out = serialize_frontmatter(&fm);

        let nested_out = serialize_nested_frontmatter(&[
            field("type", FrontmatterValue::Scalar("Guideline".into())),
            field("title", FrontmatterValue::Scalar("My Title".into())),
            field(
                "description",
                FrontmatterValue::Scalar("A one-line summary.".into()),
            ),
            field("doc_id", FrontmatterValue::Scalar("my-title".into())),
            field(
                "layer",
                FrontmatterValue::InlineList(vec!["brain".into(), "console".into()]),
            ),
            field("project", FrontmatterValue::Scalar("bastion".into())),
            field("status", FrontmatterValue::Scalar("active".into())),
            field(
                "keywords",
                FrontmatterValue::InlineList(vec![
                    "okf".into(),
                    "frontmatter".into(),
                    "scaffold".into(),
                ]),
            ),
            field(
                "related",
                FrontmatterValue::InlineList(vec!["okf-core".into()]),
            ),
        ]);

        assert_eq!(flat_out, nested_out);
    }

    #[test]
    fn flat_default_required_fields_byte_identical() {
        let flat_out = serialize_frontmatter(&OkfFrontmatter::default());
        let nested_out = serialize_nested_frontmatter(&[
            field("type", FrontmatterValue::Scalar(String::new())),
            field("title", FrontmatterValue::Scalar(String::new())),
            field("description", FrontmatterValue::Scalar(String::new())),
        ]);
        assert_eq!(flat_out, nested_out);
    }
}
