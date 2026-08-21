#!/usr/bin/env bash
#
# test_check_consumers.sh — fixture-driven regression suite for
# scripts/check_consumers.sh.
#
# Self-contained: builds a throwaway "brain" tree in a temp dir (a fake
# brain.toml plus three fake consumer repos — bastion, engine-rs, mev —
# each wired to depend on a fake okf-core directory by path, the same way
# the real fleet is wired) and copies check_consumers.sh + a per-case
# waiver file into it. `cargo` and `git` are shimmed onto a PATH prepended
# ahead of the real ones, so no consumer is ever actually compiled and no
# real repo is ever touched:
#
#   - the cargo shim answers `nextest run --no-run --locked --manifest-path
#     <path>` by reading two files dropped in the target consumer's
#     directory (a fixture exit code and fixture stderr) and appends the
#     consumer's directory to a call-log, so a test can assert cargo was
#     (or was NOT — the dirty-skip case) invoked.
#   - the git shim answers `git -C <dir> status --porcelain` from a
#     `.fake-git-status` file dropped in that consumer's directory when one
#     is present, and otherwise passes straight through to the real git
#     (needed for check_consumers.sh's own `git rev-parse
#     --git-common-dir` call, used to resolve the canonical okf-core path).
#
#   bash scripts/test_check_consumers.sh
#
# Exit status 0 = every case passed; non-zero = at least one failure.
#
set -uo pipefail

fail=0
pass_count=0
fail_count=0
check() { # check <description> <result: 0=pass>
    if [ "$2" -eq 0 ]; then
        printf 'ok   %s\n' "$1"
        pass_count=$((pass_count + 1))
    else
        printf 'FAIL %s\n' "$1"
        fail=1
        fail_count=$((fail_count + 1))
    fi
}

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"

REAL_GIT_BIN="$(command -v git)"
REAL_CARGO_BIN="$(command -v cargo)"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# check_consumers.sh canonicalizes every repo path with `pwd -P`, which on
# macOS resolves /var/folders/... (mktemp's default) to /private/var/....
# Use the same canonical form here so the isolation check's prefix match
# against logged manifest paths is comparing like with like.
WORK_REAL="$(cd "$WORK" && pwd -P)"

# ---------------------------------------------------------------------------
# Fixture brain tree.
#
#   $WORK/brain/brain.toml
#   $WORK/brain/okf-core/scripts/check_consumers.sh   (copy under test)
#   $WORK/brain/okf-core/scripts/consumer-gate-waivers.txt
#   $WORK/brain/bastion/{Cargo.toml,Cargo.lock}          -- [dependencies] path edge
#   $WORK/brain/engine-rs/{Cargo.toml,Cargo.lock,engine-core/Cargo.toml}
#                                                          -- [workspace.dependencies] edge
#   $WORK/brain/mev/{Cargo.toml,Cargo.lock}              -- renamed-key path edge (`okf = {...}`)
# ---------------------------------------------------------------------------
BRAIN="$WORK/brain"
OKF="$BRAIN/okf-core"
mkdir -p "$OKF/scripts"

cp "$SELF_DIR/check_consumers.sh" "$OKF/scripts/check_consumers.sh"
chmod +x "$OKF/scripts/check_consumers.sh"
DEFAULT_WAIVER_FILE="$OKF/scripts/consumer-gate-waivers.txt"

# okf-core itself must be a real git repo so check_consumers.sh's own
# `git rev-parse --git-common-dir` (run to resolve its canonical path)
# succeeds via the pass-through half of the git shim.
"$REAL_GIT_BIN" init -q "$OKF"

cat > "$BRAIN/brain.toml" <<TOML
[[repos]]
slug = "bastion"
repo_path = "bastion"

[[repos]]
slug = "engine-rs"
repo_path = "engine-rs"

[[repos]]
slug = "mev"
repo_path = "mev"
TOML

