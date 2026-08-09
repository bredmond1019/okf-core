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
use std::sync::Mutex;

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
    read_schema_doc_at(&locate_schema_doc())
}

/// Read the schema doc from an explicit `path`, bypassing discovery and the
/// `OKF_STATE_SCHEMA_DOC` env var entirely.
///
/// This is the seam fixture-driven tests must use instead of mutating
/// `OKF_STATE_SCHEMA_DOC`: env vars are process-global and `cargo test` runs
/// tests as threads within one process, so a fixture that sets the override
/// var races every sibling test reading the same var concurrently. Naming
/// the fixture's path directly sidesteps that race instead of trying to
/// serialize around it.
///
/// Preserves the same fail-closed behaviour as `read_schema_doc`: a missing
/// or unreadable path panics with an actionable message rather than
/// silently skipping the check it backs.
pub fn read_schema_doc_at(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| {
        panic!(
            "could not read schema doc fixture at {}: {err}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes the tests below that touch the process-global
    /// `OVERRIDE_ENV_VAR`: `cargo test` runs tests as threads within one
    /// process, so a test that sets/removes the var and one that asserts
    /// it is unset race each other without this lock. Poisoning is
    /// ignored (`unwrap_or_else(|e| e.into_inner())`) so one failing test
    /// under the lock does not spuriously fail every later test that
    /// acquires it.
    static ENV_VAR_TEST_LOCK: Mutex<()> = Mutex::new(());

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
    fn read_schema_doc_at_reads_an_explicit_path_without_touching_env() {
        // Confirm the seam works without setting OKF_STATE_SCHEMA_DOC at
        // all — this is what fixture tests must use to avoid racing
        // sibling tests over the process-global env var.
        let _guard = ENV_VAR_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(
            env::var(OVERRIDE_ENV_VAR).is_err(),
            "override env var must not be set entering this test"
        );
        let real_path = locate_schema_doc();
        let content = read_schema_doc_at(&real_path);
        assert!(!content.is_empty());
        assert!(content.contains("Block vocabulary"));
        assert!(
            env::var(OVERRIDE_ENV_VAR).is_err(),
            "read_schema_doc_at must not set the override env var as a side effect"
        );
    }

    #[test]
    fn read_schema_doc_at_panics_on_missing_path() {
        let result = std::panic::catch_unwind(|| {
            read_schema_doc_at(Path::new("/nonexistent/does-not-exist-schema.md"))
        });
        assert!(
            result.is_err(),
            "read_schema_doc_at must panic (fail closed) on an unreadable path"
        );
    }

    #[test]
    fn override_env_var_is_used_verbatim() {
        // Set the override to the file we already know exists via normal
        // discovery, and confirm it is honoured verbatim (not re-searched).
        let _guard = ENV_VAR_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let real_path = locate_schema_doc();
        // SAFETY: guarded by ENV_VAR_TEST_LOCK above, so no other test in
        // this binary reads or writes OKF_STATE_SCHEMA_DOC concurrently.
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
