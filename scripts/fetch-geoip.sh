#!/usr/bin/env bash
#
# Fetch the MaxMind GeoLite2-City database for the super-admin analytics panel's
# offline IP geolocation. MaxMind's DB is free but cannot be redistributed, so it
# is downloaded here rather than shipped in the repo.
#
# Prereq: a free MaxMind account + a license key (https://www.maxmind.com/en/geolite2/signup).
#
# Usage:
#   MAXMIND_LICENSE_KEY=xxxx scripts/fetch-geoip.sh [dest_dir]
#
# Then point the backend at the extracted file:
#   GEOIP_DB_PATH=<dest_dir>/GeoLite2-City.mmdb
#
set -euo pipefail

DEST_DIR="${1:-./geoip}"
EDITION="GeoLite2-City"

if [[ -z "${MAXMIND_LICENSE_KEY:-}" ]]; then
  echo "ERROR: MAXMIND_LICENSE_KEY is not set." >&2
  echo "Sign up free at https://www.maxmind.com/en/geolite2/signup and create a license key." >&2
  exit 1
fi

mkdir -p "$DEST_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

URL="https://download.maxmind.com/app/geoip_download?edition_id=${EDITION}&license_key=${MAXMIND_LICENSE_KEY}&suffix=tar.gz"

echo "Downloading ${EDITION}…"
curl -fsSL "$URL" -o "$TMP/db.tar.gz"

echo "Extracting…"
tar -xzf "$TMP/db.tar.gz" -C "$TMP"

MMDB="$(find "$TMP" -name "${EDITION}.mmdb" | head -1)"
if [[ -z "$MMDB" ]]; then
  echo "ERROR: ${EDITION}.mmdb not found in the downloaded archive." >&2
  exit 1
fi

cp "$MMDB" "$DEST_DIR/${EDITION}.mmdb"
echo "Done. Set:"
echo "  GEOIP_DB_PATH=$(cd "$DEST_DIR" && pwd)/${EDITION}.mmdb"
