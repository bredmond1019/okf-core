#!/usr/bin/env bash
# check_consumers.sh — consumer compile gate for okf-core.
#
# okf-core is the root of the fan-out: mev, bastion and engine-rs all depend
# on it by path. A breaking change to a shared type only ever surfaces to a
# plain `cargo build`/`cargo test` in okf-core's own repo if it also happens
# to be exercised by okf-core's own source or tests — the breaks this gate
# exists to catch (OK.3.B, D58, OK.4.B) all lived in TEST-only code in a
# *consumer* repo (a struct literal or match arm only a test target
# constructs), invisible from here without actually compiling that
# consumer's test targets.
#
# This script currently implements DISCOVERY only (task 1 of OK.5.A):
# find every repo registered in brain.toml whose Cargo.toml (root or any
# workspace member) declares a path dependency that resolves to this
# okf-core checkout, by walking the dependency tables `dependencies`,
# `dev-dependencies`, `build-dependencies` and `workspace.dependencies`.
# No consumer slug is hardcoded — engine-rs, for example, is found only
# because its root Cargo.toml's [workspace.dependencies] table carries
# `okf-core = { path = "../okf-core" }`.
#
# Classification (compiling each consumer's test targets with
# `cargo nextest run --no-run --locked`) and waiver handling land in later
# tasks of this spec; this script does not spawn cargo yet.
#
# Usage:
#   scripts/check_consumers.sh --list
#       Print the discovered consumer slugs, one per line, and exit 0.
#       Discovery only — nothing is compiled.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ---------------------------------------------------------------------------
# Locate the brain root by walking up from this script's own directory,
# looking for brain.toml (same idiom as scripts/check-typeshare.sh's
# REPO_ROOT resolution, one level further up the tree).
# ---------------------------------------------------------------------------
find_brain_root() {
    local dir="$SCRIPT_DIR"
    while [ "$dir" != "/" ]; do
        if [ -f "$dir/brain.toml" ]; then
            printf '%s\n' "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    echo "check_consumers: could not locate brain.toml by walking up from $SCRIPT_DIR" >&2
    exit 1
}

BRAIN_ROOT="$(find_brain_root)"
BRAIN_TOML="$BRAIN_ROOT/brain.toml"

# Canonical (symlink-resolved) path of this okf-core checkout — used to
# recognize "this is okf-core itself" and to decide whether a candidate
# repo's path dependency resolves back to it. Compared by path, not slug.
#
# This script may itself be run from a linked git worktree (e.g. an SDLC
# engine's per-block checkout under okf-core/trees/<slug>/), whose own
# directory is NOT the path any consumer's `path = "../okf-core"` resolves
# to — only the main checkout is. `git rev-parse --git-common-dir` returns
# the shared .git directory even from a linked worktree, so its parent is
# the main checkout's root regardless of where this script happens to be
# invoked from. Fall back to REPO_ROOT itself if git is unavailable (e.g.
# a plain export with no .git).
resolve_okf_core_real() {
    local common_git_dir main_root
    if common_git_dir="$(cd "$REPO_ROOT" && git rev-parse --git-common-dir 2>/dev/null)"; then
        case "$common_git_dir" in
            /*) : ;;
            *) common_git_dir="$REPO_ROOT/$common_git_dir" ;;
        esac
        if main_root="$(cd "$common_git_dir/.." && pwd -P)"; then
            printf '%s\n' "$main_root"
            return 0
        fi
    fi
    cd "$REPO_ROOT" && pwd -P
}

OKF_CORE_REAL="$(resolve_okf_core_real)"

# ---------------------------------------------------------------------------
# TOML helpers (deliberately POSIX-awk/sed only — no gawk extensions, since
# this must run unmodified on the fleet's macOS/BSD toolchain).
# ---------------------------------------------------------------------------

# Print each [[repos]] block's `slug` and `repo_path` as "slug<TAB>repo_path",
# one line per block. A missing key yields an empty field.
parse_repos_blocks() {
    awk '
        function flush() {
            if (in_block) {
                printf "%s\t%s\n", slug, repo_path
            }
        }
        /^\[\[repos\]\]/ {
            flush()
            in_block = 1
            slug = ""
            repo_path = ""
            next
        }
        /^\[/ {
            # Any other table header ends the current [[repos]] block.
            flush()
            in_block = 0
            next
        }
        in_block && /^slug[ \t]*=/ {
            v = $0
            sub(/^slug[ \t]*=[ \t]*/, "", v)
            gsub(/^"|"[ \t]*$/, "", v)
            slug = v
            next
        }
        in_block && /^repo_path[ \t]*=/ {
            v = $0
            sub(/^repo_path[ \t]*=[ \t]*/, "", v)
            gsub(/^"|"[ \t]*$/, "", v)
            repo_path = v
            next
        }
        END { flush() }
    ' "$BRAIN_TOML"
}

