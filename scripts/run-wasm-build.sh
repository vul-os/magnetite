#!/usr/bin/env bash
# =============================================================================
# scripts/run-wasm-build.sh — Magnetite self-hosted WASM build runner
#
# This script is the CI runner that picks up queued WASM build jobs from the
# Magnetite backend and executes them locally.  Run it on any machine that has:
#
#   - Rust toolchain (rustup) with `wasm32-unknown-unknown` target
#   - wasm-bindgen-cli  (`cargo install wasm-bindgen-cli`)
#   - wasm-opt          (optional, from binaryen — for release optimisation)
#   - curl + jq
#   - Git (to check out the game repo)
#   - S3-compatible upload tool (aws-cli or s3cmd) — only required if
#     ARTIFACT_BUCKET is set; otherwise the artifact path is reported as a
#     local file path (useful for debugging).
#
# ─── Environment ────────────────────────────────────────────────────────────
# Required:
#   MAGNETITE_API_URL  — Backend base URL, e.g. https://api.magnetite.gg
#
# Optional:
#   POLL_INTERVAL      — Seconds between polling for queued builds [default: 30]
#   GITHUB_TOKEN       — Personal access token used to clone private repos
#   ARTIFACT_BUCKET    — S3 bucket name to upload compiled artifacts
#                        e.g. "my-magnetite-artifacts"
#   ARTIFACT_PREFIX    — S3 key prefix [default: "artifacts"]
#   CDN_BASE_URL       — Public CDN URL prefix for the artifact bucket
#                        e.g. "https://cdn.magnetite.gg"
#                        If set, artifact_url reported back = CDN_BASE_URL/<key>
#   BUILD_WORKSPACE    — Temp directory for build checkouts [default: /tmp/magnetite-builds]
#   RUST_PROFILE       — "release" or "debug" [default: release]
#   SKIP_WASM_OPT      — "1" to skip wasm-opt [default: 0]
#
# ─── How it works ────────────────────────────────────────────────────────────
# 1. Poll GET $MAGNETITE_API_URL/api/v1/github/builds/pending every POLL_INTERVAL s.
# 2. For each queued build:
#    a. Clone the registered repository at the given commit SHA.
#    b. Run `cargo build --target wasm32-unknown-unknown` + wasm-bindgen.
#    c. Optionally run wasm-opt on the output.
#    d. Upload artifacts to S3 (or leave locally).
#    e. Report result to POST $MAGNETITE_API_URL/api/v1/github/builds/:id/report
#       using the runner_token from step 1.
# 3. On success the backend sets artifact_url = CDN URL and build_status =
#    'success'.  The frontend play manifest will then resolve correctly.
#
# ─── BUCKET D note ──────────────────────────────────────────────────────────
# This is the "external runner" dependency documented in DECISIONS.md §4c.
# It requires a machine with the full Rust toolchain and upload credentials.
# The backend correctly sets build_status to 'queued' (not 'success') until
# this script (or a GitHub Actions workflow using the same contract) reports
# back — no fake progress is generated server-side.
#
# See also: scripts/build-game.sh (simpler per-game local build helper).
# =============================================================================

set -euo pipefail

# ── Colour helpers ───────────────────────────────────────────────────────────
info()  { printf '\033[36m[wasm-runner]\033[0m %s\n' "$*"; }
ok()    { printf '\033[32m[wasm-runner]\033[0m %s\n' "$*"; }
warn()  { printf '\033[33m[wasm-runner]\033[0m WARNING: %s\n' "$*" >&2; }
err()   { printf '\033[31m[wasm-runner]\033[0m ERROR: %s\n'   "$*" >&2; }

# ── Validate environment ─────────────────────────────────────────────────────
MAGNETITE_API_URL="${MAGNETITE_API_URL:-}"
if [[ -z "$MAGNETITE_API_URL" ]]; then
  err "MAGNETITE_API_URL is not set. Example: export MAGNETITE_API_URL=https://api.magnetite.gg"
  exit 1