# --- bastion: standard [dependencies] path edge -----------------------------
mkdir -p "$BRAIN/bastion"
cat > "$BRAIN/bastion/Cargo.toml" <<'EOF'
[package]
name = "bastion"
version = "0.1.0"

[dependencies]
okf-core = { path = "../okf-core" }
EOF
printf 'bastion-lock-v1\n' > "$BRAIN/bastion/Cargo.lock"

# --- engine-rs: [workspace.dependencies] edge --------------------------------
mkdir -p "$BRAIN/engine-rs/engine-core"
cat > "$BRAIN/engine-rs/Cargo.toml" <<'EOF'
[workspace]
members = ["engine-core"]

[workspace.dependencies]
okf-core = { path = "../okf-core" }
EOF
cat > "$BRAIN/engine-rs/engine-core/Cargo.toml" <<'EOF'
[package]
name = "engine-core"
version = "0.1.0"

[dependencies]
okf-core = { workspace = true }
EOF
printf 'engine-rs-lock-v1\n' > "$BRAIN/engine-rs/Cargo.lock"

# --- mev: renamed dependency key (`okf`, not `okf-core`) ---------------------
mkdir -p "$BRAIN/mev"
cat > "$BRAIN/mev/Cargo.toml" <<'EOF'
[package]
name = "mev"
version = "0.1.0"

[dependencies]
okf = { path = "../okf-core" }
EOF
printf 'mev-lock-v1\n' > "$BRAIN/mev/Cargo.lock"

# ---------------------------------------------------------------------------
# Shim bin dir, prepended onto PATH ahead of the real cargo/git.
# ---------------------------------------------------------------------------
BIN="$WORK/bin"
mkdir -p "$BIN"

CARGO_CALL_LOG="$WORK/cargo-calls.log"
: > "$CARGO_CALL_LOG"

cat > "$BIN/cargo" <<SH
#!/usr/bin/env bash
manifest=""
args=("\$@")
for i in "\${!args[@]}"; do
    if [ "\${args[\$i]}" = "--manifest-path" ]; then
        manifest="\${args[\$((i+1))]}"
    fi
done
repo_dir="\$(dirname "\$manifest")"
echo "\$repo_dir" >> "$CARGO_CALL_LOG"

exit_file="\$repo_dir/.fake-cargo-exit"
stderr_file="\$repo_dir/.fake-cargo-stderr"
mutate_file="\$repo_dir/.fake-cargo-mutate-lock"

ec=0
[ -f "\$exit_file" ] && ec="\$(cat "\$exit_file")"
if [ -f "\$stderr_file" ]; then
    cat "\$stderr_file" >&2
fi
if [ -f "\$mutate_file" ]; then
    echo "mutated-by-shim" >> "\$repo_dir/Cargo.lock"
fi
exit "\$ec"
SH
chmod +x "$BIN/cargo"

cat > "$BIN/git" <<SH
#!/usr/bin/env bash
REAL_GIT="$REAL_GIT_BIN"
if [ "\$1" = "-C" ]; then
    dir="\$2"
    # EMULATE REAL GIT'S PRECEDENCE, do not simplify this away.
    #
    # An inherited GIT_DIR OVERRIDES \`-C <dir>\`: real git reports on the
    # GIT_DIR repository and ignores the directory it was pointed at. A shim
    # that honours -C unconditionally cannot reproduce the class of bug that
    # blocked OK.5.A's own push (21/21 from a shell, 3/21 under the pre-push
    # hook), so the regression cases below would pass against a script with
    # no env scrub at all — i.e. the test would be decorative.
    if [ -n "\${GIT_DIR:-}" ]; then
        dir="\$(dirname "\$GIT_DIR")"
    fi
    if [ "\$3" = "status" ] && [ "\$4" = "--porcelain" ]; then
        if [ -f "\$dir/.fake-git-status" ]; then
            cat "\$dir/.fake-git-status"
            exit 0
        fi
        exit 0
    fi
    exec "\$REAL_GIT" "\$@"
fi
exec "\$REAL_GIT" "\$@"
SH
chmod +x "$BIN/git"

