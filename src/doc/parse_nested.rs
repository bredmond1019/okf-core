// Nested frontmatter parser — the read half of the round-trip.
//
// `crate::parse::extract_frontmatter` only understands flat `key: value` lines; it
// misparses a `  - https://…` block-list continuation line as a field literally
// named `- https`, and has no notion of `  - { k: v, … }` inline-map continuation
// lines at all. This module recovers all four `FrontmatterValue` shapes — scalar,
// inline list, block list, and inline-map list — from real brain-doc frontmatter
// (the `business/docs/opportunities/anthropic.md` shape: `links` as a block list,
// `contacts: []` as an empty list, `actions[]` as a one-entry inline-map list with
// a quoted `note` field).
//
// `src/parse.rs` is left untouched: `extract_frontmatter`/`parse_frontmatter` keep
// their exact current behaviour and signatures. This module is purely additive.

use super::frontmatter_value::FrontmatterValue;

/// An error recovered while parsing nested frontmatter, carrying a 1-based source
/// line number — mirroring `crate::parse::ParseResult::MalformedLine`'s convention.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NestedParseError {
    /// Opening `---` found but no closing fence before EOF.
    #[error("unterminated frontmatter fence opened at line {open_line}")]
    UnterminatedFence { open_line: usize },
    /// A top-level line inside the block is not a valid `key: value` pair, or a
    /// continuation line (`  - …`) appears where no key introduced a list.
    #[error("malformed frontmatter line at line {source_line}")]
    MalformedLine { source_line: usize },
    /// A `  - { … }` continuation line is not a well-formed inline map (missing
    /// braces, a `key` with no `:`, or an empty key).
    #[error("malformed inline map at line {source_line}")]
    MalformedInlineMap { source_line: usize },
    /// A double-quoted value opens but never closes.
    #[error("unterminated quoted value at line {source_line}")]
    UnterminatedQuote { source_line: usize },
}

/// Parse the leading `---`-fenced frontmatter block from `content`, recovering all
/// four `FrontmatterValue` shapes (scalar, inline list, block list, inline-map
/// list). Key order in the source is preserved.
///
/// The first line must be exactly `---` (after trimming trailing whitespace); a
/// content that does not open with the fence is reported as a malformed line at 1,
/// matching this module's error surface (there is deliberately no "no frontmatter"
/// variant here — callers that need to distinguish "absent" from "malformed" should
/// probe with `crate::parse::extract_frontmatter` first).
pub fn parse_nested_frontmatter(
    content: &str,
) -> Result<Vec<(String, FrontmatterValue)>, NestedParseError> {
    let lines: Vec<&str> = content.lines().collect();

    match lines.first() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return Err(NestedParseError::MalformedLine { source_line: 1 }),
    }
    let open_line = 1usize;

    let mut fields: Vec<(String, FrontmatterValue)> = Vec::new();
    let mut i = 1usize; // index into `lines`, 0-based; source_line = i + 1

    while i < lines.len() {
        let raw = lines[i];
        let source_line = i + 1;
        let trimmed_end = raw.trim_end();

        if trimmed_end == "---" {
            return Ok(fields);
        }

        // Top-level key lines must start at column 0 — a stray indented line here
        // (a continuation with no preceding bare `key:`) is malformed.
        if raw.starts_with(' ') || raw.starts_with('\t') {
            return Err(NestedParseError::MalformedLine { source_line });
        }

        let colon_pos = trimmed_end
            .find(':')
            .ok_or(NestedParseError::MalformedLine { source_line })?;
        let key = trimmed_end[..colon_pos].trim().to_string();
        if key.is_empty() {
            return Err(NestedParseError::MalformedLine { source_line });
        }
        let value_part = trimmed_end[colon_pos + 1..].trim();

        if value_part.is_empty() {
            // Bare `key:` — either a block list, a map list, or (with no
            // continuation lines) an empty scalar.
            let (value, next_i) = parse_continuation(&lines, i + 1)?;
            fields.push((
                key,
                value.unwrap_or(FrontmatterValue::Scalar(String::new())),
            ));
            i = next_i;
        } else if let Some(inner) = value_part
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
        {
            let items = parse_inline_list_items(inner, source_line)?;
            fields.push((key, FrontmatterValue::InlineList(items)));
            i += 1;
        } else {
            let scalar = unquote(value_part, source_line)?;
            fields.push((key, FrontmatterValue::Scalar(scalar)));
            i += 1;
        }
    }

    Err(NestedParseError::UnterminatedFence { open_line })
}

