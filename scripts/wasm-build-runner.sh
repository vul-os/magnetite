#!/usr/bin/env bash
# =============================================================================
# scripts/wasm-build-runner.sh — Magnetite authoritative-WASM build runner
#
# This script builds Rust game crates for the MOAT authoritative runtime target
# (wasm32-wasip1 + the mag_* ABI) and POSTs the resulting game.wasm artifact
# back to the Magnetite distribution API.
#
# Use this script when you have registered a game repo with `magnetite register`
# (magnetite-cli) and want to compile and upload it using a self-hosted machine
# instead of a GitHub Actions CI runner.
#
# DIFFERENCE FROM run-wasm-build.sh
# ──────────────────────────────────
#   run-wasm-build.sh  — builds for wasm32-unknown-unknown + wasm-bindgen
#                         (browser JS glue, the wasm-pack / Bevy WASM path)
#   wasm-build-runner.sh — builds for wasm32-wasip1 + mag_* ABI
#                          (the MOAT authoritative server runtime path)
#
# The mag_* ABI is the interface between the sandbox host (magnetite-sandbox)
# and the compiled game WASM module.  It does NOT use wasm-bindgen; the module
# exports bare C-ABI functions (mag_init, mag_step, mag_snapshot, …).
#
# ─── Prerequisites ───────────────────────────────────────────────────────────
#   - Rust toolchain (rustup) with the wasm32-wasip1 target
#       rustup target add wasm32-wasip1
#   - wasm-opt (optional, binaryen — install via `brew install binaryen` or
#       `apt install binaryen`)
#   - curl + jq
#   - Git
#
# ─── Environment ─────────────────────────────────────────────────────────────
# Required:
#   MAGNETITE_API_URL    — Backend base URL, e.g. https://api.magnetite.gg
#   BUILD_RUNNER_TOKEN   — API bearer token with build-runner scope
#                          (generate with: magnetite token create --scope build-runner)
#
# Required for single-shot mode (-g / -p):
#   (pass game ID and repo path as arguments — see usage below)
#
# Optional:
#   ARTIFACT_BUCKET      — S3 bucket name to upload the compiled .wasm
#   ARTIFACT_PREFIX      — S3 key prefix [default: artifacts]
#   CDN_BASE_URL         — Public CDN base URL (e.g. https://cdn.magnetite.gg)
#   BUILD_WORKSPACE      — Scratch directory [default: /tmp/magnetite-wasip1-builds]
#   RUST_PROFILE         — release | debug [default: release]
#   SKIP_WASM_OPT        — 1 to skip wasm-opt [default: 0]
#   POLL_INTERVAL        — Seconds between polls (daemon mode) [default: 30]
#   GITHUB_TOKEN         — PAT for cloning private repos
#
# ─── Usage ────────────────────────────────────────────────────────────────────
#   # Daemon mode — polls the API for queued builds, processes each one:
#   export MAGNETITE_API_URL=https://api.magnetite.gg
#   export BUILD_RUNNER_TOKEN=tok_...
#   ./scripts/wasm-build-runner.sh
#
#   # Single-shot mode — build a local repo and upload the result:
#   ./scripts/wasm-build-runner.sh -g <game-uuid> -p /path/to/game-crate
#
#   # Single-shot without upload (offline test):
#   ./scripts/wasm-build-runner.sh -g <game-uuid> -p /path/to/game-crate --dry-run
#
# ─── How it works (daemon mode) ──────────────────────────────────────────────
# 1. Poll GET $MAGNETITE_API_URL/api/v1/distribution/builds/pending
# 2. For each queued build:
#    a. Clone the registered repository at the given commit SHA, or use a
#       local path when running in single-shot mode.
#    b. cargo build --release --target wasm32-wasip1 --features wasm
#    c. Optionally run wasm-opt -Oz on the output.
#    d. Upload the .wasm artifact to S3 (or leave at local path).
#    e. POST the result to /api/v1/distribution/:game_id/versions (or the
#       build-report endpoint when invoked from a queued build job).
# 3. The backend sets artifact_url and build_status = 'success'.
#    magnetite-sandbox loads game.wasm from artifact_url at session start.
#
# ─── Bucket D note ───────────────────────────────────────────────────────────
# This runner is the "external runner" dependency documented in DECISIONS.md §4c
# and docs/self-hosting/local-infra.md.  The backend correctly keeps
# build_status='queued' (never fakes 'success') until this script reports back.
# =============================================================================