TEST_PATH="$BIN:$PATH"

# ---------------------------------------------------------------------------
# Fixture helpers
# ---------------------------------------------------------------------------

# Reset every consumer directory to a clean, non-dirty, no-fixture state
# between cases: clears cargo/git fixture markers, restores each Cargo.lock
# to its original recorded content, and writes an empty waiver file.
reset_fixtures() {
    local repo
    for repo in bastion engine-rs mev; do
        rm -f "$BRAIN/$repo/.fake-cargo-exit" "$BRAIN/$repo/.fake-cargo-stderr" \
              "$BRAIN/$repo/.fake-cargo-mutate-lock" "$BRAIN/$repo/.fake-git-status"
    done
    printf 'bastion-lock-v1\n' > "$BRAIN/bastion/Cargo.lock"
    printf 'engine-rs-lock-v1\n' > "$BRAIN/engine-rs/Cargo.lock"
    printf 'mev-lock-v1\n' > "$BRAIN/mev/Cargo.lock"
    : > "$DEFAULT_WAIVER_FILE"
    : > "$CARGO_CALL_LOG"
}

set_cargo_fixture() { # set_cargo_fixture <repo> <exit_code> <stderr_text>
    local repo="$1" ec="$2" stderr="$3"
    printf '%s' "$ec" > "$BRAIN/$repo/.fake-cargo-exit"
    printf '%s' "$stderr" > "$BRAIN/$repo/.fake-cargo-stderr"
}

set_git_dirty() { # set_git_dirty <repo>
    printf ' M src/lib.rs\n' > "$BRAIN/$1/.fake-git-status"
}

set_waiver_file() { # set_waiver_file <content>
    printf '%s' "$1" > "$DEFAULT_WAIVER_FILE"
}

run_gate() { # run_gate <args...> -- runs check_consumers.sh, sets OUT and RC
    OUT="$(PATH="$TEST_PATH" "$OKF/scripts/check_consumers.sh" "$@" 2>&1)"
    RC=$?
}

cargo_was_called() { # cargo_was_called <repo> -- 0 if the call log names it
    grep -qF "/$1" "$CARGO_CALL_LOG" 2>/dev/null
}

# ---------------------------------------------------------------------------
# The two recorded 2026-08-13 stderr captures reused as literal fixtures.
# ---------------------------------------------------------------------------
BASTION_BROKEN_STDERR='error[E0063]: missing field `title` in initializer of `Board`
  --> src/serve/handlers/board.rs:660:9
   |
660 |         Board { id, blocks }
   |         ^^^^^ missing `title`

error[E0308]: mismatched types
  --> src/serve/handlers/block_graph.rs:414:22
   |
414 |     let edge: StateEdgeKind = StateEdgeKind::CarryoverBlocks;
   |                      ^^^^^^^^^^^^^^^^^^^^^^^^^ expected `StateEdge`, found `StateEdgeKind`

error: aborting due to 2 previous errors
'

LOCKFILE_STALE_STDERR='error: failed to select a version for the requirement `okf-core = "^0.1"`
candidate versions found which didnt match: 0.1.0
location searched: /brain/okf-core
required by package `engine-core v0.1.0`
error: cannot update the lock file for dependency crate: run without --locked
'

# ---------------------------------------------------------------------------
# Case 1: discovery.
# ---------------------------------------------------------------------------
reset_fixtures
run_gate --list
check "discovery: --list finds bastion, engine-rs, mev, nothing else" \
    "$( [ "$(printf '%s\n' "$OUT" | sort)" = "$(printf 'bastion\nengine-rs\nmev\n' | sort)" ] && echo 0 || echo 1 )"

run_gate --list
check "discovery: engine-rs found via [workspace.dependencies]" \
    "$(printf '%s\n' "$OUT" | grep -qx 'engine-rs' && echo 0 || echo 1)"

run_gate --list
check "discovery: mev found via a renamed dependency key (okf = {path=...})" \
    "$(printf '%s\n' "$OUT" | grep -qx 'mev' && echo 0 || echo 1)"

