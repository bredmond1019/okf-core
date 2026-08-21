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
# Discovery (task 1): find every repo registered in brain.toml whose
# Cargo.toml (root or any workspace member) declares a path dependency that
# resolves to this okf-core checkout, by walking the dependency tables
# `dependencies`, `dev-dependencies`, `build-dependencies` and
# `workspace.dependencies`. No consumer slug is hardcoded — engine-rs, for
# example, is found only because its root Cargo.toml's
# [workspace.dependencies] table carries `okf-core = { path = "../okf-core" }`.
#
# Run + classify (task 2): for each discovered consumer, compile its TEST
# targets only (`cargo nextest run --no-run --locked`) — the break class
# this gate exists to catch (OK.3.B, D58, OK.4.B) lived entirely in
# test-only code (struct literals, match arms) that a plain `cargo build`
# never walks. A dirty consumer is skipped without ever spawning cargo: its
# compile result is not evidence about okf-core either way. Classification
# reads the stderr SIGNATURE before the exit code, never the reverse — exit
# 101 has been observed both for a real break and for a stale lock, so the
# exit code alone cannot distinguish them.
#
# Waiver handling (task 3): scripts/consumer-gate-waivers.txt names a
# consumer that is knowingly broken so the gate stays green while the fix
# lives in another lane's repo. A `broken` consumer with a waiver does not
# fail the gate; a `pass` consumer that STILL has a waiver does — that is
# what stops a waiver row from outliving the break it was filed for. Every
# waiver row must carry a slug, an owning block id and a reason; a
# malformed row or one naming an undiscovered consumer is a hard error.
#
# Usage:
#   scripts/check_consumers.sh
#       Discover, run, classify and print a human report for every
#       consumer. Exits non-zero iff a consumer is broken-and-unwaived, or
#       a waiver is stale (its consumer now passes), or a waiver row is
#       malformed.
#   scripts/check_consumers.sh --list
#       Print the discovered consumer slugs, one per line, and exit 0.
#       Discovery only — nothing is compiled.
#   scripts/check_consumers.sh --json
#       Same run as the default, emitted as compact JSON instead of the
#       human report. Same exit-code rule.
#   scripts/check_consumers.sh --consumer <slug>
#       Run and report on exactly one discovered consumer. An unknown slug
#       is a hard error naming the valid ones; nothing is compiled.
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
# okf-core. Populates the parallel global arrays CONSUMER_SLUGS and
# CONSUMER_DIRS (sorted by slug), consumed by every run mode below.
# ---------------------------------------------------------------------------
CONSUMER_SLUGS=()
CONSUMER_DIRS=()

discover_consumer_records() {
    CONSUMER_SLUGS=()
    CONSUMER_DIRS=()
    local slug repo_path repo_dir repo_real
    local records=()

    while IFS=$'\t' read -r slug repo_path; do
        [ -n "$slug" ] || continue
        [ -n "$repo_path" ] || continue   # skip empty repo_path

        repo_dir="$BRAIN_ROOT/$repo_path"
        [ -f "$repo_dir/Cargo.toml" ] || continue   # skip: no Cargo.toml

        repo_real="$(cd "$repo_dir" 2>/dev/null && pwd -P)" || continue
        [ "$repo_real" != "$OKF_CORE_REAL" ] || continue   # skip okf-core itself

        if repo_is_consumer "$repo_real"; then
            records+=("$slug"$'\t'"$repo_real")
        fi
    done < <(parse_repos_blocks)

    if [ "${#records[@]}" -gt 0 ]; then
        while IFS=$'\t' read -r slug repo_real; do
            CONSUMER_SLUGS+=("$slug")
            CONSUMER_DIRS+=("$repo_real")
        done < <(printf '%s\n' "${records[@]}" | sort)
    fi
}

print_consumer_list() {
    if [ "${#CONSUMER_SLUGS[@]}" -gt 0 ]; then
        printf '%s\n' "${CONSUMER_SLUGS[@]}"
    fi
}

