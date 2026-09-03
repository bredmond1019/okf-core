//! A cross-lane message envelope — one durable, at-least-once-delivered message between
//! lanes, written under `queue/inbox/<ts>-<uuid>.json`.
//!
//! Pinned to `base-template/.claude/workflows/message.schema.json`: `message_id`, `sender`,
//! `sent_at`, `kind`, `subject`, `body`, `durable_home` and `verified_by` are all required.
//! `kind` and `durable_home.channel` are both closed vocabularies — an out-of-enum value
//! must be refused, not silently accepted.

use serde::{Deserialize, Serialize};

use super::Coord;

/// A cross-lane message envelope (`message.schema.json`), wrapped in [`Coord`] so an
/// envelope already on disk in some shape this module doesn't yet know about still
/// deserializes as [`Coord::Legacy`].
pub type Message = Coord<MessageRecord>;

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
        // task 4's known-bad fixture) — not one of the four accepted channels.
        let mut value = serde_json::to_value(full_message()).unwrap();
        value["durable_home"]["channel"] = serde_json::json!("state");
        let message: Message = serde_json::from_value(value).unwrap();
        // Falls to Legacy rather than a hard parse error (Coord<T> is untagged), but it must
        // NOT land in Typed with a bogus channel.
        assert!(message.is_legacy());
    }

    #[test]
    fn kind_rejects_values_outside_the_enum() {
        let mut value = serde_json::to_value(full_message()).unwrap();
        value["kind"] = serde_json::json!("NOTIFICATION");
        let message: Message = serde_json::from_value(value).unwrap();
        assert!(message.is_legacy());
    }
}