# ---------------------------------------------------------------------------
# Case 2: bastion broken (recorded 2026-08-13 capture), unwaived.
# ---------------------------------------------------------------------------
reset_fixtures
set_cargo_fixture bastion 101 "$BASTION_BROKEN_STDERR"
run_gate --json
check "broken: bastion classifies broken (unwaived) and exits non-zero" \
    "$( [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"broken"' && echo 0 || echo 1 )"
check "broken: both error codes+sites are named" \
    "$(printf '%s' "$OUT" | grep -q '"code":"E0063","site":"src/serve/handlers/board.rs:660:9"' \
        && printf '%s' "$OUT" | grep -q '"code":"E0308","site":"src/serve/handlers/block_graph.rs:414:22"' \
        && echo 0 || echo 1)"

# ---------------------------------------------------------------------------
# Case 3: lockfile-stale signature wins over exit code, at BOTH 101 and 102.
# ---------------------------------------------------------------------------
reset_fixtures
set_cargo_fixture engine-rs 102 "$LOCKFILE_STALE_STDERR"
run_gate --json
check "lockfile-stale at exit 102 classifies lockfile-stale, exits 0" \
    "$( [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"lockfile-stale"' && echo 0 || echo 1 )"

reset_fixtures
set_cargo_fixture bastion 101 "$LOCKFILE_STALE_STDERR"
run_gate --json
check "lockfile-stale signature at exit 101 still classifies lockfile-stale (not broken)" \
    "$( [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"lockfile-stale"' && echo 0 || echo 1 )"

# ---------------------------------------------------------------------------
# Case 4: dirty tree short-circuits — cargo is never spawned.
# ---------------------------------------------------------------------------
reset_fixtures
set_git_dirty bastion
# Fixture that WOULD classify broken if cargo were ever invoked, to prove
# the short-circuit, not merely a lucky pass fixture.
set_cargo_fixture bastion 101 "$BASTION_BROKEN_STDERR"
run_gate --json
check "dirty tree classifies skipped-dirty and exits 0" \
    "$( [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"skipped-dirty"' && echo 0 || echo 1 )"
check "dirty tree: cargo was never spawned for bastion" \
    "$( cargo_was_called bastion && echo 1 || echo 0 )"

# ---------------------------------------------------------------------------
# Case 5: unrecognised failure classifies not-evaluable, never guessed broken.
# ---------------------------------------------------------------------------
reset_fixtures
set_cargo_fixture mev 1 'thread caused a panic; signal: killed
'
run_gate --json
check "unrecognised failure classifies not-evaluable (exit code in reason), exits 0" \
    "$( [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"not-evaluable"' \
        && printf '%s' "$OUT" | grep -q '"detail":"unrecognized failure (exit 1)"' && echo 0 || echo 1 )"

# ---------------------------------------------------------------------------
# Case 6: Cargo.lock hash moves across the run -> not-evaluable naming both digests.
# ---------------------------------------------------------------------------
reset_fixtures
set_cargo_fixture engine-rs 0 ''
: > "$BRAIN/engine-rs/.fake-cargo-mutate-lock"
run_gate --json
check "moved Cargo.lock hash classifies not-evaluable, exits 0" \
    "$( [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"not-evaluable"' \
        && printf '%s' "$OUT" | grep -q 'Cargo.lock hash changed' && echo 0 || echo 1 )"

# ---------------------------------------------------------------------------
# Case 7: ANSI-wrapped diagnostics still classify broken.
# ---------------------------------------------------------------------------
reset_fixtures
ANSI_STDERR=$'\x1b[1m\x1b[38;5;9merror[E0308]\x1b[0m\x1b[1m: mismatched types\x1b[0m\n  \x1b[1m\x1b[38;5;12m-->\x1b[0m src/serve/handlers/block_graph.rs:414:22\n'
set_cargo_fixture mev 101 "$ANSI_STDERR"
run_gate --json
check "ANSI-wrapped error[E0308] still classifies broken" \
    "$( [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"broken"' \
        && printf '%s' "$OUT" | grep -q '"code":"E0308"' && echo 0 || echo 1 )"

