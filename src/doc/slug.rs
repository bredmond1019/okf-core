// Slug derivation — the shape-agnostic `title -> doc_id`-ish identifier used by
// every `BrainDocModel` implementation (`Opportunity`, `LearningArtifact`,
// `Proposal`, …). Pure and total: never panics, and always terminates in a
// (possibly empty) lowercase, hyphen-separated string.

/// Derive a URL/filename-safe slug from a human-readable title.
///
/// Rules: lowercase every alphanumeric character (Unicode-aware, via
/// `char::to_lowercase`); collapse every run of one or more non-alphanumeric
/// characters to a single `-`; trim any leading or trailing `-`. Empty input
/// (or input with no alphanumeric characters at all) yields an empty string —
/// callers that need a non-empty slug are responsible for falling back.
pub fn derive_slug(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut pending_sep = false; // true once we've seen a separator run to flush

    for ch in title.chars() {
        if ch.is_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('-');
            }
            pending_sep = false;
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            pending_sep = true;
        }
    }

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_empty_string() {
        assert_eq!(derive_slug(""), "");
    }

    #[test]
    fn simple_title() {
        assert_eq!(derive_slug("Anthropic"), "anthropic");
    }

    #[test]
    fn punctuation_collapses_to_single_hyphen() {
        assert_eq!(derive_slug("Hello, World!!!"), "hello-world");
    }

    #[test]
    fn runs_of_separators_collapse() {
        assert_eq!(derive_slug("a   b---c___d"), "a-b-c-d");
    }

    #[test]
    fn leading_and_trailing_separators_are_trimmed() {
        assert_eq!(derive_slug("  --Hello World--  "), "hello-world");
    }

    #[test]
    fn already_slugged_input_is_unchanged() {
        assert_eq!(derive_slug("already-a-slug"), "already-a-slug");
    }

    #[test]
    fn unicode_letters_are_lowercased_and_kept() {
        assert_eq!(derive_slug("Café Münchën"), "café-münchën");
    }

    #[test]
    fn no_alphanumeric_characters_yields_empty_string() {
        assert_eq!(derive_slug("!!! --- ???"), "");
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        // A grab-bag of edge-y input: emoji, control-ish whitespace, digits.
        let _ = derive_slug("🎉42\n\t-- test_case::v2");
        assert_eq!(derive_slug("🎉42\n\t-- test_case::v2"), "42-test-case-v2");
    }
}
