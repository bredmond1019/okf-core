#!/usr/bin/env bash
# check-typeshare.sh — generation guard for the typeshare-exported BlockedBy
# payload structs (BlockDep, ExternalDep, OperatorDep, ApprovalDep).
#
# Regenerates the TypeScript for this crate and asserts all four interfaces
# are present in the output. This is a developer convenience, not a gate:
# it must never hard-fail a machine that lacks the typeshare CLI, so an
# absent CLI is reported and the script exits 0.
#
# Usage:
#   scripts/check-typeshare.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# `command -v typeshare` failing does NOT mean "not installed": cargo installs
# into ~/.cargo/bin, which is not on PATH on every machine. Probe that
# directory before concluding the CLI is missing (same trap documented in
# core/bastion/scripts/gen-types.sh).
if ! command -v typeshare >/dev/null 2>&1; then
    CARGO_BIN_TYPESHARE="${CARGO_HOME:-${HOME:-}/.cargo}/bin/typeshare"
    if [ -x "$CARGO_BIN_TYPESHARE" ]; then
        echo "warning: 'typeshare' is installed at $CARGO_BIN_TYPESHARE, but that directory is not on PATH." >&2
        echo "using the installed binary directly and continuing." >&2
        PATH="$(dirname "$CARGO_BIN_TYPESHARE"):$PATH"
        export PATH
    else
        echo "check-typeshare: 'typeshare' CLI not found on PATH or in ~/.cargo/bin — skipping generation check."
        echo "install it with: cargo install typeshare-cli --locked"
        exit 0
    fi
fi

OUTPUT_FILE="$(mktemp -t okf-core-typeshare.XXXXXX).ts"
trap 'rm -f "$OUTPUT_FILE"' EXIT

typeshare "$REPO_ROOT/src" --lang typescript --output-file "$OUTPUT_FILE"

missing=()
for iface in BlockDep ExternalDep OperatorDep ApprovalDep; do
    if ! grep -q "interface $iface" "$OUTPUT_FILE"; then
        missing+=("$iface")
    fi
done

if [ "${#missing[@]}" -ne 0 ]; then
    echo "check-typeshare: missing expected interfaces: ${missing[*]}" >&2
    exit 1
fi

echo "check-typeshare: all four payload interfaces present (BlockDep, ExternalDep, OperatorDep, ApprovalDep)."