/// Starting at `start` (the line right after a bare `key:`), consume any run of
/// `  - …` continuation lines and classify them as a block list or a map list.
/// Returns `(None, start)` when there is no continuation (the caller treats the
/// key as an empty scalar), or `(Some(value), next_index)` past the consumed run.
fn parse_continuation(
    lines: &[&str],
    start: usize,
) -> Result<(Option<FrontmatterValue>, usize), NestedParseError> {
    let mut j = start;
    let mut block_items: Vec<String> = Vec::new();
    let mut map_items: Vec<Vec<(String, String)>> = Vec::new();
    let mut is_map: Option<bool> = None;

    while j < lines.len() {
        let Some(rest) = lines[j].strip_prefix("  - ") else {
            break;
        };
        let source_line = j + 1;
        let rest = rest.trim_end();

        if let Some(inner) = rest.strip_prefix('{') {
            if is_map == Some(false) {
                return Err(NestedParseError::MalformedLine { source_line });
            }
            is_map = Some(true);
            let entry = parse_map_entry(inner, source_line)?;
            map_items.push(entry);
        } else {
            if is_map == Some(true) {
                return Err(NestedParseError::MalformedLine { source_line });
            }
            is_map = Some(false);
            block_items.push(unquote(rest, source_line)?);
        }
        j += 1;
    }

    if j == start {
        return Ok((None, start));
    }
    match is_map {
        Some(true) => Ok((Some(FrontmatterValue::MapList(map_items)), j)),
        Some(false) => Ok((Some(FrontmatterValue::BlockList(block_items)), j)),
        None => unreachable!("j != start implies at least one continuation line was consumed"),
    }
}

/// Parse the comma-separated contents of an inline list (`[a, b, c]`, `[]`).
fn parse_inline_list_items(
    inner: &str,
    source_line: usize,
) -> Result<Vec<String>, NestedParseError> {
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    split_top_level_commas(inner, source_line)?
        .into_iter()
        .map(|part| unquote(part.trim(), source_line))
        .collect()
}

/// Parse a `{ k: v, … }` inline-map entry body (with the leading `{` already
/// stripped by the caller; the trailing `}` is stripped here).
fn parse_map_entry(
    body_after_open_brace: &str,
    source_line: usize,
) -> Result<Vec<(String, String)>, NestedParseError> {
    let inner = body_after_open_brace
        .trim_end()
        .strip_suffix('}')
        .ok_or(NestedParseError::MalformedInlineMap { source_line })?;

    let mut entries = Vec::new();
    for part in split_top_level_commas(inner, source_line)? {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let colon_pos = part
            .find(':')
            .ok_or(NestedParseError::MalformedInlineMap { source_line })?;
        let k = part[..colon_pos].trim().to_string();
        if k.is_empty() {
            return Err(NestedParseError::MalformedInlineMap { source_line });
        }
        let v = unquote(part[colon_pos + 1..].trim(), source_line)?;
        entries.push((k, v));
    }
    Ok(entries)
}

/// Split `s` on top-level commas — commas inside a double-quoted span do not
/// split. A quote that opens but never closes is `UnterminatedQuote`.
fn split_top_level_commas(s: &str, source_line: usize) -> Result<Vec<String>, NestedParseError> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                current.push(c);
            }
            '\\' if in_quote => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    return Err(NestedParseError::UnterminatedQuote { source_line });
                }
            }
            ',' if !in_quote => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if in_quote {
        return Err(NestedParseError::UnterminatedQuote { source_line });
    }
    parts.push(current);
    Ok(parts)
}

/// Strip a well-formed surrounding double-quote pair and unescape `\"`/`\\`,
/// mirroring `crate::frontmatter::yaml_scalar`'s escaping. A value that is not
/// quoted at all is returned as-is. A value that opens with `"` but never closes
/// is `UnterminatedQuote`.
fn unquote(v: &str, source_line: usize) -> Result<String, NestedParseError> {
    let Some(after_open) = v.strip_prefix('"') else {
        return Ok(v.to_string());
    };
    let Some(inner) = after_open
        .strip_suffix('"')
        .filter(|_| !after_open.is_empty())
    else {
        return Err(NestedParseError::UnterminatedQuote { source_line });
    };

    let mut result = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) => result.push(next),
                None => return Err(NestedParseError::UnterminatedQuote { source_line }),
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::frontmatter_value::serialize_nested_frontmatter;
    use super::*;

    /// A block modeled on the live `business/docs/opportunities/anthropic.md`
    /// shape: a `links` block list, an empty `contacts` list, and one `actions[]`
    /// inline map with a quoted `note` that itself contains a comma (proving the
    /// map-entry splitter honours quotes).
    const ANTHROPIC_SHAPE: &str = "\
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
url: https://www.anthropic.com
links:
  - https://www.anthropic.com