# Print the quoted entries of a root Cargo.toml's [workspace] members = [...]
# array, one per line, unquoted. Empty output if there is no such array.
extract_workspace_members() {
    local manifest="$1"
    awk '
        /^\[workspace\]$/ { in_ws = 1; next }
        /^\[/ { in_ws = 0 }
        in_ws && /members[ \t]*=/ { in_members = 1 }
        in_members { print }
        in_members && /\]/ { in_members = 0 }
    ' "$manifest" | grep -o '"[^"]*"' | tr -d '"'
}

# Print every line of a manifest that falls inside a dependency table
# (dependencies / dev-dependencies / build-dependencies /
# workspace.dependencies), whether that table is the bracketed section
# itself (`[dependencies]` ... inline-table entries) or a dotted per-crate
# section (`[dependencies.okf-core]` ... `path = "..."`).
lines_in_dependency_tables() {
    local manifest="$1"
    awk '
        /^\[(dependencies|dev-dependencies|build-dependencies|workspace\.dependencies)\]$/ {
            in_section = 1
            next
        }
        /^\[(dependencies|dev-dependencies|build-dependencies|workspace\.dependencies)\.[A-Za-z0-9_-]+\]$/ {
            in_section = 1
            next
        }
        /^\[/ { in_section = 0 }
        in_section { print }
    ' "$manifest"
}

# Given a manifest path, print the canonicalized directory each of its
# dependency-table `path = "..."` entries resolves to (relative to the
# manifest's own directory), one per line. Entries whose target directory
# does not exist are silently skipped (not our repo to validate).
manifest_path_dep_targets() {
    local manifest="$1"
    local manifest_dir
    manifest_dir="$(cd "$(dirname "$manifest")" && pwd -P)"
    lines_in_dependency_tables "$manifest" \
        | grep -o 'path[ \t]*=[ \t]*"[^"]*"' \
        | sed -E 's/path[ \t]*=[ \t]*"([^"]*)"/\1/' \
        | while IFS= read -r rel; do
            [ -n "$rel" ] || continue
            if target="$(cd "$manifest_dir/$rel" 2>/dev/null && pwd -P)"; then
                printf '%s\n' "$target"
            fi
        done
}

# True (0) if any dependency-table path entry in any of this repo's
# manifests (its root Cargo.toml, plus any [workspace] member manifests)
# resolves to okf-core's own canonical directory.
repo_is_consumer() {
    local repo_dir="$1"
    local root_manifest="$repo_dir/Cargo.toml"
    local manifest
    local member

    for manifest in "$root_manifest" $(
        extract_workspace_members "$root_manifest" | while IFS= read -r member; do
            [ -n "$member" ] || continue
            printf '%s/%s/Cargo.toml\n' "$repo_dir" "$member"
        done
    ); do
        [ -f "$manifest" ] || continue
        while IFS= read -r target; do
            [ -n "$target" ] || continue
            if [ "$target" = "$OKF_CORE_REAL" ]; then
                return 0
            fi
        done < <(manifest_path_dep_targets "$manifest")
    done
    return 1
}

# ---------------------------------------------------------------------------
# Discovery: walk every [[repos]] block, skip the ones that can't be
# consumers on their face, then test the rest for a path dependency back to
# okf-core.
# ---------------------------------------------------------------------------
discover_consumers() {
    local consumers=()
    local slug repo_path repo_dir repo_real

    while IFS=$'\t' read -r slug repo_path; do
        [ -n "$slug" ] || continue
        [ -n "$repo_path" ] || continue   # skip empty repo_path

        repo_dir="$BRAIN_ROOT/$repo_path"
        [ -f "$repo_dir/Cargo.toml" ] || continue   # skip: no Cargo.toml

        repo_real="$(cd "$repo_dir" 2>/dev/null && pwd -P)" || continue
        [ "$repo_real" != "$OKF_CORE_REAL" ] || continue   # skip okf-core itself

        if repo_is_consumer "$repo_real"; then
            consumers+=("$slug")
        fi
    done < <(parse_repos_blocks)

    if [ "${#consumers[@]}" -gt 0 ]; then
        printf '%s\n' "${consumers[@]}" | sort
    fi
}

# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
usage() {
    cat <<'EOF'
Usage: check_consumers.sh --list

  --list    Print the discovered consumer slugs, one per line, and exit 0.
            Discovery only; nothing is compiled.
EOF
}

main() {
    if [ "$#" -eq 0 ]; then
        usage >&2
        exit 1
    fi

    case "$1" in
        --list)
            discover_consumers
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "check_consumers: unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
}

main "$@"
