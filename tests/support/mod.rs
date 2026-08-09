// Shared test-support module for the schema/struct conformance gate
// (`3.1-schema-struct-conformance`). A subdirectory of `tests/` is NOT
// compiled as its own test binary by cargo, so this stays shared support
// pulled in via `mod support;` from the actual test binaries under
// `tests/`, rather than an independent target.

pub mod schema_doc;
pub mod schema_floor;
pub mod schema_parse;
pub mod struct_probe;
