#!/usr/bin/env bash
# =============================================================================
# scripts/build-game.sh — Build a Magnetite game from source to deployable WASM
#
# Called by the GitHub Actions game-ci workflow and can be invoked locally.
#
# Usage:
#   GAME_DIR=./game-template ./scripts/build-game.sh
#   GAME_DIR=/path/to/your-game ./scripts/build-game.sh
#
# Environment variables:
#   GAME_DIR       — directory containing the game's Cargo.toml  [required]
#   GAME_OUT_DIR   — output directory for dist artifacts          [default: $GAME_DIR/dist]
#   RUST_PROFILE   — "release" or "debug"                        [default: release]
#   SKIP_WASM_OPT  — set to "1" to skip wasm-opt post-processing [default: 0]
#
# Output structure:
#   $GAME_OUT_DIR/
#     game.js           — wasm-bindgen ES module glue
#     game_bg.wasm      — (optionally optimised) Wasm binary
#     game_bg.wasm.d.ts — TypeScript declarations
#     game.d.ts
# =============================================================================

set -euo pipefail

# ── Colour helpers ───────────────────────────────────────────────────────────
info()  { printf '\033[36m[game-build]\033[0m %s\n' "$*"; }
ok()    { printf '\033[32m[game-build]\033[0m %s\n' "$*"; }
warn()  { printf '\033[33m[game-build]\033[0m WARNING: %s\n' "$*" >&2; }
err()   { printf '\033[31m[game-build]\033[0m ERROR: %s\n'   "$*" >&2; exit 1; }

# ── Validate inputs ──────────────────────────────────────────────────────────
GAME_DIR="${GAME_DIR:-}"
[[ -n "$GAME_DIR" ]] || err "GAME_DIR is not set. Usage: GAME_DIR=./game-template $0"
[[ -d "$GAME_DIR" ]] || err "GAME_DIR '$GAME_DIR' does not exist or is not a directory."
[[ -f "$GAME_DIR/Cargo.toml" ]] || err "No Cargo.toml found in '$GAME_DIR'."

GAME_DIR="$(cd "$GAME_DIR" && pwd)"  # absolute path
GAME_OUT_DIR="${GAME_OUT_DIR:-$GAME_DIR/dist}"
RUST_PROFILE="${RUST_PROFILE:-release}"
SKIP_WASM_OPT="${SKIP_WASM_OPT:-0}"

PROFILE_FLAG=""
[[ "$RUST_PROFILE" == "release" ]] && PROFILE_FLAG="--release"

TARGET="wasm32-unknown-unknown"

# Derive crate name from Cargo.toml (underscores, not hyphens)
CRATE_NAME="$(grep -m1 '^name' "$GAME_DIR/Cargo.toml" | sed 's/.*= *"\(.*\)"/\1/' | tr '-' '_')"
info "Crate: $CRATE_NAME | Profile: $RUST_PROFILE | Target: $TARGET"
info "Output: $GAME_OUT_DIR"

# ── Check toolchain ──────────────────────────────────────────────────────────
if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
  info "Adding Rust target $TARGET…"
  rustup target add "$TARGET"
fi

if ! command -v wasm-bindgen &>/dev/null; then
  err "wasm-bindgen CLI not found. Install: cargo install wasm-bindgen-cli"
fi

# ── Build ────────────────────────────────────────────────────────────────────
info "Building $CRATE_NAME for $TARGET ($RUST_PROFILE)…"
(
  cd "$GAME_DIR"
  cargo build \
    --target "$TARGET" \
    $PROFILE_FLAG \
    --no-default-features \
    --features wasm \
    2>&1
)

WASM_FILE="$GAME_DIR/target/$TARGET/$RUST_PROFILE/${CRATE_NAME}.wasm"
[[ -f "$WASM_FILE" ]] || err "Expected wasm file not found: $WASM_FILE"
info "Wasm binary: $(du -h "$WASM_FILE" | cut -f1) — $WASM_FILE"

# ── wasm-bindgen ────────────────────────────────────────────────────────────
info "Running wasm-bindgen…"
mkdir -p "$GAME_OUT_DIR"

wasm-bindgen \
  --target web \
  --out-dir "$GAME_OUT_DIR" \
  --out-name game \
  "$WASM_FILE" \
  2>&1

# ── wasm-opt ────────────────────────────────────────────────────────────────
if [[ "$SKIP_WASM_OPT" != "1" ]] && command -v wasm-opt &>/dev/null && [[ "$RUST_PROFILE" == "release" ]]; then
  WASM_OUT="$GAME_OUT_DIR/game_bg.wasm"
  BEFORE="$(du -h "$WASM_OUT" | cut -f1)"
  info "Optimising with wasm-opt (-Oz)…"
  wasm-opt -Oz --output "$WASM_OUT" "$WASM_OUT" 2>&1
  AFTER="$(du -h "$WASM_OUT" | cut -f1)"
  ok "wasm-opt: $BEFORE → $AFTER"
elif command -v wasm-opt &>/dev/null || [[ "$RUST_PROFILE" == "release" ]]; then
  warn "Skipping wasm-opt (SKIP_WASM_OPT=$SKIP_WASM_OPT or not a release build)."
fi

# ── Summary ──────────────────────────────────────────────────────────────────
ok "Build complete."
echo ""
info "Artifacts in $GAME_OUT_DIR:"
ls -lh "$GAME_OUT_DIR/"
echo ""
info "Serve locally: cd $GAME_DIR && python3 -m http.server 8080"
info "Then open:     http://localhost:8080"
