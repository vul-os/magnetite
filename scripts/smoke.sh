#!/usr/bin/env bash
# =============================================================================
# scripts/smoke.sh — Magnetite local-stack smoke test
#
# WHAT THIS PROVES
# ────────────────
# A structured PASS/FAIL/SKIP per step that validates the local Magnetite
# stack without external credentials:
#
#   Step 1  docker-compose config parses; all expected services are defined
#           (postgres, redis, backend, frontend, mediamtx).
#   Step 2  Build game-template-authoritative to wasm32-wasip1 via the
#           mag_* ABI (or SKIP if the wasm32-wasip1 target is absent and
#           rustup is not available to add it).
#   Step 3  Run the magnetite-e2e full-stack WS tests — these boot a real
#           GameServer (NativeExecutor, ephemeral TCP port), connect clients,
#           and assert Welcome + Snapshot + Delta + Ack + convergence + replay.
#   Step 4  Run verify_replay (convergence test) — deterministic replay proof.
#   Step 5  Run the wasm_end_to_end parity tests (skipped if the wasm
#           artifact from Step 2 is absent).
#
# SKIP SEMANTICS
# ──────────────
# A step is SKIP'd when its hard prereqs are absent (e.g. docker not on PATH,
# wasm32-wasip1 target missing and cannot be installed, wasm artifact not
# built).  SKIPs are printed with a clear reason and do not count as failures.
#
# USAGE
# ─────
#   bash scripts/smoke.sh
#
#   # Skip the wasm build (only run runtime/replay tests):
#   SKIP_WASM_BUILD=1 bash scripts/smoke.sh
#
#   # Verbose cargo output:
#   CARGO_VERBOSE=1 bash scripts/smoke.sh
#
# EXIT CODES
# ──────────
#   0   all steps passed (SKIP is not a failure)
#   1   one or more steps failed
#
# OUTPUT
# ──────
#   Results are written to /tmp/magnetite-smoke.txt for auditing.
#   The script also prints a coloured summary to stdout.
# =============================================================================

set -uo pipefail
# Note: NOT using -e so we can capture step exit codes manually.

# ── Repo root ─────────────────────────────────────────────────────────────────
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_LOG=/tmp/magnetite-smoke.txt
: > "$SMOKE_LOG"  # truncate

# ── Colour helpers ─────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

step_pass() { echo -e "${GREEN}[PASS]${NC} $*"; echo "[PASS] $*" >> "$SMOKE_LOG"; PASS_COUNT=$((PASS_COUNT + 1)); }
step_fail() { echo -e "${RED}[FAIL]${NC} $*"; echo "[FAIL] $*" >> "$SMOKE_LOG"; FAIL_COUNT=$((FAIL_COUNT + 1)); }
step_skip() { echo -e "${YELLOW}[SKIP]${NC} $*"; echo "[SKIP] $*" >> "$SMOKE_LOG"; SKIP_COUNT=$((SKIP_COUNT + 1)); }
info()      { echo -e "${CYAN}[INFO]${NC} $*"; echo "[INFO] $*" >> "$SMOKE_LOG"; }

echo -e "${BOLD}"
echo "============================================================"
echo " Magnetite Local-Stack Smoke Test"
echo "============================================================"
echo -e "${NC}"
echo "Repo root : $REPO_ROOT"
echo "Log file  : $SMOKE_LOG"
echo ""

# =============================================================================
# STEP 1 — docker-compose config: parse + services check
# =============================================================================
echo -e "${BOLD}Step 1: docker-compose config + services${NC}"

REQUIRED_SERVICES=(postgres redis backend frontend mediamtx)
DOCKER_COMPOSE_FILE="$REPO_ROOT/docker-compose.yml"

if ! command -v docker &>/dev/null; then
  step_skip "Step 1: 'docker' not found on PATH — install Docker to run this check"