# Narrow CONSUMER_SLUGS/CONSUMER_DIRS down to exactly the one matching
# $1, or exit non-zero naming the valid slugs. Must run after discovery.
select_single_consumer() {
    local target="$1"
    local i
    for i in "${!CONSUMER_SLUGS[@]}"; do
        if [ "${CONSUMER_SLUGS[$i]}" = "$target" ]; then
            CONSUMER_SLUGS=("$target")
            CONSUMER_DIRS=("${CONSUMER_DIRS[$i]}")
            return 0
        fi
    done
    echo "check_consumers: unknown consumer '$target' (valid: $(printf '%s, ' "${CONSUMER_SLUGS[@]}" | sed -E 's/, $//'))" >&2
    exit 1
}

# ---------------------------------------------------------------------------
# Run + classify: compile one consumer's TEST targets with
# `cargo nextest run --no-run --locked` and classify the result.
# ---------------------------------------------------------------------------

# Strip ANSI CSI sequences (e.g. color codes) from stdin, as defence in
# depth alongside CARGO_TERM_COLOR=never — rustc's `error[E....]` prefix
# must never be defeated by escape codes wrapped around it.
strip_ansi() {
    sed -E $'s/\x1b\\[[0-9;]*[A-Za-z]//g'
}

# Print a Cargo.lock's sha256 digest, or the literal string "missing" if
# the file does not exist. "missing" is itself a valid, comparable state.
hash_lockfile() {
    local path="$1"
    if [ -f "$path" ]; then
        shasum -a 256 "$path" | awk '{print $1}'
    else
        printf 'missing'
    fi
}

# Read ANSI-stripped rustc stderr on stdin and print one line per
# `error[E....]` diagnostic: "<CODE> <file:line:col>" when a `--> ` site
# line follows within the next few lines, or bare "<CODE>" otherwise.
extract_error_sites() {
    awk '
        function flush() {
            if (code != "") {
                if (site != "") {
                    printf "%s %s\n", code, site
                } else {
                    printf "%s\n", code
                }
            }
            code = ""; site = ""; lookahead = 0
        }
        {
            if (match($0, /error\[E[0-9]+\]/)) {
                flush()
                full = substr($0, RSTART, RLENGTH)
                match(full, /E[0-9]+/)
                code = substr(full, RSTART, RLENGTH)
                lookahead = 5
                next
            }
            if (code != "" && lookahead > 0) {
                if ($0 ~ /-->/) {
                    line = $0
                    sub(/^[ \t]*-->[ \t]*/, "", line)
                    gsub(/[ \t]+$/, "", line)
                    site = line
                    flush()
                    next
                }
                lookahead--
                if (lookahead == 0) {
                    flush()
                }
            }
        }
        END { flush() }
    '
}

# Run and classify one consumer. Sets the globals VERDICT (one of pass,
# broken, lockfile-stale, skipped-dirty, not-evaluable), DETAIL (a reason
# string; empty for pass/broken) and ERROR_LINES (array of
# "<CODE> [<site>]" strings; only populated for broken). Never spawns
# cargo against a dirty tree.
VERDICT=""
DETAIL=""
ERROR_LINES=()

