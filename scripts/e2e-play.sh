#!/usr/bin/env bash
# scripts/e2e-play.sh
#
# Magnetite MOAT — full-stack end-to-end integration runner.
#
# WHAT THIS PROVES
# ─────────────────
# The full Magnetite MOAT networking + game-loop stack, without requiring any
# pre-built Wasm artifacts:
#
#   1. Starts a real GameServer (NativeExecutor, ArenaShooter, ephemeral port).
#   2. Connects 3 independent tokio-tungstenite WebSocket clients.
#   3. Drives K input rounds per client.
#   4. Asserts every client receives:
#        - ServerNet::Welcome   (player accepted + config delivered)
#        - ServerNet::Snapshot  (authoritative full-state broadcast)
#        - ServerNet::Delta     (per-tick interest-filtered diff)
#        - ServerNet::Ack or Reject (input pipeline alive, anticheat running)
#   5. Verifies two independent NativeExecutor runs produce identical
#      state_hash on every tick — state convergence.
#   6. Verifies verify_replay returns Clean — the authoritative simulation is
#      deterministic and tamper-evident (anti-cheat guarantee).
#
# EXIT CODE
# ─────────
#   0  → PASS
#   1  → FAIL (test failure, build failure, or fmt violation)
#
# USAGE
# ─────
#   bash scripts/e2e-play.sh
#   bash scripts/e2e-play.sh --bench      # also run the #[ignore] bench tests
#   bash scripts/e2e-play.sh --no-fmt     # skip cargo fmt --check
#
# OUTPUT
# ──────
#   All output is written to /tmp/e2.txt.
#   Summary is printed to stdout.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
E2E_CRATE="$REPO_ROOT/magnetite-e2e"
LOG="/tmp/e2.txt"

RUN_BENCH=0
RUN_FMT=1
for arg in "$@"; do
    case "$arg" in
        --bench)    RUN_BENCH=1 ;;
        --no-fmt)   RUN_FMT=0   ;;
    esac
done

: > "$LOG"  # truncate

log()  { echo "[e2e-play] $*" | tee -a "$LOG"; }
pass() { echo "[e2e-play] PASS: $*" | tee -a "$LOG"; }
fail() { echo "[e2e-play] FAIL: $*" | tee -a "$LOG"; exit 1; }

log "========================================================"
log " Magnetite MOAT — full-stack end-to-end test runner"
log "========================================================"
log "Repo root : $REPO_ROOT"
log "Log       : $LOG"
log ""

# ── Step 1: cargo fmt --check ────────────────────────────────────────────────
if [[ $RUN_FMT -eq 1 ]]; then
    log "Step 1: cargo fmt --check (magnetite-e2e)"
    {
        echo "=== fmt check ==="
        cd "$E2E_CRATE"
        cargo fmt --check 2>&1
        echo "=== fmt done ==="
    } >> "$LOG" 2>&1
    FMT_EXIT=$?
    if [[ $FMT_EXIT -ne 0 ]]; then
        fail "cargo fmt --check failed — run 'cargo fmt' in magnetite-e2e/ to fix"
    fi
    pass "cargo fmt --check"
    log ""
else
    log "Step 1: fmt check SKIPPED (--no-fmt)"
    log ""
fi

# ── Step 2: cargo check --tests ──────────────────────────────────────────────
log "Step 2: cargo check --tests (magnetite-e2e)"
{
    echo "=== cargo check ==="
    cd "$E2E_CRATE"
    cargo check --tests 2>&1
    echo "=== cargo check done ==="
} >> "$LOG" 2>&1
CHECK_EXIT=$?
if [[ $CHECK_EXIT -ne 0 ]]; then
    fail "cargo check --tests failed (see $LOG)"
fi
pass "cargo check --tests"
log ""

