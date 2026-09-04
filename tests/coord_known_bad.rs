//! Known-bad fixtures for OK.6.A task 4: values that violate a closed vocabulary this crate
//! DOES type must be REFUSED with a hard parse error — never silently accepted, and never
//! swallowed into `Coord::Legacy`. `Legacy` is reserved for shapes this module does not yet
//! know about at all (an older writer's format, a field this crate hasn't modeled); it is
//! not a catch-all for a value that violates a vocabulary we deliberately closed.
//!
//! The two required refusals (per the OK.6.A block record's task 4):
//! - a `disposal.json` row whose `route` is outside [`okf_core::DisposalRoute`]'s enum;
//! - a `message` whose `durable_home.channel` is outside `message.schema.json`'s enum
//!   (`lane-log`, `state-edge`, `carryover`, `run-record`) — using the real `"state"` value,
//!   the exact defect a live envelope hit (see the FINDING referenced in the fixture body).
//!
//! Both fixtures were run against the parser BEFORE the refusal landed and observed to pass
//! (fall to `Legacy`) when they should not have; that `observed_red` evidence is recorded in
//! `planning/orchestration-run/coordination-layer-port/notes.md`, not re-derived here.

use std::fs;
use std::path::Path;

use okf_core::{Disposal, Message};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/coord/known-bad")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"))
}

#[test]
fn disposal_row_with_out_of_enum_route_is_refused() {
    let raw = fixture("disposal-bad-route.json");
    let result = serde_json::from_str::<Disposal>(&raw);
    assert!(
        result.is_err(),
        "expected a disposal row with an out-of-enum `route` to be refused with a parse \
         error, got: {result:?}"
    );
}

#[test]
fn message_with_out_of_enum_durable_home_channel_is_refused() {
    let raw = fixture("message-bad-channel.json");
    let result = serde_json::from_str::<Message>(&raw);
    assert!(
        result.is_err(),
        "expected a message with durable_home.channel outside the closed vocabulary to be \
         refused with a parse error, got: {result:?}"
    );
}