# ---------------------------------------------------------------------------
# Waiver cases (task 3).
# ---------------------------------------------------------------------------

# W1: broken + waived => exits 0, reported waived-by.
reset_fixtures
set_cargo_fixture bastion 101 "$BASTION_BROKEN_STDERR"
set_waiver_file 'bastion | unowned-fix-block | bastion is known broken pending unowned-fix-block'
run_gate --json
check "waiver: broken + waived exits 0 and reports waived-by" \
    "$( [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"waiver":"waived by unowned-fix-block"' && echo 0 || echo 1 )"

# W2: broken + unwaived => exits non-zero (distinct consumer from case 2, for isolation).
reset_fixtures
set_cargo_fixture engine-rs 101 "$BASTION_BROKEN_STDERR"
run_gate --json
check "waiver: broken + unwaived exits non-zero" \
    "$( [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q '"verdict":"broken"' && echo 0 || echo 1 )"

# W3: pass + waived => stale waiver, exits non-zero.
reset_fixtures
set_cargo_fixture mev 0 ''
set_waiver_file 'mev | some-block | mev used to be broken'
run_gate --json
check "waiver: pass + waived is a stale waiver and exits non-zero" \
    "$( [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q 'stale waiver' && echo 0 || echo 1 )"

# W4: malformed row (missing field) => hard error naming the line number.
reset_fixtures
set_waiver_file $'# header\nbastion | onlytwofields\n'
run_gate --list
check "waiver: malformed row (2 fields) is a hard error naming line 2" \
    "$( [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q ':2 ' && echo 0 || echo 1 )"

# W5: waiver naming an undiscovered slug => hard error.
reset_fixtures
set_waiver_file 'not-a-real-consumer | some-block | bogus row'
run_gate --list
check "waiver: unknown consumer slug is a hard error" \
    "$( [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q 'unknown consumer' && echo 0 || echo 1 )"

# W6: lockfile-stale, skipped-dirty and not-evaluable each exit 0 on their own,
# even with no waiver present at all (already exercised above; re-asserted
# here explicitly against a clean/no-waiver run for each verdict).
reset_fixtures
set_cargo_fixture engine-rs 102 "$LOCKFILE_STALE_STDERR"
run_gate --consumer engine-rs
w6a=$RC
reset_fixtures
set_git_dirty mev
run_gate --consumer mev
w6b=$RC
reset_fixtures
set_cargo_fixture bastion 1 'signal: killed'
run_gate --consumer bastion
w6c=$RC
check "waiver: lockfile-stale/skipped-dirty/not-evaluable each exit 0 with no waiver" \
    "$( [ "$w6a" -eq 0 ] && [ "$w6b" -eq 0 ] && [ "$w6c" -eq 0 ] && echo 0 || echo 1 )"

# ---------------------------------------------------------------------------
# Shown-failing controls (must be EXECUTED, not merely described). Each
# mutates a scratch copy of check_consumers.sh, re-runs the affected case
# against it, asserts the case now goes red, then discards the scratch copy
# (the copy under test at $OKF/scripts/check_consumers.sh is never touched).
# ---------------------------------------------------------------------------
CONTROL_LOG="$WORK/controls.log"
: > "$CONTROL_LOG"

run_gate_with_script() { # run_gate_with_script <script-path> <args...>
    local script="$1"; shift
    OUT="$(PATH="$TEST_PATH" bash "$script" "$@" 2>&1)"
    RC=$?
}

# Control scripts must live alongside the copy under test (same directory
# as check_consumers.sh and consumer-gate-waivers.txt) so find_brain_root's
# walk-up and WAIVER_FILE's $SCRIPT_DIR-relative resolution still land on
# the fixture brain tree, not somewhere under plain $WORK.