# ── Step 3: full-stack WebSocket test ────────────────────────────────────────
log "Step 3: fullstack_ws tests"
log "  Tests:"
log "    fullstack_ws_welcome_snapshot_delta_ack_and_replay_clean"
log "      - 3 WS clients connect to a real GameServer (NativeExecutor/ArenaShooter)"
log "      - Every client receives Welcome + Snapshot + Delta"
log "      - Input pipeline: Ack or Reject received"
log "      - State convergence: two runs → same state_hash on every tick"
log "      - verify_replay → Clean (tamper-evident replay verification)"
log "    fullstack_ws_snapshot_ticks_are_monotonic"
log "      - Snapshot ticks from server are non-decreasing"
log "    fullstack_ws_dedicated_topology_smoke"
log "      - Dedicated topology (tick_hz=30, max_players=32) starts and broadcasts"

{
    echo "=== fullstack_ws test output ==="
    cd "$E2E_CRATE"
    cargo test --test fullstack_ws -- --nocapture 2>&1
    echo "=== fullstack_ws done ==="
} >> "$LOG" 2>&1
FS_EXIT=$?

if [[ $FS_EXIT -ne 0 ]]; then
    log ""
    log "  ERROR: fullstack_ws tests FAILED (exit $FS_EXIT)"
    log "  --- last 40 lines of log ---"
    tail -40 "$LOG"
    fail "fullstack_ws tests FAILED"
fi
pass "fullstack_ws (3/3 tests)"
log ""

# ── Step 4: convergence + anticheat tests ────────────────────────────────────
log "Step 4: convergence + anticheat tests"

{
    echo "=== convergence + anticheat ==="
    cd "$E2E_CRATE"
    cargo test --test convergence --test anticheat -- --nocapture 2>&1
    echo "=== convergence + anticheat done ==="
} >> "$LOG" 2>&1
CA_EXIT=$?

if [[ $CA_EXIT -ne 0 ]]; then
    log ""
    log "  ERROR: convergence/anticheat tests FAILED (exit $CA_EXIT)"
    tail -20 "$LOG"
    fail "convergence/anticheat tests FAILED"
fi
pass "convergence + anticheat (3/3 tests)"
log ""

# ── Step 5 (optional): bench tests ───────────────────────────────────────────
if [[ $RUN_BENCH -eq 1 ]]; then
    log "Step 5: scale bench (--ignored, --nocapture)"
    log "  Measures ticks/sec + per-tick latency across player counts and topologies."
    log "  This takes ~10 seconds."

    {
        echo "=== scale_bench ==="
        cd "$E2E_CRATE"
        cargo test --test scale_bench -- scale_bench --ignored --nocapture 2>&1
        echo "=== scale_bench done ==="
    } >> "$LOG" 2>&1
    BENCH_EXIT=$?

    if [[ $BENCH_EXIT -ne 0 ]]; then
        log "  WARN: scale_bench FAILED (exit $BENCH_EXIT) — see $LOG"
        # Don't fail the overall script; bench is informational.
    else
        pass "scale_bench"
        # Extract and print the bench report.
        grep -A 20 "Magnetite MOAT — Scale" "$LOG" | head -25 || true
    fi
    log ""
else
    log "Step 5: scale bench SKIPPED (pass --bench to run)"
    log "         cargo test -p magnetite-e2e -- scale_bench --ignored --nocapture"
    log ""
fi

# ── Summary ──────────────────────────────────────────────────────────────────
log "========================================================"
log " PASS — full-stack e2e complete"
log "========================================================"
log ""
log "  Proven:"
log "  - 3 WS clients received Welcome + Snapshot + Delta from a real GameServer"
log "  - Input pipeline live: Ack/Reject received for every input sent"
log "  - State convergence: NativeExecutor runs agree tick-by-tick"
log "  - verify_replay → Clean: simulation is deterministic + tamper-evident"
log "  - Dedicated topology: separate smoke test PASSED"
log ""
log "  To run the perf bench separately:"
log "    cd magnetite-e2e"
log "    cargo test -- scale_bench --ignored --nocapture"
log "    cargo test -- ws_round_trip_latency_bench --ignored --nocapture"
log ""
log "  Full log: $LOG"
log "========================================================"

echo ""
echo "PASS"