fi
# Strip trailing slash
MAGNETITE_API_URL="${MAGNETITE_API_URL%/}"

POLL_INTERVAL="${POLL_INTERVAL:-30}"
GITHUB_TOKEN="${GITHUB_TOKEN:-}"
ARTIFACT_BUCKET="${ARTIFACT_BUCKET:-}"
ARTIFACT_PREFIX="${ARTIFACT_PREFIX:-artifacts}"
CDN_BASE_URL="${CDN_BASE_URL:-}"
BUILD_WORKSPACE="${BUILD_WORKSPACE:-/tmp/magnetite-builds}"
RUST_PROFILE="${RUST_PROFILE:-release}"
SKIP_WASM_OPT="${SKIP_WASM_OPT:-0}"

TARGET="wasm32-unknown-unknown"

# ── Prerequisite checks ──────────────────────────────────────────────────────
check_prereqs() {
  local missing=()
  command -v rustup  &>/dev/null || missing+=("rustup")
  command -v cargo   &>/dev/null || missing+=("cargo")
  command -v git     &>/dev/null || missing+=("git")
  command -v curl    &>/dev/null || missing+=("curl")
  command -v jq      &>/dev/null || missing+=("jq")
  command -v wasm-bindgen &>/dev/null || missing+=("wasm-bindgen-cli (cargo install wasm-bindgen-cli)")

  if [[ ${#missing[@]} -gt 0 ]]; then
    err "Missing prerequisites: ${missing[*]}"
    exit 1
  fi

  if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
    info "Adding Rust target $TARGET..."
    rustup target add "$TARGET"
  fi

  if [[ -n "$ARTIFACT_BUCKET" ]] && ! command -v aws &>/dev/null && ! command -v s3cmd &>/dev/null; then
    warn "ARTIFACT_BUCKET is set but neither 'aws' nor 's3cmd' is installed — artifacts will not be uploaded."
    ARTIFACT_BUCKET=""
  fi

  info "Prerequisites OK. Polling $MAGNETITE_API_URL every ${POLL_INTERVAL}s."
}

# ── Fetch pending builds ─────────────────────────────────────────────────────
fetch_pending() {
  curl -sf \
    -H "Accept: application/json" \
    "$MAGNETITE_API_URL/api/v1/github/builds/pending" \
    || true  # Don't fail the script if the API is temporarily unreachable
}

# ── Report build result back to the platform ─────────────────────────────────
# $1: build_id (UUID)
# $2: runner_token (UUID)
# $3: outcome ("success" or "failed")
# $4: artifact_url (optional, required for success)
# $5: sha256_hash (optional)
# $6: file_size_bytes (optional integer, pass "" if unknown)
# $7: log_output (string)
report_result() {
  local build_id="$1"
  local runner_token="$2"
  local outcome="$3"
  local artifact_url="${4:-}"
  local sha256_hash="${5:-}"
  local file_size_bytes="${6:-}"
  local log_output="${7:-}"

  # Truncate log to 60 KiB to stay under typical HTTP body limits
  if [[ ${#log_output} -gt 61440 ]]; then
    log_output="${log_output: -61440}"
    log_output="...[truncated]...${log_output}"
  fi

  # Build JSON payload
  local payload
  payload="$(jq -n \
    --arg outcome "$outcome" \
    --arg artifact_url "$artifact_url" \
    --arg sha256_hash "$sha256_hash" \
    --argjson file_size_bytes "${file_size_bytes:-null}" \
    --arg log_output "$log_output" \
    '{
       outcome: $outcome,
       artifact_url:    (if $artifact_url    != "" then $artifact_url    else null end),
       sha256_hash:     (if $sha256_hash     != "" then $sha256_hash     else null end),
       file_size_bytes: $file_size_bytes,
       log_output:      (if $log_output      != "" then $log_output      else null end)
     }')"

  local response
  response="$(curl -sf \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $runner_token" \
    -d "$payload" \
    "$MAGNETITE_API_URL/api/v1/github/builds/$build_id/report")" || {
    warn "Failed to report build $build_id result to platform API (will retry next poll)."
    return 1
  }

  ok "Reported build $build_id as '$outcome'. Platform response: $(echo "$response" | jq -c .)"
}

# ── Upload artifact to S3 ────────────────────────────────────────────────────
# Returns the public CDN URL (or local path) via stdout.
# $1: local wasm file path
# $2: S3 key suffix (e.g. game-uuid/build-uuid/game_bg.wasm)
upload_artifact() {
  local local_path="$1"
  local key_suffix="$2"
  local s3_key="${ARTIFACT_PREFIX}/${key_suffix}"

  if [[ -z "$ARTIFACT_BUCKET" ]]; then
    # No bucket configured — report local path (useful for local runner testing)
    echo "file://$local_path"
    return 0
  fi

  if command -v aws &>/dev/null; then
    aws s3 cp "$local_path" "s3://${ARTIFACT_BUCKET}/${s3_key}" \
      --content-type "application/wasm" \
      --acl public-read 2>&1
  elif command -v s3cmd &>/dev/null; then
    s3cmd put "$local_path" "s3://${ARTIFACT_BUCKET}/${s3_key}" \
      --mime-type "application/wasm" \
      --acl-public 2>&1
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

# ── Build a single queued job ────────────────────────────────────────────────
# $1: JSON object with { id, repository, commit_sha, runner_token }
process_build() {
  local build_json="$1"
  local build_id repository commit_sha runner_token

  build_id="$(echo "$build_json"    | jq -r '.id')"
  repository="$(echo "$build_json"  | jq -r '.repository')"
  commit_sha="$(echo "$build_json"  | jq -r '.commit_sha')"
  runner_token="$(echo "$build_json"| jq -r '.runner_token')"

  info "=== Starting build $build_id: $repository@${commit_sha:0:8} ==="

  local work_dir="$BUILD_WORKSPACE/$build_id"
  mkdir -p "$work_dir"
  trap "rm -rf '$work_dir'" EXIT

  local log_file="$work_dir/build.log"
  local exit_code=0

  {
    # ── 1. Clone ─────────────────────────────────────────────────────────────
    echo "=== Cloning $repository at $commit_sha ==="
    local clone_url
    if [[ -n "$GITHUB_TOKEN" ]]; then
      clone_url="https://x-access-token:${GITHUB_TOKEN}@github.com/${repository}.git"
    else
      clone_url="https://github.com/${repository}.git"
    fi

    git clone --depth=50 --no-tags "$clone_url" "$work_dir/src" 2>&1
    (
      cd "$work_dir/src"
      git checkout "$commit_sha" 2>&1
    )

    echo "=== Clone complete ==="

    # ── 2. Find Cargo.toml ───────────────────────────────────────────────────
    local game_dir="$work_dir/src"
    if [[ ! -f "$game_dir/Cargo.toml" ]]; then
      echo "ERROR: No Cargo.toml found in repository root." >&2
      exit 1
    fi

    # ── 3. cargo build ───────────────────────────────────────────────────────
    echo "=== Building $game_dir for $TARGET ($RUST_PROFILE) ==="
    local profile_flag=""
    [[ "$RUST_PROFILE" == "release" ]] && profile_flag="--release"

    CRATE_NAME="$(grep -m1 '^name' "$game_dir/Cargo.toml" \
      | sed 's/.*= *"\(.*\)"/\1/' | tr '-' '_')"
    echo "Crate name: $CRATE_NAME"

    (
      cd "$game_dir"
      cargo build \
        --target "$TARGET" \
        $profile_flag \
        --no-default-features \
        --features wasm \
        2>&1
    )

    local wasm_file="$game_dir/target/$TARGET/$RUST_PROFILE/${CRATE_NAME}.wasm"
    if [[ ! -f "$wasm_file" ]]; then
      echo "ERROR: Expected wasm file not found: $wasm_file" >&2
      exit 1
    fi
    echo "Wasm binary: $(du -h "$wasm_file" | cut -f1)"

    # ── 4. wasm-bindgen ──────────────────────────────────────────────────────
    echo "=== Running wasm-bindgen ==="
    local out_dir="$work_dir/dist"
    mkdir -p "$out_dir"

    wasm-bindgen \
      --target web \
      --out-dir "$out_dir" \
      --out-name game \
      "$wasm_file" \
      2>&1

    # ── 5. wasm-opt (optional) ───────────────────────────────────────────────
    if [[ "$SKIP_WASM_OPT" != "1" ]] \
       && command -v wasm-opt &>/dev/null \
       && [[ "$RUST_PROFILE" == "release" ]]; then
      echo "=== Running wasm-opt (-Oz) ==="
      local wasm_out="$out_dir/game_bg.wasm"
      local before after
      before="$(du -h "$wasm_out" | cut -f1)"
      wasm-opt -Oz --output "$wasm_out" "$wasm_out" 2>&1
      after="$(du -h "$wasm_out" | cut -f1)"
      echo "wasm-opt: $before → $after"
    fi

    echo "=== Build complete — dist/ contents: ==="
    ls -lh "$out_dir/"

  } > "$log_file" 2>&1 || exit_code=$?

  local log_output
  log_output="$(cat "$log_file")"

  if [[ $exit_code -ne 0 ]]; then
    err "Build $build_id FAILED (exit $exit_code). Reporting failure."
    report_result "$build_id" "$runner_token" "failed" "" "" "" "$log_output"
    rm -rf "$work_dir"
    trap - EXIT
    return 0
  fi

  ok "Build $build_id succeeded. Uploading artifacts..."

  # ── 6. Upload & report success ───────────────────────────────────────────
  local wasm_out="$BUILD_WORKSPACE/$build_id/dist/game_bg.wasm"
  local artifact_url sha256_hash file_size_bytes

  # sha256 hash of the compiled wasm
  sha256_hash="$(shasum -a 256 "$wasm_out" 2>/dev/null | awk '{print $1}' || true)"
  file_size_bytes="$(wc -c < "$wasm_out" 2>/dev/null | tr -d '[:space:]' || echo "")"

  # Upload (returns CDN URL or file:// path)
  local game_slug="${repository//\//-}"
  local s3_key="${game_slug}/${build_id}/game_bg.wasm"
  artifact_url="$(upload_artifact "$wasm_out" "$s3_key")"

  ok "Artifact: $artifact_url  (sha256=${sha256_hash:0:16}...  size=${file_size_bytes}B)"

  report_result \
    "$build_id" "$runner_token" "success" \
    "$artifact_url" "$sha256_hash" "$file_size_bytes" \
    "$log_output"

  rm -rf "$work_dir"
  trap - EXIT
}

# ── Main poll loop ────────────────────────────────────────────────────────────
check_prereqs
mkdir -p "$BUILD_WORKSPACE"

info "Magnetite WASM build runner started."
info "  API:       $MAGNETITE_API_URL"
info "  Workspace: $BUILD_WORKSPACE"
info "  Bucket:    ${ARTIFACT_BUCKET:-<none — local only>}"
info "  CDN:       ${CDN_BASE_URL:-<none>}"
info "  Profile:   $RUST_PROFILE"
echo ""

while true; do
  info "Checking for queued builds..."

  pending_json="$(fetch_pending)"

  if [[ -z "$pending_json" ]] || [[ "$(echo "$pending_json" | jq 'length')" == "0" ]]; then
    info "No pending builds. Sleeping ${POLL_INTERVAL}s..."
    sleep "$POLL_INTERVAL"
    continue
  fi

  build_count="$(echo "$pending_json" | jq 'length')"
  info "Found $build_count queued build(s)."

  echo "$pending_json" | jq -c '.[]' | while read -r build_json; do
    process_build "$build_json" || warn "Failed to process build — skipping."
  done

  info "Batch done. Sleeping ${POLL_INTERVAL}s..."
  sleep "$POLL_INTERVAL"
done