# Control 1: delete the stale-waiver check -> pass+waived must go red
# (i.e. must now incorrectly exit 0 instead of failing on "stale waiver").
CONTROL1_SCRIPT="$OKF/scripts/control1_check_consumers.sh"
cp "$OKF/scripts/check_consumers.sh" "$CONTROL1_SCRIPT"
python3 - "$CONTROL1_SCRIPT" <<'PY'
import sys
path = sys.argv[1]
with open(path) as f:
    text = f.read()
marker = '''            pass)
                GATE_NOTE="stale waiver (owned by $WAIVER_OWNER) — consumer now passes; delete this waiver row"
                return 1
                ;;'''
assert marker in text, "control1: stale-waiver branch not found verbatim"
mutated = text.replace(marker, '''            pass)
                return 0
                ;;''')
with open(path, "w") as f:
    f.write(mutated)
PY
reset_fixtures
set_cargo_fixture mev 0 ''
set_waiver_file 'mev | some-block | mev used to be broken'
run_gate_with_script "$CONTROL1_SCRIPT" --json
{
    echo "=== control 1: stale-waiver check deleted, pass+waived re-run ==="
    echo "exit code: $RC"
    echo "$OUT"
    echo
} >> "$CONTROL_LOG"
check "control 1 (stale-waiver check deleted): pass+waived now goes red (incorrectly exits 0)" \
    "$( [ "$RC" -eq 0 ] && echo 0 || echo 1 )"
rm -f "$CONTROL1_SCRIPT"

# Control 2: reorder classification so the exit code is read before the
# stderr signature -> the lockfile-at-101 case must go red (misclassified
# broken). Mutation: insert an exit-code-first branch ("exit 101 always
# means broken") ahead of the lockfile-stale signature check, exactly the
# bug this ordering rule exists to prevent.
CONTROL2_SCRIPT="$OKF/scripts/control2_check_consumers.sh"
cp "$OKF/scripts/check_consumers.sh" "$CONTROL2_SCRIPT"
python3 - "$CONTROL2_SCRIPT" <<'PY'
import sys
path = sys.argv[1]
with open(path) as f:
    text = f.read()
original = '''    if [ "$rc" -eq 0 ]; then
        VERDICT="pass"
        return 0
    fi

    if printf '%s' "$stderr_clean" | grep -q 'cannot update the lock file'; then'''
assert original in text, "control2: classification block not found verbatim"
reordered = '''    if [ "$rc" -eq 0 ]; then
        VERDICT="pass"
        return 0
    fi

    # BUG (control 2): exit code interpreted before the stderr signature —
    # exit 101 has been observed for both a real break and a stale lock,
    # so deciding from the exit code alone misclassifies one of them.
    if [ "$rc" -eq 101 ]; then
        VERDICT="broken"
        DETAIL="exit 101 assumed broken (exit-code-first bug)"
        return 0
    fi

    if printf '%s' "$stderr_clean" | grep -q 'cannot update the lock file'; then'''
text2 = text.replace(original, reordered, 1)
assert text2 != text, "control2: replacement did not apply"
with open(path, "w") as f:
    f.write(text2)
PY
reset_fixtures
set_cargo_fixture bastion 101 "$LOCKFILE_STALE_STDERR"
run_gate_with_script "$CONTROL2_SCRIPT" --json
{
    echo "=== control 2: exit-code-before-signature reorder, lockfile-at-101 re-run ==="
    echo "exit code: $RC"
    echo "$OUT"
    echo
} >> "$CONTROL_LOG"
check "control 2 (exit-code-before-signature): lockfile-at-101 now goes red (misclassified)" \
    "$( printf '%s' "$OUT" | grep -q '"verdict":"broken"' && echo 0 || echo 1 )"
rm -f "$CONTROL2_SCRIPT"

