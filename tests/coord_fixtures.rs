//! Cross-cutting fixture tests for the six live `coord` record kinds (OK.6.A task 2) —
//! parses the representative real records under `tests/fixtures/coord/` and confirms each
//! lands in the family (`Typed` vs `Legacy`) the fixture's filename says it should.

use std::fs;
use std::path::Path;

use okf_core::{Escalation, HeartbeatValue, Lease, Message, Registry, Slot};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/coord")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"))
}

#[test]
fn registry_claim_fixture_parses_as_typed() {
    let raw = fixture("registry-claim.json");
    let claim: Registry = serde_json::from_str(&raw).unwrap();
    assert!(!claim.is_legacy(), "expected Typed, got Legacy: {raw}");
}

#[test]
fn lease_fixture_parses_as_typed() {
    let raw = fixture("lease.json");
    let lease: Lease = serde_json::from_str(&raw).unwrap();
    assert!(!lease.is_legacy(), "expected Typed, got Legacy: {raw}");
}

#[test]
fn slot_fixture_parses_as_typed() {
    let raw = fixture("slot.json");
    let slot: Slot = serde_json::from_str(&raw).unwrap();
    assert!(!slot.is_legacy(), "expected Typed, got Legacy: {raw}");
}

#[test]
fn message_fixture_parses_as_typed() {
    let raw = fixture("message.json");
    let message: Message = serde_json::from_str(&raw).unwrap();
    assert!(!message.is_legacy(), "expected Typed, got Legacy: {raw}");
}

#[test]
fn heartbeat_iso_fixture_parses_as_iso() {
    let raw = fixture("heartbeat-iso.heartbeat");
    let value = HeartbeatValue::parse_raw(&raw);
    assert_eq!(value, HeartbeatValue::Iso(raw.trim().to_string()));
}

#[test]
fn heartbeat_epoch_fixture_parses_as_epoch() {
    let raw = fixture("heartbeat-epoch.heartbeat");
    let value = HeartbeatValue::parse_raw(&raw);
    assert!(
        matches!(value, HeartbeatValue::Epoch(_)),
        "expected Epoch, got {value:?}"
    );
}

#[test]
fn escalation_typed_fixture_lines_all_parse_as_typed() {
    let raw = fixture("escalation-typed.jsonl");
    let mut count = 0;
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let escalation: Escalation = serde_json::from_str(line).unwrap();
        assert!(
            !escalation.is_legacy(),
            "expected Typed, got Legacy: {line}"
        );
        count += 1;
    }
    assert!(count > 0, "fixture had no lines");
}

#[test]
fn escalation_legacy_fixture_lines_all_parse_as_legacy() {
    let raw = fixture("escalation-legacy.jsonl");
    let mut count = 0;
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let escalation: Escalation = serde_json::from_str(line).unwrap();
        assert!(escalation.is_legacy(), "expected Legacy, got Typed: {line}");
        count += 1;
    }
    assert!(count > 0, "fixture had no lines");
}