run_and_classify_consumer() {
    local slug="$1" repo_dir="$2"
    VERDICT=""
    DETAIL=""
    ERROR_LINES=()

    local git_status git_rc=0
    git_status="$(git -C "$repo_dir" status --porcelain 2>&1)" || git_rc=$?
    if [ "$git_rc" -ne 0 ]; then
        VERDICT="not-evaluable"
        DETAIL="git status failed: $git_status"
        return 0
    fi
    if [ -n "$git_status" ]; then
        VERDICT="skipped-dirty"
        DETAIL="working tree has uncommitted changes"
        return 0
    fi

    local lockfile="$repo_dir/Cargo.lock"
    local hash_before hash_after
    hash_before="$(hash_lockfile "$lockfile")"

    local target_dir stderr_file rc=0
    target_dir="$(mktemp -d)"
    stderr_file="$(mktemp)"

    CARGO_TARGET_DIR="$target_dir" CARGO_TERM_COLOR=never \
        cargo nextest run --no-run --locked --manifest-path "$repo_dir/Cargo.toml" \
        >/dev/null 2>"$stderr_file" || rc=$?

    local stderr_raw stderr_clean
    stderr_raw="$(cat "$stderr_file")"
    rm -f "$stderr_file"
    rm -rf "$target_dir"

    stderr_clean="$(printf '%s' "$stderr_raw" | strip_ansi)"

    hash_after="$(hash_lockfile "$lockfile")"
    if [ "$hash_before" != "$hash_after" ]; then
        VERDICT="not-evaluable"
        DETAIL="Cargo.lock hash changed ($hash_before -> $hash_after); --locked was dropped or defeated"
        return 0
    fi

    if [ "$rc" -eq 0 ]; then
        VERDICT="pass"
        return 0
    fi

    if printf '%s' "$stderr_clean" | grep -q 'cannot update the lock file'; then
        VERDICT="lockfile-stale"
        DETAIL="cargo reported: cannot update the lock file (exit $rc)"
        return 0
    fi

    if printf '%s' "$stderr_clean" | grep -Eq 'error\[E[0-9]+\]'; then
        VERDICT="broken"
        local line
        while IFS= read -r line; do
            [ -n "$line" ] || continue
            ERROR_LINES+=("$line")
        done < <(printf '%s' "$stderr_clean" | extract_error_sites)
        return 0
    fi

    VERDICT="not-evaluable"
    DETAIL="unrecognized failure (exit $rc)"
    return 0
}

# ---------------------------------------------------------------------------
# Waivers (task 3): scripts/consumer-gate-waivers.txt keeps okf-core
# pushable while a consumer is knowingly broken, without letting the debt
# go silently permanent. Format: one row per waived consumer —
#
#   <slug> | <owning-block-id> | <reason>
#
# `#` comments and blank lines are ignored. All three fields are mandatory:
# a row missing any of them, or naming a slug that is not a discovered
# consumer, is a hard error naming the offending line number — a waiver
# with no owning block is how debt becomes permanent.
#
# A `broken` consumer WITH a waiver does not fail the gate (reported as
# "broken (waived by <block-id>)"). A consumer that is `pass` but STILL HAS
# a waiver FAILS the gate as a stale waiver — that is the property that
# stops the waiver file from outliving the break it was filed for.
# lockfile-stale, skipped-dirty and not-evaluable never fail the gate
# either way; none of them is evidence that okf-core broke anything.
# ---------------------------------------------------------------------------
WAIVER_FILE="$SCRIPT_DIR/consumer-gate-waivers.txt"
WAIVER_SLUGS=()
WAIVER_BLOCKS=()
WAIVER_REASONS=()

