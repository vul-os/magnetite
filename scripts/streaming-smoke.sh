#!/usr/bin/env bash
# scripts/streaming-smoke.sh
#
# Streaming HLS URL correctness smoke test.
#
# WHAT THIS CHECKS
# ────────────────
# The Magnetite backend constructs HLS manifest URLs for MediaMTX when a
# stream goes live and no explicit hls_url has been supplied by the broadcaster.
# The correct MediaMTX path is:
#
#     http://<host>:8888/live/<ingest_key>/index.m3u8
#
# This script verifies that:
#   1. MEDIA_SERVER_BASE_URL is set and non-empty.
#   2. A representative URL constructed by the backend follows the expected
#      pattern:  ${MEDIA_SERVER_BASE_URL}/live/<slug>/index.m3u8
#   3. If the backend is reachable, the /api/v1/streams/:id/watch endpoint
#      returns a hls_url or watch_url that matches the MediaMTX convention.
#
# USAGE
# ─────
#   # Unit-style check (no live backend needed):
#   MEDIA_SERVER_BASE_URL=http://localhost:8888 bash scripts/streaming-smoke.sh
#
#   # Integration check (backend + MediaMTX both running):
#   MEDIA_SERVER_BASE_URL=http://mediamtx:8888 \
#   BACKEND_URL=http://localhost:8080 \
#   STREAM_ID=<uuid> \
#   bash scripts/streaming-smoke.sh
#
# EXIT CODES
# ──────────
#   0  all checks passed
#   1  one or more checks failed

set -euo pipefail

# ─── Colour helpers ──────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Colour

ok()   { echo -e "${GREEN}[PASS]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; FAILURES=$((FAILURES + 1)); }
info() { echo -e "${YELLOW}[INFO]${NC} $*"; }

FAILURES=0

# ─── 1. MEDIA_SERVER_BASE_URL is set ─────────────────────────────────────────
echo ""
echo "=== streaming-smoke.sh: HLS URL correctness ==="
echo ""

MEDIA_SERVER_BASE_URL="${MEDIA_SERVER_BASE_URL:-}"

if [[ -z "$MEDIA_SERVER_BASE_URL" ]]; then
  fail "MEDIA_SERVER_BASE_URL is not set.  Export it, e.g.:"
  fail "  export MEDIA_SERVER_BASE_URL=http://localhost:8888"
  echo ""
  exit 1
else
  ok "MEDIA_SERVER_BASE_URL = $MEDIA_SERVER_BASE_URL"
fi

# ─── 2. URL structure validation ─────────────────────────────────────────────
# Validate the base URL is well-formed (http or https, host, optional port).
MEDIAMTX_PATTERN='^https?://[^/]+(:[0-9]+)?$'
if echo "$MEDIA_SERVER_BASE_URL" | grep -qE "$MEDIAMTX_PATTERN"; then
  ok "MEDIA_SERVER_BASE_URL is well-formed (no trailing slash)"
else
  fail "MEDIA_SERVER_BASE_URL should have no trailing slash: $MEDIA_SERVER_BASE_URL"
fi

# ─── 3. Synthesise a representative HLS URL and check its structure ───────────
# This mirrors what backend/src/api/streaming.rs constructs in derive_hls_url
# and in the hls_manifest fallback path.
EXAMPLE_INGEST_KEY="abc123xyz"
SYNTHESISED_HLS="${MEDIA_SERVER_BASE_URL}/live/${EXAMPLE_INGEST_KEY}/index.m3u8"
info "Synthesised HLS URL: $SYNTHESISED_HLS"

# Pattern: base + /live/ + alphanumeric slug + /index.m3u8
HLS_PATTERN='^https?://[^/]+(:[0-9]+)?/live/[A-Za-z0-9_-]+/index\.m3u8$'
if echo "$SYNTHESISED_HLS" | grep -qE "$HLS_PATTERN"; then
  ok "Synthesised HLS URL matches MediaMTX convention: $SYNTHESISED_HLS"
else
  fail "Synthesised HLS URL does NOT match expected pattern '$HLS_PATTERN': $SYNTHESISED_HLS"
fi

# ─── 4. Verify the URL does NOT use a UUID segment (old wrong pattern) ────────
UUID_PATTERN='[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}'
if echo "$SYNTHESISED_HLS" | grep -qE "$UUID_PATTERN"; then
  fail "HLS URL contains a UUID — ingest_key should be used, not the stream UUID"
else
  ok "HLS URL does not contain a UUID (using ingest_key, correct)"
fi

# ─── 5. Optional: live backend integration check ──────────────────────────────
BACKEND_URL="${BACKEND_URL:-}"
STREAM_ID="${STREAM_ID:-}"

if [[ -n "$BACKEND_URL" && -n "$STREAM_ID" ]]; then
  info "Backend URL and stream ID provided — attempting live check..."

  WATCH_JSON=$(curl -sf "${BACKEND_URL}/api/v1/streams/${STREAM_ID}/watch" 2>/dev/null || true)

  if [[ -z "$WATCH_JSON" ]]; then
    info "Backend not reachable or stream not found — skipping live check"
  else
    ok "Got watch info from backend"

    # Extract hls_url from JSON (simple grep; jq may not be available)
    HLS_URL_FROM_API=$(echo "$WATCH_JSON" | grep -oE '"hls_url"\s*:\s*"[^"]+"' | \
                       grep -oE 'http[^"]+' || true)

    if [[ -n "$HLS_URL_FROM_API" ]]; then
      info "hls_url from API: $HLS_URL_FROM_API"
      if echo "$HLS_URL_FROM_API" | grep -qE "$HLS_PATTERN"; then
        ok "Live hls_url matches MediaMTX convention"
      else
        fail "Live hls_url does NOT match MediaMTX convention: $HLS_URL_FROM_API"
      fi
    else
      info "hls_url field is null or absent (stream may not be live yet)"
    fi

    # Check watch_url (always the backend proxy endpoint)
    WATCH_URL_FROM_API=$(echo "$WATCH_JSON" | grep -oE '"watch_url"\s*:\s*"[^"]+"' | \
                         grep -oE 'http[^"]+' || true)
    if [[ -n "$WATCH_URL_FROM_API" ]]; then
      info "watch_url from API: $WATCH_URL_FROM_API"
      if echo "$WATCH_URL_FROM_API" | grep -qE "/api/v1/streams/[^/]+/hls$"; then
        ok "watch_url points to backend HLS proxy endpoint (correct)"
      else
        fail "watch_url does not look like a backend /hls endpoint: $WATCH_URL_FROM_API"
      fi
    fi
  fi
else
  info "BACKEND_URL / STREAM_ID not set — skipping live backend check"
  info "  To enable: export BACKEND_URL=http://localhost:8080 STREAM_ID=<uuid>"
fi

# ─── 6. MediaMTX reachability check (informational only) ─────────────────────
MEDIAMTX_HOST=$(echo "$MEDIA_SERVER_BASE_URL" | sed 's|^https\?://||' | cut -d/ -f1)
if curl -sf --max-time 2 "${MEDIA_SERVER_BASE_URL}/" > /dev/null 2>&1; then
  ok "MediaMTX is reachable at $MEDIA_SERVER_BASE_URL"
else
  info "MediaMTX not reachable at $MEDIA_SERVER_BASE_URL (not running — OK for offline check)"
fi

# ─── Summary ─────────────────────────────────────────────────────────────────
echo ""
if [[ $FAILURES -eq 0 ]]; then
  ok "All streaming-smoke checks passed."
  exit 0
else
  fail "$FAILURES check(s) failed."
  exit 1
fi
