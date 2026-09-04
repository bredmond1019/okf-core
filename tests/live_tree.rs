//! The live-tree lint (OK.6.A task 5): walks this machine's real coordination-artifact tree
//! and names every record that this crate's `coord` types cannot make sense of, plus a
//! per-kind count, for every kind that has one.
//!
//! `#[ignore]`d, load-bearing: the tree this walks (`.fleet-locks/`, `planning/roadmaps/*/
//! {escalations,drain-log}.jsonl`, `planning/roadmaps/*/sweeps/*.json`,
//! `planning/open-work/orchestration-runs/retros/disposal-*.json`) lives at the private HQ
//! vault root, two directories up from this crate (`core/okf-core` -> `core` ->
//! `agentic-portfolio`), and simply does not exist on a CI runner or a fresh clone. A plain
//! `cargo test` must stay hermetic and machine-independent, so this lint is never run by
//! default — only explicitly:
//!
//! ```text
//! cargo test -p okf-core --test live_tree -- --ignored --nocapture
//! ```
//!
//! CI-safety, and why no path under `planning/` appears anywhere in this file as a literal:
//! the brain root is resolved AT RUNTIME by walking up from `CARGO_MANIFEST_DIR` looking for
//! `brain.toml` (or honoring an `OKF_CORE_BRAIN_ROOT` env override), and the lint SKIPS
//! CLEANLY with a printed note — it never fails a test run — when no brain root is found.
//! `planning/` itself is a symlink into the private HQ vault, excluded from this repo's own
//! git by `base-template/.gitignore:20`; a compiled-in path under it would pass every local
//! gate and fail only in CI, where nothing reachable from a developer's machine could have
//! caught it. Every path this file touches is joined onto the runtime-resolved `brain_root`
//! variable, never written as a string literal that itself starts with `planning/`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use okf_core::{
    DrainLog, Escalation, HeartbeatValue, Lease, Message, Registry, Slot, SweepSnapshot,
};

/// Walk up from `start` looking for a directory containing `brain.toml`; honor
/// `OKF_CORE_BRAIN_ROOT` first when it is set and actually names one.
fn find_brain_root(start: &Path) -> Option<PathBuf> {
    if let Ok(root) = env::var("OKF_CORE_BRAIN_ROOT") {
        let candidate = PathBuf::from(root);
        if candidate.join("brain.toml").is_file() {
            return Some(candidate);
        }
    }
    let mut dir = Some(start.to_path_buf());
    while let Some(d) = dir {
        if d.join("brain.toml").is_file() {
            return Some(d);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

/// One kind's tally: how many records parsed strictly (`Typed`), how many fell back to
/// `Legacy` (named, with a short summary of the offending payload), and how many produced an
/// outright parse error (a closed-vocabulary refusal, e.g. `Disposal`/`Message`'s hard
/// `route`/`durable_home.channel` checks — a different, louder failure than `Legacy`, and
/// named separately so the two are never confused).
#[derive(Default)]
struct KindReport {
    typed: usize,
    legacy: Vec<(String, String)>,
    errors: Vec<(String, String)>,
}

impl KindReport {
    fn record_typed(&mut self) {
        self.typed += 1;
    }

    fn record_legacy(&mut self, location: String, payload: &serde_json::Value) {
        self.legacy.push((location, summarize_value(payload)));
    }

    fn record_error(&mut self, location: String, err: impl std::fmt::Display) {
        self.errors.push((location, err.to_string()));
    }

    fn print(&self, kind: &str) {
        println!(
            "coord::{kind} -- {} typed, {} legacy, {} error",
            self.typed,
            self.legacy.len(),
            self.errors.len()
        );
        for (location, summary) in &self.legacy {
            println!("  LEGACY {location} -- {summary}");
        }
        for (location, err) in &self.errors {
            println!("  ERROR  {location} -- {err}");
        }
    }
}

/// Truncate a JSON value's rendered form so a report line names an offending record without
/// dumping its entire body.
fn summarize_value(value: &serde_json::Value) -> String {
    let rendered = value.to_string();
    if rendered.len() > 200 {
        format!("{}...", &rendered[..200])
    } else {
        rendered
    }
}

/// The `.json` files directly inside `dir` (non-recursive), each as `(path, content)`.
/// Missing/unreadable directories yield an empty list rather than erroring — many of these
/// directories are legitimately absent (e.g. no lease has ever been taken yet).
fn json_files_in(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(content) = fs::read_to_string(&path)
        {
            out.push((path, content));
        }
    }
    out
}

/// Every `.json` file under `dir`, recursing into subdirectories (used for the message
/// queue's `<repo>/<lane>/{inbox,processing,done}/*.json` layout).
fn json_files_recursive(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    walk_json_files_recursive(dir, &mut out);
    out
}

fn walk_json_files_recursive(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_json_files_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(content) = fs::read_to_string(&path)
        {
            out.push((path, content));
        }
    }
}

/// Non-empty, trimmed lines of a `.jsonl` file, each as `(1-based line number, text)`.
/// A missing file yields an empty list.
fn jsonl_lines(path: &Path) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    if let Ok(content) = fs::read_to_string(path) {
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                out.push((idx + 1, trimmed.to_string()));
            }
        }
    }
    out
}