trim() {
    printf '%s' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

# Parse and validate WAIVER_FILE into WAIVER_SLUGS/WAIVER_BLOCKS/
# WAIVER_REASONS. Must run after discover_consumer_records — each waived
# slug is validated against the full discovered CONSUMER_SLUGS list, not a
# --consumer-narrowed subset, so the file's validity never depends on which
# flag it was run with. Exits non-zero on any malformed row or unknown
# slug, naming WAIVER_FILE and the offending line number.
parse_waivers() {
    WAIVER_SLUGS=()
    WAIVER_BLOCKS=()
    WAIVER_REASONS=()
    [ -f "$WAIVER_FILE" ] || return 0

    local lineno=0 rawline trimmed f1 f2 f3 known s
    while IFS= read -r rawline || [ -n "$rawline" ]; do
        lineno=$((lineno + 1))
        trimmed="$(trim "$rawline")"
        [ -z "$trimmed" ] && continue
        case "$trimmed" in
            \#*) continue ;;
        esac

        IFS='|' read -r f1 f2 f3 <<< "$trimmed"
        f1="$(trim "${f1:-}")"
        f2="$(trim "${f2:-}")"
        f3="$(trim "${f3:-}")"

        if [ -z "$f1" ] || [ -z "$f2" ] || [ -z "$f3" ]; then
            echo "check_consumers: malformed waiver row at $WAIVER_FILE:$lineno — need 3 fields: slug | owning-block-id | reason" >&2
            exit 1
        fi

        known=0
        for s in "${CONSUMER_SLUGS[@]}"; do
            if [ "$s" = "$f1" ]; then
                known=1
                break
            fi
        done
        if [ "$known" -eq 0 ]; then
            echo "check_consumers: waiver at $WAIVER_FILE:$lineno names unknown consumer '$f1' (discovered: $(printf '%s, ' "${CONSUMER_SLUGS[@]}" | sed -E 's/, $//'))" >&2
            exit 1
        fi

        WAIVER_SLUGS+=("$f1")
        WAIVER_BLOCKS+=("$f2")
        WAIVER_REASONS+=("$f3")
    done < "$WAIVER_FILE"
}

# Sets WAIVER_OWNER to $1's owning block id and returns 0 if $1 has a
# waiver row; returns 1 (WAIVER_OWNER cleared) otherwise.
WAIVER_OWNER=""
lookup_waiver() {
    local slug="$1" i
    WAIVER_OWNER=""
    for i in "${!WAIVER_SLUGS[@]}"; do
        if [ "${WAIVER_SLUGS[$i]}" = "$slug" ]; then
            WAIVER_OWNER="${WAIVER_BLOCKS[$i]}"
            return 0
        fi
    done
    return 1
}

# Decide whether one consumer's (slug, verdict) pair fails the gate. Sets
# GATE_NOTE to a human-readable suffix (may be empty) and returns 0 when
# the gate should NOT fail on this consumer, 1 when it should.
GATE_NOTE=""
gate_outcome_for() {
    local slug="$1" verdict="$2"
    GATE_NOTE=""
    if lookup_waiver "$slug"; then
        case "$verdict" in
            broken)
                GATE_NOTE="waived by $WAIVER_OWNER"
                return 0
                ;;
            pass)
                GATE_NOTE="stale waiver (owned by $WAIVER_OWNER) — consumer now passes; delete this waiver row"
                return 1
                ;;
            *)
                return 0
                ;;
        esac
    fi
    if [ "$verdict" = "broken" ]; then
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Report rendering: human text and compact JSON, both over CONSUMER_SLUGS.
# Both append every observed verdict to RESULT_VERDICTS so the caller can
# decide the exit code.
# ---------------------------------------------------------------------------
RESULT_VERDICTS=()
ANY_GATE_FAILURE=0

json_escape() {
    local s="$1"
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    printf '%s' "$s"
}

print_report() {
    local i slug dir eline
    for i in "${!CONSUMER_SLUGS[@]}"; do
        slug="${CONSUMER_SLUGS[$i]}"
        dir="${CONSUMER_DIRS[$i]}"
        run_and_classify_consumer "$slug" "$dir"
        if ! gate_outcome_for "$slug" "$VERDICT"; then
            ANY_GATE_FAILURE=1
        fi
        if [ -n "$GATE_NOTE" ]; then
            echo "== $slug: $VERDICT ($GATE_NOTE) =="
        else
            echo "== $slug: $VERDICT =="
        fi
        if [ "$VERDICT" = "broken" ]; then
            for eline in "${ERROR_LINES[@]}"; do
                echo "  $eline"
            done
        elif [ -n "$DETAIL" ]; then
            echo "  $DETAIL"
        fi
        RESULT_VERDICTS+=("$VERDICT")
    done
}

