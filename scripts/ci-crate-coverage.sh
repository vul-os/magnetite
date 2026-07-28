#!/usr/bin/env bash
#
# Assert that EVERY Rust crate in this repo is covered by the CI matrix.
#
# Why this exists
# ---------------
# Until now `.github/workflows/ci.yml` ran cargo with `working-directory:
# backend`, so exactly ONE of the fourteen crates in this repo was gated.
# magnetite-seams, magnetite-sandbox, magnetite-anticheat, magnetite-runtime,
# magnetite-e2e, magnetite-solana-rail, magnetite-cli, game-client-bevy, the
# SDK and all four game templates compiled, tested and linted nowhere. That is
# how stale lockfiles and clippy failures survived in the tree.
#
# A matrix alone does not stop that from happening again: the next crate added
# to the repo will simply not be listed in ci/rust-crates.json, and CI will
# stay green while gating nothing new. This script closes that hole by diffing
# the crates ON DISK against the crates IN THE MANIFEST and failing on any
# difference in either direction.
#
# It FAILS CLOSED: a missing manifest, missing jq, or a zero-crate scan is an
# error, not a pass. Run it from anywhere; it locates the repo root itself.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/ci/rust-crates.json"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

command -v jq >/dev/null 2>&1 || fail "jq is not installed — crate coverage NOT verified."
[ -f "$MANIFEST" ] || fail "$MANIFEST is missing — crate coverage NOT verified."

# --- Crates the CI matrix claims to cover -----------------------------------
jq -e 'type == "array" and length > 0' "$MANIFEST" >/dev/null 2>&1 \
  || fail "$MANIFEST is not a non-empty JSON array — crate coverage NOT verified."

MANIFEST_DIRS="$(jq -r '.[].dir' "$MANIFEST" | sort -u)"
MANIFEST_COUNT="$(printf '%s\n' "$MANIFEST_DIRS" | grep -c . || true)"

# Every entry must name a directory that really has a Cargo.toml, or the
# matrix leg silently does nothing.
while IFS= read -r d; do
  [ -n "$d" ] || continue
  [ -f "$ROOT/$d/Cargo.toml" ] \
    || fail "manifest lists '$d' but $d/Cargo.toml does not exist."
done <<<"$MANIFEST_DIRS"

# --- Crates actually on disk ------------------------------------------------
DISK_DIRS="$(
  cd "$ROOT"
  find . -name Cargo.toml \
    -not -path '*/target/*' \
    -not -path '*/node_modules/*' \
    -print \
  | sed 's|/Cargo.toml$||; s|^\./||' \
  | sort -u
)"
DISK_COUNT="$(printf '%s\n' "$DISK_DIRS" | grep -c . || true)"

[ "$DISK_COUNT" -gt 0 ] \
  || fail "found 0 Cargo.toml files — the scan itself is broken; coverage NOT verified."

# --- Diff -------------------------------------------------------------------
UNGATED="$(comm -13 <(printf '%s\n' "$MANIFEST_DIRS") <(printf '%s\n' "$DISK_DIRS") || true)"
PHANTOM="$(comm -23 <(printf '%s\n' "$MANIFEST_DIRS") <(printf '%s\n' "$DISK_DIRS") || true)"

echo "Rust crate coverage"
echo "  crates on disk    : $DISK_COUNT"
echo "  crates in CI matrix: $MANIFEST_COUNT"
printf '%s\n' "$DISK_DIRS" | sed 's/^/    - /'

status=0

if [ -n "$UNGATED" ]; then
  echo >&2
  echo "FAIL: these crates exist but are NOT in the CI matrix — they are gated by NOTHING:" >&2
  printf '%s\n' "$UNGATED" | sed 's/^/    - /' >&2
  echo "Add them to ci/rust-crates.json." >&2
  status=1
fi

if [ -n "$PHANTOM" ]; then
  echo >&2
  echo "FAIL: these manifest entries have no crate on disk (the matrix leg would gate nothing):" >&2
  printf '%s\n' "$PHANTOM" | sed 's/^/    - /' >&2
  status=1
fi

if [ "$DISK_COUNT" -ne "$MANIFEST_COUNT" ]; then
  echo >&2
  echo "FAIL: coverage count mismatch — $DISK_COUNT crates on disk, $MANIFEST_COUNT in the matrix." >&2
  status=1
fi

[ "$status" -eq 0 ] || exit "$status"

echo "  OK — all $DISK_COUNT crates are covered by the CI matrix."