research_ref: engine-rs-research-runs
contacts: []
actions:
  - { at: 2026-07-25, kind: research, note: \"Generated brief, cut short\" }
---
";

    fn find<'a>(fields: &'a [(String, FrontmatterValue)], key: &str) -> &'a FrontmatterValue {
        &fields
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("missing field {key}"))
            .1
    }

    // ── anthropic.md shape ──────────────────────────────────────────────────────

    #[test]
    fn parses_anthropic_shape_scalars() {
        let fields = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        assert_eq!(
            find(&fields, "title"),
            &FrontmatterValue::Scalar("Anthropic".into())
        );
        assert_eq!(
            find(&fields, "doc_id"),
            &FrontmatterValue::Scalar("opportunity-anthropic".into())
        );
        assert_eq!(
            find(&fields, "source"),
            &FrontmatterValue::Scalar("RESEARCH_AGENT test run (company mode)".into())
        );
    }

    #[test]
    fn parses_anthropic_shape_inline_list() {
        let fields = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        assert_eq!(
            find(&fields, "layer"),
            &FrontmatterValue::InlineList(vec!["business".into()])
        );
    }

    #[test]
    fn parses_anthropic_shape_links_block_list() {
        let fields = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        assert_eq!(
            find(&fields, "links"),
            &FrontmatterValue::BlockList(vec!["https://www.anthropic.com".into()])
        );
    }

    #[test]
    fn parses_anthropic_shape_empty_contacts_list() {
        let fields = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        assert_eq!(
            find(&fields, "contacts"),
            &FrontmatterValue::InlineList(vec![])
        );
    }

    #[test]
    fn parses_anthropic_shape_actions_map_list_with_quoted_comma_note() {
        let fields = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        assert_eq!(
            find(&fields, "actions"),
            &FrontmatterValue::MapList(vec![vec![
                ("at".to_string(), "2026-07-25".to_string()),
                ("kind".to_string(), "research".to_string()),
                ("note".to_string(), "Generated brief, cut short".to_string()),
            ]])
        );
    }

    #[test]
    fn parses_anthropic_shape_key_order_preserved() {
        let fields = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "type",
                "title",
                "description",
                "doc_id",
                "layer",
                "status",
                "kind",
                "stage",
                "source",
                "url",
                "links",
                "research_ref",
                "contacts",
                "actions",
            ]
        );
    }

    #[test]
    fn parse_serialize_parse_is_stable() {
        let first = parse_nested_frontmatter(ANTHROPIC_SHAPE).expect("must parse");
        let serialized = serialize_nested_frontmatter(&first);
        let second = parse_nested_frontmatter(&serialized).expect("must re-parse");
        assert_eq!(first, second);
    }

    // ── Error variants ───────────────────────────────────────────────────────────

    #[test]
    fn unterminated_fence_reports_open_line() {
        let content = "---\ntitle: Hello\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::UnterminatedFence { open_line: 1 })
        );
    }

    #[test]
    fn malformed_line_no_colon_reports_source_line() {
        let content = "---\ntitle: Hello\nthis is not kv\n---\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::MalformedLine { source_line: 3 })
        );
    }

    #[test]
    fn malformed_line_stray_continuation_reports_source_line() {
        // A `  - …` line with no preceding bare `key:` is malformed.
        let content = "---\n  - stray\n---\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::MalformedLine { source_line: 2 })
        );
    }

    #[test]
    fn malformed_inline_map_missing_closing_brace_reports_source_line() {
        let content = "---\nactions:\n  - { at: 2026-07-25\n---\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::MalformedInlineMap { source_line: 3 })
        );
    }

    #[test]
    fn malformed_inline_map_missing_colon_reports_source_line() {
        let content = "---\nactions:\n  - { at }\n---\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::MalformedInlineMap { source_line: 3 })
        );
    }

    #[test]
    fn unterminated_quote_in_map_value_reports_source_line() {
        let content = "---\nactions:\n  - { note: \"never closed }\n---\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::UnterminatedQuote { source_line: 3 })
        );
    }

    #[test]
    fn unterminated_quote_in_scalar_reports_source_line() {
        let content = "---\ntitle: \"never closed\n---\n";
        assert_eq!(
            parse_nested_frontmatter(content),
            Err(NestedParseError::UnterminatedQuote { source_line: 2 })
        );
    }

    // ── Misc shapes ──────────────────────────────────────────────────────────────

    #[test]
    fn bare_key_with_no_continuation_is_empty_scalar() {
        let content = "---\ntitle:\n---\n";
        let fields = parse_nested_frontmatter(content).expect("must parse");
        assert_eq!(
            find(&fields, "title"),
            &FrontmatterValue::Scalar(String::new())
        );
    }

    #[test]
    fn inline_list_with_quoted_item_containing_comma() {
        let content = "---\nkeywords: [\"has, comma\", plain]\n---\n";
        let fields = parse_nested_frontmatter(content).expect("must parse");
        assert_eq!(
            find(&fields, "keywords"),
            &FrontmatterValue::InlineList(vec!["has, comma".into(), "plain".into()])
        );
    }

    #[test]
    fn block_list_multiple_items() {
        let content = "---\nkeywords:\n  - okf\n  - frontmatter\n---\n";
        let fields = parse_nested_frontmatter(content).expect("must parse");
        assert_eq!(
            find(&fields, "keywords"),
            &FrontmatterValue::BlockList(vec!["okf".into(), "frontmatter".into()])
        );
    }
}