# ---------------------------------------------------------------------------
# Hook-environment regression (OK.5.A hotfix, 2026-08-21).
#
# `hooks/pre-push` exports GIT_DIR, and an inherited GIT_DIR OVERRIDES
# `git -C <dir>` — silently. Before the scrub at the top of
# check_consumers.sh, this suite passed 21/21 from a shell and 3/21 under
# the real hook, which is how a gate that had just been merged as
# `gates: true` blocked the very push that would have delivered it.
#
# These two cases are the positive control for that scrub. They set the
# hook's variables explicitly and assert the gate still answers about the
# CONSUMER, not about okf-core. Same root cause as mev's MV.17.A P0.
# ---------------------------------------------------------------------------
run_gate_hookenv() { # like run_gate, but with the pre-push hook's git env set
    OUT="$(PATH="$TEST_PATH" \
        GIT_DIR="$OKF/.git" \
        GIT_WORK_TREE="$OKF" \
        GIT_PREFIX="" \
        "$OKF/scripts/check_consumers.sh" "$@" 2>&1)"
    RC=$?
}

reset_fixtures
run_gate_hookenv --list
check "hook env: discovery still finds all three consumers with GIT_DIR/GIT_WORK_TREE set" \
    "$( [ "$(printf '%s\n' "$OUT" | sort)" = "$(printf 'bastion\nengine-rs\nmev\n' | sort)" ] && echo 0 || echo 1 )"

# THE discriminating case. bastion is dirty; the fixture okf-core that
# GIT_DIR points at is clean. With the scrub, `git -C bastion status` reports
# bastion => skipped-dirty. Without it, GIT_DIR wins, bastion inherits
# okf-core's clean status, cargo gets spawned against a dirty tree, and the
# verdict is a meaningless `pass`. Delete the scrub and this case goes red.
reset_fixtures
set_git_dirty bastion
set_cargo_fixture bastion 0 ""
run_gate_hookenv --json
check "hook env: a dirty consumer is still seen as dirty (GIT_DIR must not mask it)" \
    "$(printf '%s' "$OUT" | grep -q '"slug":"bastion","verdict":"skipped-dirty"' && echo 0 || echo 1)"
check "hook env: cargo was still never spawned for the dirty consumer" \
    "$( ! cargo_was_called bastion && echo 0 || echo 1 )"

# ---------------------------------------------------------------------------
# Zero discovered consumers must FAIL, never report a pass. This is the
# symptom the GIT_DIR bug actually produced, and reporting "all pass" on the
# strength of having checked nothing is the silent-green failure the gate
# exists to prevent (CLAUDE.md standing rule 11).
# ---------------------------------------------------------------------------
reset_fixtures
EMPTY_BRAIN="$(mktemp -d)"
mkdir -p "$EMPTY_BRAIN"
printf '# no [[repos]] at all\n' > "$EMPTY_BRAIN/brain.toml"
OUT="$(PATH="$TEST_PATH" OKF_BRAIN_ROOT="$EMPTY_BRAIN" "$OKF/scripts/check_consumers.sh" --json 2>&1)"; RC=$?
check "zero discovered consumers exits non-zero rather than reporting a pass" \
    "$( [ "$RC" -ne 0 ] && echo 0 || echo 1 )"
check "zero discovered consumers says so explicitly" \
    "$(printf '%s' "$OUT" | grep -q 'ZERO consumers' && echo 0 || echo 1)"
rm -rf "$EMPTY_BRAIN"

echo
echo "--- shown-failing control output (also destined for tasks.md Notes) ---"
cat "$CONTROL_LOG"
echo "--- end control output ---"
echo

# ---------------------------------------------------------------------------
# Isolation guard: this suite must never have touched anything outside its
# own mktemp -d — the only cargo/git on PATH during every run above were the
# shims, and every repo dir the shims ever saw is under $WORK.
# ---------------------------------------------------------------------------
check "isolation: every cargo invocation this suite logged stayed under \$WORK" \
    "$( ! grep -qv "^$WORK_REAL" "$CARGO_CALL_LOG" && echo 0 || echo 1 )"

echo
echo "== $pass_count passed, $fail_count failed =="
exit "$fail"
