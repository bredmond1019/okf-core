// Markdown parser for the brain's authored `docs/state/state-schema.md` —
// extracts the PIPE TABLES only, never the ```json template fences (see
// `tasks.md` decision 1: the `tracks[].blocks[]` fence omits `note`, so a
// fence-driven check would have been green throughout the live data-loss
// bug this block exists to catch).
//
// Test-only I/O (AGENT.md: okf-core's `src/` is a pure leaf library with no
// I/O and no path deps; the doc-parsing needed for this conformance gate
// lives exclusively here, under `tests/`).
//
// Uses only `std` — no new `[dependencies]` or `[dev-dependencies]` entry.

use std::collections::BTreeSet;

/// One row of a `| Field | Shape | Meaning |` table, tagged with the `## `
/// section heading it was found under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentedField {
    pub name: String,
    pub shape: String,
    pub section: String,
}

/// Trim a table cell of surrounding whitespace and backticks (field names
/// and shapes are always backtick- or plain-formatted in the doc, e.g.
/// `` `note` `` or `string?`).
fn clean_cell(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().to_string()
}

/// Split a `|`-delimited table row into its cell contents, dropping the
/// leading/trailing empty cells produced by the row's outer `|` fences.
fn split_row(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    inner.split('|').collect()
}

/// A separator row looks like `|---|---|---|` (any number of `-`/`:` per
/// cell) — distinguish it from a data row so we don't misparse it as one.
fn is_separator_row(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return false;
    }
    split_row(trimmed)
        .iter()
        .all(|cell| !cell.trim().is_empty() && cell.trim().chars().all(|c| c == '-' || c == ':'))
}

/// Walk `doc` line by line, tracking the current `## ` heading and fenced
/// code-block state, and collect every `| Field | Shape | Meaning |` table
/// row into a `DocumentedField`. Fenced code blocks (```...```) are skipped
/// entirely so no table-looking line inside a JSON template fence is ever
/// consumed — the templates are deliberately out of scope (decision 1).
pub fn parse_field_tables(doc: &str) -> Vec<DocumentedField> {
    let mut fields = Vec::new();
    let mut current_section = String::new();
    let mut in_fence = false;
    let mut in_field_table = false;

    let mut lines = doc.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            in_field_table = false;
            continue;
        }
        if in_fence {
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("## ") {
            current_section = heading.trim().to_string();
            in_field_table = false;
            continue;
        }

        if in_field_table {
            if trimmed.starts_with('|') && !is_separator_row(trimmed) {
                let cells = split_row(trimmed);
                if cells.len() >= 2 {
                    fields.push(DocumentedField {
                        name: clean_cell(cells[0]),
                        shape: clean_cell(cells[1]),
                        section: current_section.clone(),
                    });
                }
                continue;
            } else {
                in_field_table = false;
                // fall through: this line might itself start a new table
            }
        }

        if trimmed == "| Field | Shape | Meaning |" {
            in_field_table = true;
            // Consume the following `|---|` separator row, if present.
            if let Some(next) = lines.peek()
                && is_separator_row(next.trim())
            {
                lines.next();
            }
        }
    }

    fields
}

/// Normalise a dotted/bracketed doc path to its trailing field name, e.g.
/// `tracks[].blocks[].tasks` -> `tasks`, `focus` -> `focus`.
fn trailing_field_name(path: &str) -> String {
    let without_brackets = path.replace("[]", "");
    without_brackets
        .rsplit('.')
        .next()
        .unwrap_or(&without_brackets)
        .trim()
        .to_string()
}

/// Extract every backtick-delimited identifier from a table cell, e.g.
/// `` `focus`, `tracks[].blocks[].tasks`, brain `repos[]` `` yields
/// `["focus", "tracks[].blocks[].tasks", "repos[]"]`.
fn backticked_identifiers(cell: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = cell.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if c == '`'
            && let Some(end) = cell[start + 1..].find('`')
        {
            let ident = &cell[start + 1..start + 1 + end];
            out.push(ident.to_string());
            // Skip past the closing backtick.
            while let Some((idx, _)) = chars.peek().copied() {
                if idx <= start + 1 + end {
                    chars.next();
                } else {
                    break;
                }
            }
        }
    }
    out
}

