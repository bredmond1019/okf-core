//! A cross-lane message envelope — one durable, at-least-once-delivered message between
//! lanes, written under `queue/inbox/<ts>-<uuid>.json`.
//!
//! Pinned to `base-template/.claude/workflows/message.schema.json`: `message_id`, `sender`,
//! `sent_at`, `kind`, `subject`, `body`, `durable_home` and `verified_by` are all required.
//! `kind` and `durable_home.channel` are both closed vocabularies — an out-of-enum value
//! must be refused, not silently accepted.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

use super::Coord;

/// A cross-lane message envelope (`message.schema.json`), wrapped in [`Coord`] so an
/// envelope already on disk in some shape this module doesn't yet know about still
/// deserializes as [`Coord::Legacy`].
///
/// **Not** a plain `Coord<MessageRecord>` type alias: `durable_home.channel` is a closed
/// vocabulary this module DOES type ([`DurableHomeChannel`]), and a value outside it must be
/// REFUSED with a hard parse error, not swallowed into `Legacy` — the exact defect a live
/// envelope hit (`durable_home.channel: "state"`; see `check_messages.py`'s FINDING). As with
/// [`crate::Disposal`], `Coord<T>`'s `#[serde(untagged)]` derive can't tell "unknown shape"
/// from "known field, disallowed value" apart on its own, so this wrapper inspects the raw
/// JSON before attempting the generic `Coord` parse. (OK.6.A task 4.)
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Message(Coord<MessageRecord>);

impl Message {
    /// True if this record fell back to the untyped [`Coord::Legacy`] escape.
    pub fn is_legacy(&self) -> bool {
        self.0.is_legacy()
    }

    /// The strict typed value, if this record parsed into one.
    pub fn typed(&self) -> Option<&MessageRecord> {
        self.0.typed()
    }

    /// The raw JSON of a legacy record, if this record fell back to [`Coord::Legacy`].
    pub fn legacy_value(&self) -> Option<&serde_json::Value> {
        self.0.legacy_value()
    }
}

/// The `durable_home.channel` values [`DurableHomeChannel`] accepts on the wire, per
/// `message.schema.json`.
const VALID_DURABLE_HOME_CHANNELS: [&str; 4] =
    ["lane-log", "state-edge", "carryover", "run-record"];

/// Returns the message's `durable_home.channel` when present as a string outside
/// [`VALID_DURABLE_HOME_CHANNELS`]. Returns `None` when the field is absent, not a string, or
/// already in the closed vocabulary — those cases are left to the generic [`Coord`] parse
/// (and its `Legacy` fallback) to sort out.
fn out_of_enum_durable_home_channel(value: &serde_json::Value) -> Option<&str> {
    let channel = value.get("durable_home")?.get("channel")?.as_str()?;
    (!VALID_DURABLE_HOME_CHANNELS.contains(&channel)).then_some(channel)
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(bad_channel) = out_of_enum_durable_home_channel(&value) {
            return Err(de::Error::custom(format!(
                "message durable_home.channel {bad_channel:?} is not one of the closed \
                 DurableHomeChannel values {VALID_DURABLE_HOME_CHANNELS:?}"
            )));
        }
        let coord: Coord<MessageRecord> =
            serde_json::from_value(value).map_err(de::Error::custom)?;
        Ok(Message(coord))
    }
}

/// The five message kinds, each derived from a measured incident, no others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageKind {
    /// A dependency edge is now clear on the sender's side.
    EdgeReleased,
    /// A cross-lane observation worth relaying.
    Finding,
    /// A need for two lanes to synchronize before either proceeds.
    Rendezvous,
    /// Taking or releasing a repo lease (`lease.schema.json`), discriminated in `body`.
    LeaseRelease,
    /// A read-only question that implies no action by the receiver.
    Query,
}

/// Which durable-record kind holds the item a message is signalling about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DurableHomeChannel {
    LaneLog,
    StateEdge,
    Carryover,
    RunRecord,
}

/// The BT.6.A registry key identifying who sent a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageSender {
    /// The `ListAgents` nickname the sender was reachable at when this message was sent.
    pub agent_name: String,
    /// Repo slug the sending lane is driving.
    pub repo: String,
    /// The sending lane's name/slug.
    pub lane: String,
    /// Slug of the roadmap directory the sending lane runs under.
    pub roadmap: String,
}

/// What a message is about: the subject block or repo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageSubject {
    /// Repo slug this message concerns.
    pub repo: String,
    /// OPTIONAL block id this message concerns, when the message is about a specific block
    /// rather than the repo as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
}