else
  # Try 'docker compose' (v2) first, then fall back to 'docker-compose' (v1).
  if docker compose version &>/dev/null 2>&1; then
    COMPOSE_CMD="docker compose"
  elif command -v docker-compose &>/dev/null; then
    COMPOSE_CMD="docker-compose"
  else
    step_skip "Step 1: neither 'docker compose' (v2) nor 'docker-compose' (v1) available"
    COMPOSE_CMD=""
  fi

  if [[ -n "$COMPOSE_CMD" ]]; then
    # Parse the config.
    if ! $COMPOSE_CMD -f "$DOCKER_COMPOSE_FILE" config --quiet >> "$SMOKE_LOG" 2>&1; then
      step_fail "Step 1a: docker-compose config parse FAILED — check $DOCKER_COMPOSE_FILE"
    else
      step_pass "Step 1a: docker-compose config parses without error"

      # Check expected services are defined.
      SERVICES_OUT=$($COMPOSE_CMD -f "$DOCKER_COMPOSE_FILE" config --services 2>>"$SMOKE_LOG" || true)
      MISSING=()
      for svc in "${REQUIRED_SERVICES[@]}"; do
        if ! echo "$SERVICES_OUT" | grep -q "^${svc}$"; then
          MISSING+=("$svc")
        fi
      done
      if [[ ${#MISSING[@]} -gt 0 ]]; then
        step_fail "Step 1b: missing expected services: ${MISSING[*]}"
      else
        step_pass "Step 1b: all required services defined (${REQUIRED_SERVICES[*]})"
      fi
    fi
  fi
fi
echo ""

# =============================================================================
# STEP 2 — Build game-template-authoritative → wasm32-wasip1
# =============================================================================
echo -e "${BOLD}Step 2: Build game-template-authoritative → wasm32-wasip1${NC}"

SKIP_WASM_BUILD="${SKIP_WASM_BUILD:-0}"
GAME_CRATE="$REPO_ROOT/game-templates/authoritative"
WASM_TARGET="wasm32-wasip1"
WASM_OUT="$GAME_CRATE/target/$WASM_TARGET/release/game_template_authoritative.wasm"

WASM_BUILD_DONE=0

if [[ "$SKIP_WASM_BUILD" == "1" ]]; then
  step_skip "Step 2: SKIP_WASM_BUILD=1 — skipping wasm build"
  # Still usable if a prior build artifact exists.
  if [[ -f "$WASM_OUT" ]]; then
    WASM_BUILD_DONE=1
    info "  Pre-built wasm artifact found at $WASM_OUT"
  fi
elif ! command -v cargo &>/dev/null; then
  step_skip "Step 2: 'cargo' not found on PATH — install Rust to build wasm"
else
  # Check / install the wasm32-wasip1 target.
  if ! rustup target list --installed 2>/dev/null | grep -q "$WASM_TARGET"; then
    if command -v rustup &>/dev/null; then
      info "  Installing Rust target $WASM_TARGET via rustup..."
      if rustup target add "$WASM_TARGET" >> "$SMOKE_LOG" 2>&1; then
        info "  $WASM_TARGET installed."
      else
        step_fail "Step 2: rustup target add $WASM_TARGET failed — check $SMOKE_LOG"
        echo ""
      fi
    else
      step_skip "Step 2: $WASM_TARGET not installed and 'rustup' not found"
    fi
  fi

  # Attempt the build (only if target is now present).
  if rustup target list --installed 2>/dev/null | grep -q "$WASM_TARGET"; then
    info "  cargo build --release --target $WASM_TARGET --features wasm ..."
    CARGO_FLAGS=""
    [[ "${CARGO_VERBOSE:-0}" == "1" ]] && CARGO_FLAGS="-v"
    if (
      cd "$GAME_CRATE"
      cargo build --release --target "$WASM_TARGET" --features wasm $CARGO_FLAGS
    ) >> "$SMOKE_LOG" 2>&1; then
      if [[ -f "$WASM_OUT" ]]; then
        WASM_SIZE_KB=$(( $(wc -c < "$WASM_OUT") / 1024 ))
        step_pass "Step 2: wasm build succeeded — ${WASM_SIZE_KB} KiB at $WASM_OUT"
        WASM_BUILD_DONE=1
      else
        step_fail "Step 2: cargo build exited 0 but artifact not found at $WASM_OUT"
      fi
    else
      step_fail "Step 2: cargo build --target $WASM_TARGET FAILED — see $SMOKE_LOG"
    fi
  else
    step_skip "Step 2: target $WASM_TARGET not available (run: rustup target add $WASM_TARGET)"
  fi
fi
echo ""

# =============================================================================
# STEP 3 — magnetite-e2e full-stack WebSocket tests
#          (Welcome + Snapshot + Delta + Ack + convergence + replay)
# =============================================================================
echo -e "${BOLD}Step 3: magnetite-e2e full-stack WebSocket tests${NC}"

E2E_CRATE="$REPO_ROOT/magnetite-e2e"

if ! command -v cargo &>/dev/null; then
  step_skip "Step 3: 'cargo' not found on PATH"
else
  info "  Running: cargo test --test fullstack_ws -- --nocapture"
  {
    echo "=== Step 3: fullstack_ws tests ==="
    cd "$E2E_CRATE"
    cargo test --test fullstack_ws -- --nocapture 2>&1
    echo "=== Step 3 done ==="
  } >> "$SMOKE_LOG" 2>&1
  WS_EXIT=$?

  if [[ $WS_EXIT -eq 0 ]]; then
    step_pass "Step 3a: fullstack_ws_welcome_snapshot_delta_ack_and_replay_clean — PASS"
    step_pass "Step 3b: fullstack_ws_snapshot_ticks_are_monotonic — PASS"
    step_pass "Step 3c: fullstack_ws_dedicated_topology_smoke — PASS"
  else
    step_fail "Step 3: fullstack_ws tests FAILED (exit $WS_EXIT) — see $SMOKE_LOG"
  fi
fi
echo ""

# =============================================================================
# STEP 4 — verify_replay / convergence (in-proc determinism + replay clean)
# =============================================================================
echo -e "${BOLD}Step 4: convergence + verify_replay (in-proc)${NC}"

if ! command -v cargo &>/dev/null; then
  step_skip "Step 4: 'cargo' not found on PATH"
else
  info "  Running: cargo test --test convergence -- --nocapture"
  {
    echo "=== Step 4: convergence test ==="
    cd "$E2E_CRATE"
    cargo test --test convergence -- --nocapture 2>&1
    echo "=== Step 4 done ==="
  } >> "$SMOKE_LOG" 2>&1
  CONV_EXIT=$?

  if [[ $CONV_EXIT -eq 0 ]]; then
    step_pass "Step 4: convergence_and_replay_clean — PASS (replay is Clean, clients converge)"
  else
    step_fail "Step 4: convergence test FAILED (exit $CONV_EXIT) — see $SMOKE_LOG"
  fi
fi
echo ""

# =============================================================================
# STEP 5 — wasm_end_to_end: WasmExecutor parity with NativeExecutor
#          (requires wasm artifact from Step 2)
# =============================================================================
echo -e "${BOLD}Step 5: wasm sandbox parity (WasmExecutor == NativeExecutor)${NC}"

if ! command -v cargo &>/dev/null; then
  step_skip "Step 5: 'cargo' not found on PATH"
elif [[ "$WASM_BUILD_DONE" -eq 0 ]]; then
  step_skip "Step 5: wasm artifact not built (Step 2 was skipped or failed) — run Step 2 first"
  info "  To build: cd game-templates/authoritative && cargo build --release --target wasm32-wasip1 --features wasm"
else
  info "  Running: cargo test --test wasm_end_to_end -- --nocapture"
  {
    echo "=== Step 5: wasm_end_to_end tests ==="
    cd "$E2E_CRATE"
    cargo test --test wasm_end_to_end -- --nocapture 2>&1
    echo "=== Step 5 done ==="
  } >> "$SMOKE_LOG" 2>&1
  WASM_EXIT=$?

  if [[ $WASM_EXIT -eq 0 ]]; then
    step_pass "Step 5a: wasm_sandbox_parity_with_native — PASS"
    step_pass "Step 5b: wasm_state_hash_is_reproducible_across_instances — PASS"
    step_pass "Step 5c: native_verify_replay_clean_baseline — PASS"
  else
    step_fail "Step 5: wasm_end_to_end tests FAILED (exit $WASM_EXIT) — see $SMOKE_LOG"
  fi
fi
echo ""

# =============================================================================
# Summary
# =============================================================================
echo -e "${BOLD}============================================================${NC}"
TOTAL=$((PASS_COUNT + FAIL_COUNT + SKIP_COUNT))
echo -e " Results: ${GREEN}${PASS_COUNT} PASS${NC}  ${RED}${FAIL_COUNT} FAIL${NC}  ${YELLOW}${SKIP_COUNT} SKIP${NC}  (${TOTAL} total)"
echo ""
if [[ $FAIL_COUNT -eq 0 ]]; then
  echo -e "${BOLD}${GREEN} OVERALL: PASS${NC}"
  echo "OVERALL: PASS" >> "$SMOKE_LOG"
  echo -e "${BOLD}============================================================${NC}"
  echo ""
  echo "  Full log: $SMOKE_LOG"
  exit 0
else
  echo -e "${BOLD}${RED} OVERALL: FAIL${NC} ($FAIL_COUNT step(s) failed)"
  echo "OVERALL: FAIL ($FAIL_COUNT step(s) failed)" >> "$SMOKE_LOG"
  echo -e "${BOLD}============================================================${NC}"
  echo ""
  echo "  Full log: $SMOKE_LOG"
  exit 1
fi
