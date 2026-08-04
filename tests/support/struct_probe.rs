// Struct-side probe for the schema/struct conformance gate
// (`3.1-schema-struct-conformance`). Determines whether a given okf-core
// struct carries a field of a given name — WITHOUT reflection and without a
// new dependency, via a round-trip through `serde_json`.
//
// Technique: build a minimal seed JSON object (the caller supplies the
// fields the target struct needs to deserialize at all), add the field
// under test set to a synthetic value derived from the doc's Shape column,
// deserialize into `T`, then re-serialize back to `serde_json::Value`. If
// `T` has no matching field, serde silently drops the unknown key on
// deserialize and it is therefore absent after re-serialization — this is
// EXACTLY the historical `TrackBlock.note` data-loss mechanism (see
// tasks.md), so the probe tests the real failure mode rather than a proxy
// for it.
//
// Test-only I/O-free helper (uses `serde`/`serde_json`, both already
// `okf-core` dependencies; no new `[dependencies]`/`[dev-dependencies]`
// entry, and this lives under `tests/`, never `src/`).

use serde_json::{Value, json};

/// Implemented by every authored struct that carries an `extra` capture map
/// (`#[serde(flatten, default)] pub extra: serde_json::Map<String, Value>`).
///
/// Needed because the capture map changes what "round-trips" means for this
/// probe: with `extra` present, an UNMODELED field also survives a
/// deserialize→serialize round-trip (it rides along in `extra`), so
/// round-tripping alone can no longer distinguish a real typed field from a
/// captured one — every documented field would report PRESENT and
/// `schema_struct_conformance` would pass vacuously. Checking
/// `parsed.extra().contains_key(field)` after the round-trip restores that
/// distinction: a field that landed in the capture map has no typed field.
pub trait HasExtra {
    fn extra(&self) -> &serde_json::Map<String, Value>;
}

/// Map a doc Shape column value to a plausible synthetic JSON value.
/// Shapes carry a trailing `?` for "optional" — stripped before matching.
///
/// - `integer` -> `1`
/// - `array` (or anything containing `array`/`[]`) -> `["x"]`
/// - `object` (or anything containing `object`/`{}`) -> `{}`
/// - anything else (`enum`, `string`, free text) -> `"x"`
///
/// Array shapes deliberately use a NON-empty array (`["x"]`), not `[]`: many
/// `Vec` fields in this crate carry `skip_serializing_if = "Vec::is_empty"`
/// (e.g. `TrackBlock.epics`), so an empty synthetic array would round-trip
/// through a real field and still vanish from the re-serialized output —
/// the exact false-"absent" this probe must not produce. A non-empty array
/// either round-trips visibly (proving presence) or fails to deserialize
/// against a differently-shaped element type, which the caller already
/// treats as PRESENT (a type conflict proves the field exists).
fn synthetic_value(shape: &str) -> Value {
    let shape = shape.trim().trim_end_matches('?').trim();
    if shape.contains("integer") {
        json!(1)
    } else if shape.contains("array") || shape.contains("[]") {
        json!(["x"])
    } else if shape.contains("object") || shape.contains("{}") {
        json!({})
    } else {
        json!("x")
    }
}

/// Whether struct `T` carries a field named `field`.
///
/// `seed` is a minimal valid JSON object for `T` (the fields `T` requires to
/// deserialize at all, supplied by the caller — each struct under check
/// needs a different minimal seed, e.g. `TrackBlock` needs `id` + `title`).
///
/// A field with no struct counterpart is dropped by serde on deserialize
/// and is therefore absent from the re-serialized output — that is the
/// "absent" case. If the synthetic value instead makes deserialization fail
/// outright (a type conflict against a real typed field), that is treated
/// as PRESENT, not absent: a type conflict proves the field exists on `T`,
/// it's just typed differently than our synthetic guess.
pub fn struct_has_field<T>(seed: Value, field: &str, shape: &str) -> bool
where
    T: serde::Serialize + serde::de::DeserializeOwned + HasExtra,
{
    let mut obj = match seed {
        Value::Object(map) => map,
        other => panic!("struct_has_field seed must be a JSON object, got {other:?}"),
    };
    obj.insert(field.to_string(), synthetic_value(shape));
    let probe = Value::Object(obj);

    let parsed: T = match serde_json::from_value(probe) {
        Ok(v) => v,
        // A type conflict against a real typed field proves the field
        // exists — treat deserialize failure as PRESENT.
        Err(_) => return true,
    };

    // With the whole-object capture map, an unmodeled field ALSO survives a
    // round-trip (it rides along in `extra`), so round-tripping alone can no
    // longer tell a typed field from a captured one. If the probed name
    // landed in `extra`, the struct has NO typed field for it — report
    // ABSENT regardless of what the re-serialized JSON contains.
    if parsed.extra().contains_key(field) {
        return false;
    }

    let round_tripped =
        serde_json::to_value(&parsed).expect("re-serializing a successfully-deserialized value");

    match round_tripped {
        Value::Object(map) => map.contains_key(field),
        other => panic!("expected struct to serialize to a JSON object, got {other:?}"),
    }
}

impl HasExtra for okf_core::StateFile {
    fn extra(&self) -> &serde_json::Map<String, Value> {
        &self.extra
    }
}

impl HasExtra for okf_core::Track {
    fn extra(&self) -> &serde_json::Map<String, Value> {
        &self.extra
    }
}

impl HasExtra for okf_core::TrackBlock {
    fn extra(&self) -> &serde_json::Map<String, Value> {
        &self.extra
    }
}

impl HasExtra for okf_core::Backlog {
    fn extra(&self) -> &serde_json::Map<String, Value> {
        &self.extra
    }
}

impl HasExtra for okf_core::Epic {
    fn extra(&self) -> &serde_json::Map<String, Value> {
        &self.extra
    }
}

impl HasExtra for okf_core::Carryover {
    fn extra(&self) -> &serde_json::Map<String, Value> {
        &self.extra
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use okf_core::TrackBlock;

    fn track_block_seed() -> Value {
        json!({
            "id": "OK.3.A",
            "title": "seed block",
        })
    }

    #[test]
    fn note_field_is_present_on_track_block() {
        assert!(
            struct_has_field::<TrackBlock>(track_block_seed(), "note", "string?"),
            "expected `note` to be a real field on TrackBlock"
        );
    }

    #[test]
    fn unknown_field_is_absent_on_track_block() {
        assert!(
            !struct_has_field::<TrackBlock>(
                track_block_seed(),
                "definitely_not_a_field",
                "string?"
            ),
            "expected an unrecognised field name to be reported absent"
        );
    }
}
