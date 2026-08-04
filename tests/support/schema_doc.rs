// Locates the brain's authored `docs/state/state-schema.md` doc from
// wherever this crate happens to be built — the main tree
// (`core/okf-core`) or an SDLC worktree (`core/okf-core/trees/<name>/`).
//
// Test-only I/O (AGENT.md: okf-core's `src/` is a pure leaf library with no
// I/O and no path deps; the doc-reading needed for this conformance gate
// lives exclusively here, under `tests/`).
//
// Uses only `std` — no new `[dependencies]` or `[dev-dependencies]` entry.

use std::env;
use std::path::{Path, PathBuf};

/// Relative path, from the monorepo root, to the authored schema doc.
const SCHEMA_DOC_RELATIVE: &str = "docs/state/state-schema.md";

/// Env var that lets a non-standard checkout (e.g. a standalone clone of
/// okf-core outside the `agentic-portfolio` monorepo workspace) override
/// doc discovery explicitly.
const OVERRIDE_ENV_VAR: &str = "OKF_STATE_SCHEMA_DOC";

/// Resolve the path to the brain's `docs/state/state-schema.md`.
///
/// Resolution order:
/// 1. If `OKF_STATE_SCHEMA_DOC` is set, use it verbatim.
/// 2. Otherwise start at `CARGO_MANIFEST_DIR` and ascend parent
///    directories, returning the first `<ancestor>/docs/state/state-schema.md`
///    that exists. This is what makes the search worktree-safe: it
///    resolves at depth 2 from the main tree (`core/okf-core`) and at
///    depth 4 from a worktree (`core/okf-core/trees/<name>/`) with no
///    hardcoded depth or `../../` literal.
///
/// Fails closed: if the doc cannot be found by the filesystem root, this
/// panics with an actionable message rather than silently skipping the
/// check it backs — a silently-skipping gate is exactly the failure mode
/// this block exists to eliminate.
pub fn locate_schema_doc() -> PathBuf {
    if let Ok(override_path) = env::var(OVERRIDE_ENV_VAR) {
        return PathBuf::from(override_path);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut ancestor: &Path = manifest_dir.as_path();

    loop {
        let candidate = ancestor.join(SCHEMA_DOC_RELATIVE);
        if candidate.is_file() {
            return candidate;
        }
        match ancestor.parent() {
            Some(parent) => ancestor = parent,
            None => break,
        }
    }

    panic!(
        "could not locate `{SCHEMA_DOC_RELATIVE}` by ascending from \
         CARGO_MANIFEST_DIR ({}); this test requires the brain's authored \
         schema doc. If this is a standalone checkout of okf-core outside \
         the `agentic-portfolio` monorepo workspace, set the `{OVERRIDE_ENV_VAR}` \
         env var to the doc's path explicitly.",
        manifest_dir.display()
    );
}

/// Locate + read the schema doc to a `String`.
pub fn read_schema_doc() -> String {
    let path = locate_schema_doc();
    std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "found `{SCHEMA_DOC_RELATIVE}` at {} but could not read it: {err}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_schema_doc_finds_the_file() {
        let path = locate_schema_doc();
        assert!(
            path.is_file(),
            "locate_schema_doc returned a path that does not exist: {}",
            path.display()
        );
        assert!(path.ends_with(SCHEMA_DOC_RELATIVE));
    }

    #[test]
    fn read_schema_doc_returns_non_empty_content() {
        let content = read_schema_doc();
        assert!(!content.is_empty());
        assert!(content.contains("Block vocabulary"));
    }

    #[test]
    fn override_env_var_is_used_verbatim() {
        // Set the override to the file we already know exists via normal
        // discovery, and confirm it is honoured verbatim (not re-searched).
        let real_path = locate_schema_doc();
        // SAFETY: test-only, single-threaded within this process for this
        // var; no other test in this binary reads OKF_STATE_SCHEMA_DOC
        // concurrently.
        unsafe {
            env::set_var(OVERRIDE_ENV_VAR, &real_path);
        }
        let resolved = locate_schema_doc();
        unsafe {
            env::remove_var(OVERRIDE_ENV_VAR);
        }
        assert_eq!(resolved, real_path);
    }
}