set -euo pipefail

# ── Colour helpers ─────────────────────────────────────────────────────────
info()  { printf '\033[36m[wasip1-runner]\033[0m %s\n' "$*"; }
ok()    { printf '\033[32m[wasip1-runner]\033[0m %s\n' "$*"; }
warn()  { printf '\033[33m[wasip1-runner]\033[0m WARNING: %s\n' "$*" >&2; }
err()   { printf '\033[31m[wasip1-runner]\033[0m ERROR: %s\n'   "$*" >&2; }

# ── Default config ──────────────────────────────────────────────────────────
MAGNETITE_API_URL="${MAGNETITE_API_URL:-}"
BUILD_RUNNER_TOKEN="${BUILD_RUNNER_TOKEN:-}"
ARTIFACT_BUCKET="${ARTIFACT_BUCKET:-}"
ARTIFACT_PREFIX="${ARTIFACT_PREFIX:-artifacts}"
CDN_BASE_URL="${CDN_BASE_URL:-}"
BUILD_WORKSPACE="${BUILD_WORKSPACE:-/tmp/magnetite-wasip1-builds}"
RUST_PROFILE="${RUST_PROFILE:-release}"
SKIP_WASM_OPT="${SKIP_WASM_OPT:-0}"
POLL_INTERVAL="${POLL_INTERVAL:-30}"
GITHUB_TOKEN="${GITHUB_TOKEN:-}"

TARGET="wasm32-wasip1"
SINGLE_SHOT_GAME_ID=""
SINGLE_SHOT_PATH=""
DRY_RUN="0"

# ── Argument parsing ────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    -g|--game-id)  SINGLE_SHOT_GAME_ID="$2"; shift 2 ;;
    -p|--path)     SINGLE_SHOT_PATH="$2";    shift 2 ;;
    --dry-run)     DRY_RUN="1";              shift ;;
    -h|--help)
      grep '^#' "$0" | grep -v '^#!/' | sed 's/^# \{0,2\}//'
      exit 0 ;;
    *) err "Unknown argument: $1"; exit 1 ;;
  esac
done