/// `<brain_root>/planning/roadmaps/*/<relative>` — every roadmap directory's copy of a
/// fixed relative path, for whichever of those exist. `relative` may itself contain
/// separators (e.g. `"sweeps"`).
fn each_roadmap_path(brain_root: &Path, relative: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let roadmaps_dir = brain_root.join("planning").join("roadmaps");
    let Ok(entries) = fs::read_dir(&roadmaps_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let candidate = path.join(relative);
            if candidate.exists() {
                out.push(candidate);
            }
        }
    }
    out
}

fn parse_json_records<T, F>(files: Vec<(PathBuf, String)>, report: &mut KindReport, is_legacy: F)
where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> Option<&serde_json::Value>,
{
    for (path, content) in files {
        let location = path.display().to_string();
        match serde_json::from_str::<T>(&content) {
            Ok(record) => match is_legacy(&record) {
                Some(payload) => report.record_legacy(location, payload),
                None => report.record_typed(),
            },
            Err(e) => report.record_error(location, e),
        }
    }
}

#[test]
#[ignore]
fn names_every_legacy_coordination_record_on_this_machine() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let brain_root = match find_brain_root(&manifest_dir) {
        Some(root) => root,
        None => {
            println!(
                "live-tree lint: no brain.toml found walking up from {} (and \
                 OKF_CORE_BRAIN_ROOT is unset or does not name one) -- skipping cleanly, \
                 this machine has no HQ vault to lint",
                manifest_dir.display()
            );
            return;
        }
    };
    println!(
        "live-tree lint: brain root resolved to {}",
        brain_root.display()
    );

    let fleet_locks = brain_root.join(".fleet-locks");
    let mut total_legacy = 0usize;
    let mut total_errors = 0usize;

    // --- Registry claims: .fleet-locks/lane-agents/*.json ---
    let mut registry_report = KindReport::default();
    parse_json_records::<Registry, _>(
        json_files_in(&fleet_locks.join("lane-agents")),
        &mut registry_report,
        Registry::legacy_value,
    );
    registry_report.print("registry");
    total_legacy += registry_report.legacy.len();
    total_errors += registry_report.errors.len();

    // --- Leases: .fleet-locks/leases/*.json ---
    let mut lease_report = KindReport::default();
    parse_json_records::<Lease, _>(
        json_files_in(&fleet_locks.join("leases")),
        &mut lease_report,
        Lease::legacy_value,
    );
    lease_report.print("lease");
    total_legacy += lease_report.legacy.len();
    total_errors += lease_report.errors.len();

    // --- Slots: .fleet-locks/*.json (top level only -- the concurrency-check registrations,
    // not the subdirectories, which are their own kinds) ---
    let mut slot_report = KindReport::default();
    parse_json_records::<Slot, _>(
        json_files_in(&fleet_locks),
        &mut slot_report,
        Slot::legacy_value,
    );
    slot_report.print("slot");
    total_legacy += slot_report.legacy.len();
    total_errors += slot_report.errors.len();

    // --- Messages: .fleet-locks/queue/**/{inbox,processing,done}/*.json ---
    let mut message_report = KindReport::default();
    parse_json_records::<Message, _>(
        json_files_recursive(&fleet_locks.join("queue")),
        &mut message_report,
        Message::legacy_value,
    );
    message_report.print("message");
    total_legacy += message_report.legacy.len();
    total_errors += message_report.errors.len();

    // --- Commander heartbeats: .fleet-locks/commander-heartbeats/*.heartbeat ---
    // HeartbeatValue::parse_raw never errors (see coord::heartbeat's module doc) -- any
    // input that isn't a bare integer is treated as the ISO-string form, so this kind has no
    // Coord<T> wrapper and never lands in Legacy. Every file is still named here, split by
    // its parsed form, so the epoch/ISO divergence this block exists to surface is visible in
    // the report even though neither form is a parse failure.
    let heartbeats_dir = fleet_locks.join("commander-heartbeats");
    let mut heartbeat_forms: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    if let Ok(entries) = fs::read_dir(&heartbeats_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("heartbeat") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let form = match HeartbeatValue::parse_raw(&raw) {
                HeartbeatValue::Epoch(_) => "epoch",
                HeartbeatValue::Iso(_) => "iso",
            };
            heartbeat_forms
                .entry(form)
                .or_default()
                .push(path.display().to_string());
        }
    }
    println!(
        "coord::heartbeat -- {} epoch-format, {} iso-format (neither is Legacy -- see \
         coord::heartbeat's module doc)",
        heartbeat_forms.get("epoch").map(Vec::len).unwrap_or(0),
        heartbeat_forms.get("iso").map(Vec::len).unwrap_or(0)
    );
    for (form, paths) in &heartbeat_forms {
        for path in paths {
            println!("  {form} {path}");
        }
    }

    // --- Escalations: planning/roadmaps/*/escalations.jsonl ---
    let mut escalation_report = KindReport::default();
    for jsonl_path in each_roadmap_path(&brain_root, "escalations.jsonl") {
        for (line_no, line) in jsonl_lines(&jsonl_path) {
            let location = format!("{}:{}", jsonl_path.display(), line_no);
            match serde_json::from_str::<Escalation>(&line) {
                Ok(record) => match record.legacy_value() {
                    Some(payload) => escalation_report.record_legacy(location, payload),
                    None => escalation_report.record_typed(),
                },
                Err(e) => escalation_report.record_error(location, e),
            }
        }
    }
    escalation_report.print("escalation");
    total_legacy += escalation_report.legacy.len();
    total_errors += escalation_report.errors.len();

    // --- Drain-log rows: planning/roadmaps/*/drain-log.jsonl ---
    let mut drain_log_report = KindReport::default();
    for jsonl_path in each_roadmap_path(&brain_root, "drain-log.jsonl") {
        for (line_no, line) in jsonl_lines(&jsonl_path) {
            let location = format!("{}:{}", jsonl_path.display(), line_no);
            match serde_json::from_str::<DrainLog>(&line) {
                Ok(record) => match record.legacy_value() {
                    Some(payload) => drain_log_report.record_legacy(location, payload),
                    None => drain_log_report.record_typed(),
                },
                Err(e) => drain_log_report.record_error(location, e),
            }
        }
    }
    drain_log_report.print("drain_log");
    total_legacy += drain_log_report.legacy.len();
    total_errors += drain_log_report.errors.len();

    // --- Sweep snapshots: planning/roadmaps/*/sweeps/*.json ---
    let mut sweep_report = KindReport::default();
    for sweeps_dir in each_roadmap_path(&brain_root, "sweeps") {
        parse_json_records::<SweepSnapshot, _>(
            json_files_in(&sweeps_dir),
            &mut sweep_report,
            SweepSnapshot::legacy_value,
        );
    }
    sweep_report.print("sweep_snapshot");
    total_legacy += sweep_report.legacy.len();
    total_errors += sweep_report.errors.len();

    // --- Disposal files: planning/open-work/orchestration-runs/retros/disposal-*.json ---
    let mut disposal_report = KindReport::default();
    let retros_dir = brain_root
        .join("planning")
        .join("open-work")
        .join("orchestration-runs")
        .join("retros");
    for (path, content) in json_files_in(&retros_dir) {
        let name_matches = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("disposal-") && n.ends_with(".json"));
        if !name_matches {
            continue;
        }
        let location = path.display().to_string();
        match serde_json::from_str::<okf_core::Disposal>(&content) {
            Ok(record) => match record.legacy_value() {
                Some(payload) => disposal_report.record_legacy(location, payload),
                None => disposal_report.record_typed(),
            },
            Err(e) => disposal_report.record_error(location, e),
        }
    }
    disposal_report.print("disposal");
    total_legacy += disposal_report.legacy.len();
    total_errors += disposal_report.errors.len();

    println!(
        "live-tree lint: TOTAL {total_legacy} legacy record(s), {total_errors} hard parse \
         error(s) across every kind walked"
    );

    // Not an assertion of zero -- see this task's acceptance criteria and the block's own
    // notes: the count reaching zero depends on HQ.12.A's migration and BT.8.C's writer fix
    // landing in other repos, and is the roadmap's Wave 1 exit criterion, not this block's.
    // This lint's job is only to name what it finds; it never fails the run on a non-zero
    // count.
}
