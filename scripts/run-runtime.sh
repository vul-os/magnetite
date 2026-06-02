#!/usr/bin/env bash
# =============================================================================
# scripts/run-runtime.sh — Start the magnetite-runtime authoritative server
#
# The magnetite-runtime process is the authoritative game-server host that
# runs compiled WASM game modules inside the magnetite-sandbox, applying the
# mag_* ABI tick loop, delta broadcast, and ClientNet/ServerNet protocol.
#
# It listens on a WebSocket port (default: 9000) and is separate from the main
# Magnetite API backend (port 8080).  The backend matchmaking service connects
# clients here via the GAME_SERVER_WS_BASE environment variable.
#
# ─── End-to-end play flow ────────────────────────────────────────────────────
#
#   1. Developer registers a game:
#        magnetite register --repo owner/game-repo
#
#   2. Self-hosted WASM build runner compiles game.wasm:
#        ./scripts/wasm-build-runner.sh -g <game-uuid> -p /path/to/game-crate
#
#   3. Player visits the game page → clicks Play → browser fetches play manifest:
#        GET /api/v1/distribution/<game-id>/play
#        → { server_url: "ws://localhost:9000", wasm_url: "...", ... }
#
#   4. Frontend opens a WebSocket to server_url and sends ClientNet::InputFrame.
#
#   5. magnetite-runtime loads game.wasm from artifact_url, starts the tick loop,
#      and broadcasts ServerNet::{Welcome, Snapshot, Delta, Ack, Reject}.
#
# ─── Prerequisites ───────────────────────────────────────────────────────────
#   - Rust toolchain (for `cargo run`)  —  OR —  pre-built binary in PATH
#   - MAGNETITE_API_URL: backend URL (runtime fetches the play manifest here)
#
# ─── Environment ─────────────────────────────────────────────────────────────
#   RUNTIME_HOST         — Bind address [default: 127.0.0.1]
#   RUNTIME_PORT         — WebSocket listen port [default: 9000]
#   RUNTIME_WORKERS      — Tokio worker threads; 0 = auto [default: 0]
#   MAGNETITE_API_URL    — Backend URL for manifest/artifact resolution
#                          [default: http://localhost:8080]
#   RUST_LOG             — Log filter [default: info]
#
# ─── Usage ────────────────────────────────────────────────────────────────────
#   # Quick start (builds from source):
#   ./scripts/run-runtime.sh
#
#   # Custom host/port:
#   RUNTIME_PORT=9001 ./scripts/run-runtime.sh
#
#   # Pre-built binary:
#   RUNTIME_BIN=/usr/local/bin/magnetite-runtime ./scripts/run-runtime.sh
#
#   # In Docker (via docker compose up magnetite-runtime):
#   See docker-compose.override.yml — the runtime service is pre-configured.
#
# ─── Docker Compose ───────────────────────────────────────────────────────────
#   docker compose up magnetite-runtime
#   # Exposes ws://localhost:9000 and sets GAME_SERVER_WS_BASE automatically
#   # for the backend service via docker-compose.override.yml.
#
# =============================================================================

set -euo pipefail

info()  { printf '\033[36m[magnetite-runtime]\033[0m %s\n' "$*"; }
ok()    { printf '\033[32m[magnetite-runtime]\033[0m %s\n' "$*"; }
warn()  { printf '\033[33m[magnetite-runtime]\033[0m WARNING: %s\n' "$*" >&2; }
err()   { printf '\033[31m[magnetite-runtime]\033[0m ERROR: %s\n'   "$*" >&2; }

# ── Config ──────────────────────────────────────────────────────────────────
RUNTIME_HOST="${RUNTIME_HOST:-127.0.0.1}"
RUNTIME_PORT="${RUNTIME_PORT:-9000}"
RUNTIME_WORKERS="${RUNTIME_WORKERS:-0}"
MAGNETITE_API_URL="${MAGNETITE_API_URL:-http://localhost:8080}"
RUST_LOG="${RUST_LOG:-info}"
RUNTIME_BIN="${RUNTIME_BIN:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUNTIME_DIR="$REPO_ROOT/magnetite-runtime"

# ── Find / build the runtime binary ─────────────────────────────────────────
find_runtime_bin() {
  # 1. Explicit RUNTIME_BIN env.
  if [[ -n "$RUNTIME_BIN" ]]; then
    if [[ ! -x "$RUNTIME_BIN" ]]; then
      err "RUNTIME_BIN=$RUNTIME_BIN is not executable."
      exit 1
    fi
    echo "$RUNTIME_BIN"
    return 0
  fi

  # 2. Pre-built release binary in the crate target directory.
  local release_bin="$RUNTIME_DIR/target/release/magnetite-runtime"
  if [[ -x "$release_bin" ]]; then
    echo "$release_bin"
    return 0
  fi

  # 3. magnetite-runtime in PATH.
  if command -v magnetite-runtime &>/dev/null; then
    echo "$(command -v magnetite-runtime)"
    return 0
  fi

  # 4. Fall back to `cargo run` inside the crate.
  echo "cargo-run"
}

# ── Main ─────────────────────────────────────────────────────────────────────
main() {
  info "Starting magnetite-runtime authoritative game-server host"
  info "  Listen:       ws://$RUNTIME_HOST:$RUNTIME_PORT"
  info "  Workers:      ${RUNTIME_WORKERS} (0 = auto)"
  info "  API:          $MAGNETITE_API_URL"
  info "  RUST_LOG:     $RUST_LOG"
  echo ""

  local bin
  bin="$(find_runtime_bin)"

  if [[ "$bin" == "cargo-run" ]]; then
    if [[ ! -d "$RUNTIME_DIR" ]]; then
      err "magnetite-runtime crate not found at $RUNTIME_DIR"
      err "Clone the full Magnetite repository to use this script."
      exit 1
    fi

    info "No pre-built binary found. Building from source with 'cargo run'..."
    info "(This takes a minute on first run; subsequent runs use the cache.)"
    echo ""

    export RUST_LOG
    export RUNTIME_HOST
    export RUNTIME_PORT
    export RUNTIME_WORKERS
    export MAGNETITE_API_URL

    exec cargo run \
      --release \
      --manifest-path "$RUNTIME_DIR/Cargo.toml" \
      -- \
      --host "$RUNTIME_HOST" \
      --port "$RUNTIME_PORT" \
      --workers "$RUNTIME_WORKERS"
  else
    info "Using binary: $bin"
    echo ""

    export RUST_LOG
    export MAGNETITE_API_URL

    exec "$bin" \
      --host "$RUNTIME_HOST" \
      --port "$RUNTIME_PORT" \
      --workers "$RUNTIME_WORKERS"
  fi
}

main "$@"