# ── Prerequisite checks ─────────────────────────────────────────────────────
check_prereqs() {
  local missing=()
  command -v rustup &>/dev/null || missing+=("rustup")
  command -v cargo  &>/dev/null || missing+=("cargo")
  command -v curl   &>/dev/null || missing+=("curl")
  command -v jq     &>/dev/null || missing+=("jq")
  command -v git    &>/dev/null || missing+=("git")

  if [[ ${#missing[@]} -gt 0 ]]; then
    err "Missing prerequisites: ${missing[*]}"
    exit 1
  fi

  if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
    info "Adding Rust target $TARGET..."
    rustup target add "$TARGET"
  fi

  if [[ -n "$ARTIFACT_BUCKET" ]] && ! command -v aws &>/dev/null && ! command -v s3cmd &>/dev/null; then
    warn "ARTIFACT_BUCKET set but neither 'aws' nor 's3cmd' found — artifacts will be local only."
    ARTIFACT_BUCKET=""
  fi

  info "Prerequisites OK. Target: $TARGET | Profile: $RUST_PROFILE"
}

# ── Validate required env ───────────────────────────────────────────────────
check_env() {
  local errors=0
  if [[ -z "$MAGNETITE_API_URL" && "$DRY_RUN" != "1" ]]; then
    err "MAGNETITE_API_URL is not set."
    errors=$((errors + 1))
  fi
  if [[ -z "$BUILD_RUNNER_TOKEN" && "$DRY_RUN" != "1" ]]; then
    err "BUILD_RUNNER_TOKEN is not set. Generate one with: magnetite token create --scope build-runner"
    errors=$((errors + 1))
  fi
  [[ $errors -eq 0 ]] || exit 1
  MAGNETITE_API_URL="${MAGNETITE_API_URL%/}"
}

# ── S3 / CDN upload ─────────────────────────────────────────────────────────
# Returns the public artifact URL via stdout (CDN URL or file:// path).
upload_artifact() {
  local local_path="$1"
  local key_suffix="$2"
  local s3_key="${ARTIFACT_PREFIX}/${key_suffix}"

  if [[ -z "$ARTIFACT_BUCKET" ]]; then
    echo "file://$local_path"
    return 0
  fi

  if command -v aws &>/dev/null; then
    aws s3 cp "$local_path" "s3://${ARTIFACT_BUCKET}/${s3_key}" \
      --content-type "application/wasm" \
      --acl public-read >/dev/null 2>&1
  elif command -v s3cmd &>/dev/null; then
    s3cmd put "$local_path" "s3://${ARTIFACT_BUCKET}/${s3_key}" \
      --mime-type "application/wasm" \
      --acl-public >/dev/null 2>&1
  else
    echo "file://$local_path"
    return 0
  fi

  if [[ -n "$CDN_BASE_URL" ]]; then
    echo "${CDN_BASE_URL%/}/${s3_key}"
  else
    echo "https://${ARTIFACT_BUCKET}.s3.amazonaws.com/${s3_key}"
  fi
}

# ── Report result back to the distribution API ──────────────────────────────
report_result() {
  local game_id="$1"
  local artifact_url="$2"
  local sha256_hash="$3"
  local file_size_bytes="$4"
  local log_output="$5"
  local outcome="$6"   # success | failed

  if [[ "$DRY_RUN" == "1" ]]; then
    ok "[dry-run] Would POST result to $MAGNETITE_API_URL/api/v1/distribution/${game_id}/builds/report"
    ok "[dry-run] artifact_url=$artifact_url  sha256=${sha256_hash:0:16}...  size=${file_size_bytes}B  outcome=$outcome"
    return 0
  fi

  # Truncate log to 60 KiB
  if [[ ${#log_output} -gt 61440 ]]; then
    log_output="...[truncated]...${log_output: -61440}"
  fi

  local payload
  payload="$(jq -n \
    --arg outcome      "$outcome" \
    --arg artifact_url "$artifact_url" \
    --arg sha256_hash  "$sha256_hash" \
    --argjson file_size_bytes "${file_size_bytes:-null}" \
    --arg log_output   "$log_output" \
    '{
       outcome:          $outcome,
       artifact_url:     (if $artifact_url != "" then $artifact_url else null end),
       sha256_hash:      (if $sha256_hash  != "" then $sha256_hash  else null end),
       file_size_bytes:  $file_size_bytes,
       log_output:       (if $log_output   != "" then $log_output   else null end),
       runtime_target:   "wasm32-wasip1"
     }')"

  curl -sf \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $BUILD_RUNNER_TOKEN" \
    -d "$payload" \
    "$MAGNETITE_API_URL/api/v1/distribution/${game_id}/builds/report" \
  && ok "Reported $outcome for game $game_id" \
  || warn "Failed to report result for game $game_id (will retry next poll)"
}

# ── Build a single game crate ────────────────────────────────────────────────
# $1: game_id (UUID string)
# $2: source directory (path to the crate root containing Cargo.toml)
# $3: optional commit_sha (for logging; pass "" if not from a CI job)
build_game() {
  local game_id="$1"
  local src_dir="$2"
  local commit_sha="${3:-local}"

  info "=== Building game $game_id (${commit_sha:0:8}) from $src_dir ==="

  if [[ ! -f "$src_dir/Cargo.toml" ]]; then
    err "No Cargo.toml in $src_dir"
    report_result "$game_id" "" "" "" "No Cargo.toml found in $src_dir" "failed"
    return 1
  fi

  local work_dir="$BUILD_WORKSPACE/${game_id}"
  mkdir -p "$work_dir"
  local log_file="$work_dir/build.log"
  local exit_code=0
  local out_dir="$work_dir/dist"
  mkdir -p "$out_dir"

  {
    echo "=== wasm32-wasip1 authoritative build ==="
    echo "game_id:     $game_id"
    echo "commit_sha:  $commit_sha"
    echo "src_dir:     $src_dir"
    echo "profile:     $RUST_PROFILE"
    echo ""

    # Determine profile flag
    local profile_flag=""
    [[ "$RUST_PROFILE" == "release" ]] && profile_flag="--release"

    # Derive crate name from Cargo.toml (hyphens → underscores)
    local crate_name
    crate_name="$(grep -m1 '^name' "$src_dir/Cargo.toml" \
      | sed 's/.*= *"\(.*\)"/\1/' | tr '-' '_')"
    echo "crate_name:  $crate_name"
    echo ""

    # Build for wasm32-wasip1 with the --features wasm flag (exposes mag_* ABI exports).
    # The game crate must have a [features] wasm section in Cargo.toml that enables
    # the mag_* export functions (see game-templates/authoritative/src/wasm_abi.rs).
    echo "=== cargo build --target wasm32-wasip1 ==="
    (
      cd "$src_dir"
      cargo build \
        --target "$TARGET" \
        $profile_flag \
        --no-default-features \
        --features wasm \
        2>&1
    )

    local wasm_file="$src_dir/target/$TARGET/$RUST_PROFILE/${crate_name}.wasm"
    if [[ ! -f "$wasm_file" ]]; then
      echo "ERROR: expected wasm output not found: $wasm_file" >&2
      exit 1
    fi
    echo "Wasm binary: $(du -h "$wasm_file" | cut -f1) at $wasm_file"

    # Copy to dist/ and rename to the canonical game.wasm name.
    cp "$wasm_file" "$out_dir/game.wasm"

    # ── wasm-opt (optional) ──────────────────────────────────────────────────
    # wasm-opt is safe to use on WASI modules; it treats them as standard Wasm.
    if [[ "$SKIP_WASM_OPT" != "1" ]] \
       && command -v wasm-opt &>/dev/null \
       && [[ "$RUST_PROFILE" == "release" ]]; then
      echo "=== Running wasm-opt -Oz ==="
      local before after
      before="$(du -h "$out_dir/game.wasm" | cut -f1)"
      wasm-opt -Oz --output "$out_dir/game.wasm" "$out_dir/game.wasm" 2>&1
      after="$(du -h "$out_dir/game.wasm" | cut -f1)"
      echo "wasm-opt: $before → $after"
    fi

    echo ""
    echo "=== Build complete ==="
    ls -lh "$out_dir/"

  } > "$log_file" 2>&1 || exit_code=$?

  local log_output
  log_output="$(cat "$log_file")"

  if [[ $exit_code -ne 0 ]]; then
    err "Build $game_id FAILED (exit $exit_code)"
    report_result "$game_id" "" "" "" "$log_output" "failed"
    rm -rf "$work_dir"
    return 1
  fi

  ok "Build $game_id succeeded."

  local wasm_out="$out_dir/game.wasm"
  local artifact_url sha256_hash file_size_bytes

  sha256_hash="$(shasum -a 256 "$wasm_out" 2>/dev/null | awk '{print $1}' || true)"
  file_size_bytes="$(wc -c < "$wasm_out" 2>/dev/null | tr -d '[:space:]' || echo "")"

  # Upload to S3 / CDN (or local path if no bucket configured).
  local s3_key="${game_id}/${commit_sha}/game.wasm"
  artifact_url="$(upload_artifact "$wasm_out" "$s3_key")"

  ok "Artifact: $artifact_url"
  ok "SHA-256:  ${sha256_hash:0:16}..."
  ok "Size:     ${file_size_bytes} bytes"

  report_result "$game_id" "$artifact_url" "$sha256_hash" "$file_size_bytes" "$log_output" "success"

  rm -rf "$work_dir"
}

# ── Fetch pending builds from the distribution API ──────────────────────────
fetch_pending() {
  curl -sf \
    -H "Accept: application/json" \
    -H "Authorization: Bearer $BUILD_RUNNER_TOKEN" \
    "$MAGNETITE_API_URL/api/v1/distribution/builds/pending" \
    || true
}

# ── Clone a repo at a specific commit SHA ───────────────────────────────────
# $1: repository slug (owner/name)
# $2: commit SHA
# $3: destination directory
clone_repo() {
  local repository="$1"
  local commit_sha="$2"
  local dest="$3"

  local clone_url
  if [[ -n "$GITHUB_TOKEN" ]]; then
    clone_url="https://x-access-token:${GITHUB_TOKEN}@github.com/${repository}.git"
  else
    clone_url="https://github.com/${repository}.git"
  fi

  git clone --depth=50 --no-tags "$clone_url" "$dest" 2>&1
  (cd "$dest" && git checkout "$commit_sha" 2>&1)
}

# ── Main ─────────────────────────────────────────────────────────────────────
check_prereqs
check_env
mkdir -p "$BUILD_WORKSPACE"

# Single-shot mode: build a specific local directory.
if [[ -n "$SINGLE_SHOT_GAME_ID" || -n "$SINGLE_SHOT_PATH" ]]; then
  if [[ -z "$SINGLE_SHOT_GAME_ID" || -z "$SINGLE_SHOT_PATH" ]]; then
    err "Both -g/--game-id and -p/--path are required for single-shot mode."
    exit 1
  fi
  info "Single-shot mode: game=$SINGLE_SHOT_GAME_ID  path=$SINGLE_SHOT_PATH"
  build_game "$SINGLE_SHOT_GAME_ID" "$SINGLE_SHOT_PATH" "local"
  exit $?
fi

# Daemon mode: poll the API for queued builds.
info "Magnetite WASM-wasip1 build runner started (daemon mode)."
info "  API:       $MAGNETITE_API_URL"
info "  Workspace: $BUILD_WORKSPACE"
info "  Bucket:    ${ARTIFACT_BUCKET:-<none — local file:// paths>}"
info "  CDN:       ${CDN_BASE_URL:-<none>}"
info "  Profile:   $RUST_PROFILE"
echo ""

while true; do
  info "Checking for queued builds..."

  pending_json="$(fetch_pending)"

  if [[ -z "$pending_json" ]] || [[ "$(echo "$pending_json" | jq 'length' 2>/dev/null)" == "0" ]]; then
    info "No pending builds. Sleeping ${POLL_INTERVAL}s..."
    sleep "$POLL_INTERVAL"
    continue
  fi

  build_count="$(echo "$pending_json" | jq 'length')"
  info "Found $build_count queued build(s)."

  echo "$pending_json" | jq -c '.[]' | while read -r build_json; do
    game_id="$(echo "$build_json"    | jq -r '.game_id')"
    repository="$(echo "$build_json" | jq -r '.repository // empty')"
    commit_sha="$(echo "$build_json" | jq -r '.commit_sha // empty')"

    if [[ -n "$repository" && -n "$commit_sha" ]]; then
      # CI-style: clone from GitHub and build.
      clone_dir="$BUILD_WORKSPACE/clone_${game_id}"
      mkdir -p "$clone_dir"
      if clone_repo "$repository" "$commit_sha" "$clone_dir"; then
        build_game "$game_id" "$clone_dir" "$commit_sha" || true
      else
        err "Clone failed for $repository@${commit_sha:0:8}"
        report_result "$game_id" "" "" "" "git clone failed" "failed"
      fi
      rm -rf "$clone_dir"
    else
      warn "Build job $game_id has no repository/commit_sha — skipping."
    fi
  done

  info "Batch done. Sleeping ${POLL_INTERVAL}s..."
  sleep "$POLL_INTERVAL"
done
