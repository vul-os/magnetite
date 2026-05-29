#!/usr/bin/env bash
# =============================================================================
# build.sh — Magnetite game template: Rust → WASM build script
#
# Usage:
#   ./build.sh                  # release build → dist/
#   ./build.sh --dev            # debug build   → dist/
#   ./build.sh --check          # cargo check only (fast, no wasm output)
#   ./build.sh --serve          # build + start a local HTTP server
#   GAME_OUT_DIR=./my-out ./build.sh   # custom output directory
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli    # must match wasm-bindgen crate version
#   (optional) cargo install basic-http-server  OR  python3 in PATH
#
# Output:
#   $GAME_OUT_DIR/
#     game.js          — wasm-bindgen ES module glue
#     game_bg.wasm     — optimised Wasm binary
#     game_bg.wasm.d.ts
#     game.d.ts
#
#   Copy index.html next to dist/ (it already imports ./dist/game.js).
# =============================================================================

set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────────────
CRATE_NAME="magnetite_game_template"
TARGET="wasm32-unknown-unknown"
GAME_OUT_DIR="${GAME_OUT_DIR:-./dist}"
PROFILE="release"
PROFILE_FLAG="--release"

# ── Flags ───────────────────────────────────────────────────────────────────
DO_CHECK=0
DO_SERVE=0
for arg in "$@"; do
  case "$arg" in
    --dev)    PROFILE="debug";  PROFILE_FLAG="" ;;
    --check)  DO_CHECK=1 ;;
    --serve)  DO_SERVE=1 ;;
    --help|-h)
      sed -n '2,20p' "$0" | grep '^#' | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "Unknown flag: $arg  (use --help for usage)" >&2
      exit 1
      ;;
  esac
done

# ── Helpers ─────────────────────────────────────────────────────────────────
info()  { printf '\033[36m[build]\033[0m %s\n' "$*"; }
ok()    { printf '\033[32m[build]\033[0m %s\n' "$*"; }
warn()  { printf '\033[33m[build]\033[0m WARNING: %s\n' "$*" >&2; }
err()   { printf '\033[31m[build]\033[0m ERROR: %s\n'   "$*" >&2; exit 1; }

# ── Verify toolchain ─────────────────────────────────────────────────────────
if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
  info "Adding Rust target $TARGET…"
  rustup target add "$TARGET" || err "Failed to add $TARGET. Run: rustup target add $TARGET"
fi

# ── cargo check (fast path) ──────────────────────────────────────────────────
if [[ $DO_CHECK -eq 1 ]]; then
  info "Running cargo check --no-default-features…"
  cargo check --no-default-features 2>&1
  ok "cargo check passed."
  exit 0
fi

# ── wasm-bindgen-cli version check ───────────────────────────────────────────
if ! command -v wasm-bindgen &>/dev/null; then
  warn "wasm-bindgen CLI not found."
  info "Install it with: cargo install wasm-bindgen-cli --version \$(grep 'wasm-bindgen' Cargo.toml | head -1 | grep -oE '\"[0-9.]+\"' | tr -d '\"')"
  err "wasm-bindgen-cli is required for the WASM build."
fi

# The wasm-bindgen CLI version must match the crate version exactly.
CLI_VER="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')"
CRATE_VER="$(grep 'wasm-bindgen' Cargo.toml | grep -v '^#' | grep -oE '"[0-9]+\.[0-9]+(\.[0-9]+)?"' | head -1 | tr -d '"')"
if [[ "$CLI_VER" != "$CRATE_VER" ]]; then
  warn "wasm-bindgen CLI version ($CLI_VER) != crate version ($CRATE_VER)."
  warn "Run: cargo install wasm-bindgen-cli --version $CRATE_VER --force"
fi

# ── Build ────────────────────────────────────────────────────────────────────
info "Building $CRATE_NAME ($PROFILE) for $TARGET…"

cargo build \
  --target "$TARGET" \
  $PROFILE_FLAG \
  --no-default-features \
  --features wasm \
  2>&1

WASM_FILE="target/$TARGET/$PROFILE/${CRATE_NAME}.wasm"
if [[ ! -f "$WASM_FILE" ]]; then
  err "Expected $WASM_FILE not found after cargo build."
fi

# ── wasm-bindgen ────────────────────────────────────────────────────────────
info "Running wasm-bindgen…"
mkdir -p "$GAME_OUT_DIR"

wasm-bindgen \
  --target web \
  --out-dir "$GAME_OUT_DIR" \
  --out-name game \
  "$WASM_FILE" \
  2>&1

ok "WASM build complete → $GAME_OUT_DIR/"
ls -lh "$GAME_OUT_DIR/"

# ── Optional: wasm-opt ───────────────────────────────────────────────────────
if command -v wasm-opt &>/dev/null && [[ "$PROFILE" == "release" ]]; then
  WASM_OUT="$GAME_OUT_DIR/game_bg.wasm"
  info "Optimising with wasm-opt…"
  wasm-opt -Oz --output "$WASM_OUT" "$WASM_OUT" 2>&1 && ok "wasm-opt done."
fi

# ── Serve ────────────────────────────────────────────────────────────────────
if [[ $DO_SERVE -eq 1 ]]; then
  PORT="${PORT:-8080}"
  info "Starting local server on http://localhost:$PORT …"
  info "Point your browser at http://localhost:$PORT (serves index.html + dist/)"
  if command -v basic-http-server &>/dev/null; then
    basic-http-server . --addr "127.0.0.1:$PORT"
  elif command -v python3 &>/dev/null; then
    python3 -m http.server "$PORT"
  elif command -v npx &>/dev/null; then
    npx serve . -p "$PORT" -s
  else
    err "No local HTTP server found. Install basic-http-server (cargo install basic-http-server) or python3."
  fi
fi