/// Parse the `## Authored vs derived` section's `| Bucket | Fields | Who
/// writes it |` table and return the set of bare field names in the row
/// whose Bucket cell contains "Derived", normalised via
/// [`trailing_field_name`] so the set is comparable to bare struct/doc
/// field names.
pub fn parse_derived_fields(doc: &str) -> BTreeSet<String> {
    let mut derived = BTreeSet::new();
    let mut current_section = String::new();
    let mut in_fence = false;
    let mut in_bucket_table = false;

    let mut lines = doc.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            in_bucket_table = false;
            continue;
        }
        if in_fence {
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("## ") {
            current_section = heading.trim().to_string();
            in_bucket_table = false;
            continue;
        }

        if in_bucket_table {
            if trimmed.starts_with('|') && !is_separator_row(trimmed) {
                let cells = split_row(trimmed);
                if cells.len() >= 2 && cells[0].contains("Derived") {
                    for ident in backticked_identifiers(cells[1]) {
                        derived.insert(trailing_field_name(&ident));
                    }
                }
                continue;
            } else {
                in_bucket_table = false;
            }
        }

        if current_section == "Authored vs derived"
            && trimmed == "| Bucket | Fields | Who writes it |"
        {
            in_bucket_table = true;
            if let Some(next) = lines.peek()
                && is_separator_row(next.trim())
            {
                lines.next();
            }
        }
    }

    derived
}

/// Whether `field` is documented as derived (i.e. present in the set
/// returned by [`parse_derived_fields`]).
pub fn is_derived(field: &str, derived: &BTreeSet<String>) -> bool {
    derived.contains(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::schema_doc::read_schema_doc;

    #[test]
    fn finds_block_vocabulary_section() {
        let doc = read_schema_doc();
        let fields = parse_field_tables(&doc);
        assert!(
            fields
                .iter()
                .any(|f| f.section.starts_with("Block vocabulary")),
            "expected a field parsed under a `Block vocabulary` section"
        );
    }

    #[test]
    fn block_vocabulary_contains_note_and_description() {
        let doc = read_schema_doc();
        let fields = parse_field_tables(&doc);
        let block_vocab_names: BTreeSet<&str> = fields
            .iter()
            .filter(|f| f.section.starts_with("Block vocabulary"))
            .map(|f| f.name.as_str())
            .collect();

        // Regression guard for decision 1: `note` and `description` exist
        // only in the pipe table, not in the JSON template fence — if the
        // parser were reading the fence instead, these would be missing.
        assert!(
            block_vocab_names.contains("note"),
            "expected `note` in Block vocabulary pipe table fields: {block_vocab_names:?}"
        );
        assert!(
            block_vocab_names.contains("description"),
            "expected `description` in Block vocabulary pipe table fields: {block_vocab_names:?}"
        );
    }

    #[test]
    fn derived_fields_contains_tasks_and_focus() {
        let doc = read_schema_doc();
        let derived = parse_derived_fields(&doc);
        assert!(
            derived.contains("tasks"),
            "expected `tasks` in derived field set: {derived:?}"
        );
        assert!(
            derived.contains("focus"),
            "expected `focus` in derived field set: {derived:?}"
        );
    }

    #[test]
    fn is_derived_reflects_the_set() {
        let mut derived = BTreeSet::new();
        derived.insert("tasks".to_string());
        assert!(is_derived("tasks", &derived));
        assert!(!is_derived("note", &derived));
    }

    #[test]
    fn skips_json_fences() {
        let doc = "\
## Fenced example
```json
| Field | Shape | Meaning |
|---|---|---|
| `fenced_field` | string | should not be parsed |
```
";
        let fields = parse_field_tables(doc);
        assert!(
            fields.is_empty(),
            "expected no fields parsed from inside a fence, got {fields:?}"
        );
    }
}