print_json() {
    local i slug dir first=1 efirst eline code site gate_fail_json
    printf '['
    for i in "${!CONSUMER_SLUGS[@]}"; do
        slug="${CONSUMER_SLUGS[$i]}"
        dir="${CONSUMER_DIRS[$i]}"
        run_and_classify_consumer "$slug" "$dir"
        if gate_outcome_for "$slug" "$VERDICT"; then
            gate_fail_json="false"
        else
            gate_fail_json="true"
            ANY_GATE_FAILURE=1
        fi
        [ "$first" -eq 1 ] || printf ','
        first=0
        printf '{"slug":"%s","verdict":"%s","gate_fail":%s' "$(json_escape "$slug")" "$(json_escape "$VERDICT")" "$gate_fail_json"
        if [ -n "$GATE_NOTE" ]; then
            printf ',"waiver":"%s"' "$(json_escape "$GATE_NOTE")"
        fi
        if [ -n "$DETAIL" ]; then
            printf ',"detail":"%s"' "$(json_escape "$DETAIL")"
        fi
        if [ "${#ERROR_LINES[@]}" -gt 0 ]; then
            printf ',"errors":['
            efirst=1
            for eline in "${ERROR_LINES[@]}"; do
                [ "$efirst" -eq 1 ] || printf ','
                efirst=0
                code="${eline%% *}"
                if [ "$eline" = "$code" ]; then
                    site=""
                else
                    site="${eline#* }"
                fi
                if [ -n "$site" ]; then
                    printf '{"code":"%s","site":"%s"}' "$(json_escape "$code")" "$(json_escape "$site")"
                else
                    printf '{"code":"%s"}' "$(json_escape "$code")"
                fi
            done
            printf ']'
        fi
        printf '}'
        RESULT_VERDICTS+=("$VERDICT")
    done
    printf ']\n'
}

# Exit non-zero iff at least one consumer is broken-and-unwaived, or a
# waiver is stale (its consumer now passes). ANY_GATE_FAILURE is set by
# print_report/print_json via gate_outcome_for as each consumer is
# classified; a malformed or unknown-slug waiver row exits earlier, from
# parse_waivers itself, before any consumer is even run.
exit_for_verdicts() {
    if [ "$ANY_GATE_FAILURE" -eq 1 ]; then
        exit 1
    fi
    exit 0
}

# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
usage() {
    cat <<'EOF'
Usage: check_consumers.sh [--list | --json | --consumer <slug>]

  (no args)          Discover, run, classify and report on every consumer.
                      Exits non-zero iff a consumer is broken-and-unwaived,
                      a waiver is stale, or a waiver row is malformed.
  --list             Print the discovered consumer slugs, one per line,
                      and exit 0. Discovery only; nothing is compiled.
  --json             Same run as (no args), emitted as compact JSON.
  --consumer <slug>  Run and report on exactly one discovered consumer.
                      An unknown slug is a hard error naming the valid ones.
EOF
}

main() {
    discover_consumer_records
    # Validated against the FULL discovered list, before --consumer (below)
    # narrows it — a waiver's validity must never depend on which flag the
    # gate was run with.
    parse_waivers

    if [ "$#" -eq 0 ]; then
        RESULT_VERDICTS=()
        ANY_GATE_FAILURE=0
        print_report
        exit_for_verdicts
    fi

    case "$1" in
        --list)
            print_consumer_list
            ;;
        --json)
            RESULT_VERDICTS=()
            ANY_GATE_FAILURE=0
            print_json
            exit_for_verdicts
            ;;
        --consumer)
            if [ -z "${2:-}" ]; then
                echo "check_consumers: --consumer requires a slug argument" >&2
                exit 1
            fi
            select_single_consumer "$2"
            RESULT_VERDICTS=()
            ANY_GATE_FAILURE=0
            print_report
            exit_for_verdicts
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