/// Where the item a message is about was already, or is about to be, written to disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDurableHome {
    /// Which durable-record kind holds this item.
    pub channel: DurableHomeChannel,
    /// A locator within that channel precise enough for a receiver to go find the durable
    /// item itself.
    #[serde(rename = "ref")]
    pub reference: String,
}

/// The strict, current shape of a cross-lane message envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageRecord {
    /// The uuid half of the message's filename stem.
    pub message_id: String,
    /// Who sent this message.
    pub sender: MessageSender,
    /// ISO-8601 timestamp with timezone marking when this message was written to the inbox.
    pub sent_at: String,
    /// Which of the five kinds this message is.
    pub kind: MessageKind,
    /// What this message is about.
    pub subject: MessageSubject,
    /// The message content itself.
    pub body: String,
    /// Where the durable record behind this message already lives.
    pub durable_home: MessageDurableHome,
    /// Evidence backing this envelope's claim.
    pub verified_by: String,
    /// OPTIONAL. Which physical/logical host wrote this record — not part of
    /// `message.schema.json` today; added uniformly across every `coord` record kind per the
    /// OK.6.A block's `what`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_message() -> MessageRecord {
        MessageRecord {
            message_id: "70ef6ce8-abcd-4e21-9f10-0000000000aa".to_string(),
            sender: MessageSender {
                agent_name: "base-template-4c".to_string(),
                repo: "base-template".to_string(),
                lane: "types".to_string(),
                roadmap: "coordination-layer-port".to_string(),
            },
            sent_at: "2026-08-28T02:08:00Z".to_string(),
            kind: MessageKind::EdgeReleased,
            subject: MessageSubject {
                repo: "bastion".to_string(),
                block: Some("BA.21.A".to_string()),
            },
            body: "bastion:BA.21.A is now unblocked on the engine side.".to_string(),
            durable_home: MessageDurableHome {
                channel: DurableHomeChannel::StateEdge,
                reference: "bastion/planning/state.json#BA.21.A".to_string(),
            },
            verified_by: "UNVERIFIED: engine-rs-31".to_string(),
            host: Some("brain-mini".to_string()),
        }
    }

    #[test]
    fn full_message_round_trips() {
        let message = full_message();
        let json = serde_json::to_string(&message).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_legacy());
        assert_eq!(parsed.typed(), Some(&message));
    }

    #[test]
    fn minimal_message_omits_optional_fields() {
        let mut message = full_message();
        message.subject.block = None;
        message.host = None;
        let json = serde_json::to_string(&message).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(!value["subject"].as_object().unwrap().contains_key("block"));
        assert!(!value.as_object().unwrap().contains_key("host"));

        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.typed(), Some(&message));
    }

    #[test]
    fn kind_enum_uses_screaming_snake_case_wire_values() {
        assert_eq!(
            serde_json::to_string(&MessageKind::EdgeReleased).unwrap(),
            "\"EDGE_RELEASED\""
        );
        assert_eq!(
            serde_json::to_string(&MessageKind::Finding).unwrap(),
            "\"FINDING\""
        );
        assert_eq!(
            serde_json::to_string(&MessageKind::Rendezvous).unwrap(),
            "\"RENDEZVOUS\""
        );
        assert_eq!(
            serde_json::to_string(&MessageKind::LeaseRelease).unwrap(),
            "\"LEASE_RELEASE\""
        );
        assert_eq!(
            serde_json::to_string(&MessageKind::Query).unwrap(),
            "\"QUERY\""
        );
    }

    #[test]
    fn durable_home_channel_rejects_values_outside_the_enum() {
        // "state" is the exact defect a live envelope hit (see the FINDING referenced in
        // task 4's known-bad fixture) — not one of the four accepted channels. A closed
        // vocabulary we DO type must be refused with a hard parse error, never accepted and
        // never swallowed into Legacy (OK.6.A task 4).
        let mut value = serde_json::to_value(full_message()).unwrap();
        value["durable_home"]["channel"] = serde_json::json!("state");
        let result: Result<Message, _> = serde_json::from_value(value);
        assert!(
            result.is_err(),
            "expected an out-of-enum durable_home.channel to be refused, got: {result:?}"
        );
    }

    #[test]
    fn kind_rejects_values_outside_the_enum() {
        let mut value = serde_json::to_value(full_message()).unwrap();
        value["kind"] = serde_json::json!("NOTIFICATION");
        let message: Message = serde_json::from_value(value).unwrap();
        assert!(message.is_legacy());
    }
}
