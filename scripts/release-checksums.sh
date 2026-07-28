#!/usr/bin/env bash
# =============================================================================
# scripts/release-checksums.sh — emit and verify a release's SHA256SUMS
#
# WHAT THIS IS FOR
# ────────────────
# Magnetite publishes binaries and a web bundle on a `v*` tag
# (`.github/workflows/release.yml`). Until this script existed it published
# them with NO integrity manifest at all: a downloader had nothing to check the
# bytes against, so a corrupted or substituted asset was indistinguishable from
# a good one.
#
# Two subcommands, both operating on one staging directory:
#
#   emit   <dir>   write <dir>/SHA256SUMS covering every OTHER file in <dir>
#   verify <dir>   re-derive every digest and compare; exit non-zero on any doubt
#   selftest       prove `verify` actually fails on each way a release can be
#                  wrong (run by CI — see .github/workflows/ci.yml)
#
# FAIL-CLOSED — read this before adding a flag to it
# ─────────────────────────────────────────────────────────────────────────────
# `verify` has exactly two outcomes: "every published byte is accounted for" or
# "exit 1". There is deliberately NO skip switch, NO "warn and continue", and
# no path where a missing SHA256SUMS is treated as "nothing to check". That
# last one is the failure mode this design exists to avoid: the common way a
# checksum step breaks is that the manifest is absent, and a verifier that
# shrugs at an absent manifest prints a line that looks like verification
# happened while checking nothing at all. A verifier that reports safety it did
# not check is worse than no verifier.
#
# Specifically, `verify` fails when:
#   1. <dir> does not exist, or holds no assets.
#   2. SHA256SUMS is missing, empty, or unreadable.
#   3. A file listed in SHA256SUMS is not present in <dir>   (missing asset).
#   4. A file present in <dir> is not listed in SHA256SUMS   (unvouched asset —
#      it would be published under a manifest that says nothing about it).
#   5. Any digest differs from the file's actual digest      (tampered/corrupt).
#
# Filenames are matched EXACTLY on field 2 of the manifest, never as a regex or
# a substring, so a `.` in an asset name can never match a different asset.
#
# PORTABILITY
# ───────────
# GNU coreutils `sha256sum` (CI, Linux) and Perl `shasum -a 256` (macOS, where
# a maintainer runs this by hand) produce the same two-field format, so either
# one is accepted and the manifest is byte-identical between them.
#
# USAGE
# ─────
#   bash scripts/release-checksums.sh emit   release-out
#   bash scripts/release-checksums.sh verify release-out
#   bash scripts/release-checksums.sh selftest
# =============================================================================

set -euo pipefail

MANIFEST="SHA256SUMS"

die() {
  echo "::error::$*" >&2
  exit 1
}

# Print "<digest>  <name>" for one file, using whichever tool this host has.
sha_line() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file"
  else
    die "neither sha256sum nor shasum is available — cannot checksum ${file}. \
Install coreutils (Linux) or Perl's shasum (macOS ships it)."
  fi
}

sha_of() {
  sha_line "$1" | awk '{print $1}'
}

# Every regular file in <dir> except the manifest itself, one per line, sorted
# so the manifest is reproducible. Top level only: the release staging dir is
# flat by construction (each build job stages finished assets into it).
assets_in() {
  local dir="$1"
  find "$dir" -maxdepth 1 -type f ! -name "$MANIFEST" -exec basename {} \; | LC_ALL=C sort
}

cmd_emit() {
  local dir="${1:-}"
  [ -n "$dir" ] || die "usage: release-checksums.sh emit <dir>"
  [ -d "$dir" ] || die "staging directory ${dir} does not exist"

  local assets
  assets="$(assets_in "$dir")"
  [ -n "$assets" ] || die "staging directory ${dir} holds no assets — refusing to \
write an empty ${MANIFEST}, which would vouch for nothing while looking like it vouched for a release"

  # Written from inside <dir> so the manifest carries bare filenames, matching
  # how the assets appear on the GitHub Release (and how a downloader will have
  # them on disk next to the manifest).
  (
    cd "$dir"
    : > "$MANIFEST"
    while IFS= read -r asset; do
      sha_line "$asset" >> "$MANIFEST"
    done <<< "$assets"
  )

  echo "Wrote ${dir}/${MANIFEST}:"
  cat "${dir}/${MANIFEST}"
}

cmd_verify() {
  local dir="${1:-}"
  [ -n "$dir" ] || die "usage: release-checksums.sh verify <dir>"
  [ -d "$dir" ] || die "staging directory ${dir} does not exist — nothing to verify. \
A release with no assets is a broken release, not a clean one."

  local manifest="${dir}/${MANIFEST}"
  [ -f "$manifest" ] || die "${manifest} is missing — refusing to publish unverified bytes. \
Every Magnetite release publishes ${MANIFEST}; its absence means the release is incomplete."
  [ -s "$manifest" ] || die "${manifest} is empty — refusing to publish unverified bytes."

  local assets
  assets="$(assets_in "$dir")"
  [ -n "$assets" ] || die "${dir} holds no assets besides ${MANIFEST} — nothing was built."

  local failures=0

  # (3) + (5): every listed file must exist and match. Read the manifest, not
  # the directory, so an asset the manifest names but the build never produced
  # is caught rather than silently skipped.
  local listed=""
  while read -r want_digest want_name; do
    [ -n "${want_digest:-}" ] || continue
    # `sha256sum` writes "*name" for binary mode; accept and strip that marker.
    want_name="${want_name#\*}"
    listed="${listed}${want_name}"$'\n'
    if [ ! -f "${dir}/${want_name}" ]; then
      echo "::error::${want_name} is listed in ${MANIFEST} but was not built" >&2
      failures=$((failures + 1))
      continue
    fi
    local got
    got="$(sha_of "${dir}/${want_name}")"
    if [ "$got" != "$want_digest" ]; then
      echo "::error::checksum mismatch for ${want_name} — ${MANIFEST} says ${want_digest}, file is ${got}" >&2
      failures=$((failures + 1))
    else
      echo "  ok  ${want_name}  ${got}"
    fi
  done < "$manifest"

  # (4): the other direction. An asset the manifest does not name would be
  # published alongside a manifest that says nothing about it — a downloader
  # doing an exact-name lookup gets no digest and (correctly) refuses, so this
  # has to be a red build here rather than a dead end in a user's terminal.
  while IFS= read -r asset; do
    if ! grep -qxF "$asset" <<< "$listed"; then
      echo "::error::${asset} is present but not listed in ${MANIFEST} — it would ship unvouched for" >&2
      failures=$((failures + 1))
    fi
  done <<< "$assets"

  [ "$failures" -eq 0 ] || die "${failures} checksum problem(s) — refusing to publish this release."

  echo "${MANIFEST} covers all $(wc -l < "$manifest" | tr -d ' ') asset(s) in ${dir}, every digest matches."
}

# --- selftest ---------------------------------------------------------------
# Asserts the five failure modes above actually fail. A verifier is only worth
# having if its refusals are exercised, so this runs in CI next to the tests.
expect_fail() {
  local what="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    die "selftest: ${what} — verify PASSED but must have failed"
  fi
  echo "  ok  refuses: ${what}"
}

expect_pass() {
  local what="$1"
  shift
  if ! "$@" >/dev/null 2>&1; then
    die "selftest: ${what} — verify FAILED but must have passed"
  fi
  echo "  ok  accepts: ${what}"
}

cmd_selftest() {
  local tmp
  tmp="$(mktemp -d)"
  # Expanded now, not at trap time: `tmp` is function-local and the EXIT trap
  # runs after the function has returned, where it would be unset under `set -u`.
  # shellcheck disable=SC2064
  trap "rm -rf '${tmp}'" EXIT
  local self="${BASH_SOURCE[0]}"

  echo "release-checksums selftest"

  # A good release.
  mkdir -p "$tmp/good"
  printf 'binary-one' > "$tmp/good/magnetite-0.0.0-linux-x86_64"
  printf 'bundle' > "$tmp/good/magnetite-web-0.0.0.tar.gz"
  bash "$self" emit "$tmp/good" >/dev/null
  expect_pass "a complete, matching release" bash "$self" verify "$tmp/good"

  # (1) empty directory.
  mkdir -p "$tmp/empty"
  expect_fail "an empty staging directory" bash "$self" verify "$tmp/empty"

  # (1) missing directory.
  expect_fail "a staging directory that does not exist" bash "$self" verify "$tmp/absent"

  # (2) no manifest — the failure mode that must never be read as "nothing to check".
  cp -R "$tmp/good" "$tmp/no-manifest"
  rm "$tmp/no-manifest/SHA256SUMS"
  expect_fail "a release with no SHA256SUMS at all" bash "$self" verify "$tmp/no-manifest"

  # (2) empty manifest.
  cp -R "$tmp/good" "$tmp/empty-manifest"
  : > "$tmp/empty-manifest/SHA256SUMS"
  expect_fail "an empty SHA256SUMS" bash "$self" verify "$tmp/empty-manifest"

  # (3) listed asset was never built.
  cp -R "$tmp/good" "$tmp/missing-asset"
  rm "$tmp/missing-asset/magnetite-web-0.0.0.tar.gz"
  expect_fail "an asset listed in SHA256SUMS that was not built" bash "$self" verify "$tmp/missing-asset"

  # (4) built asset nobody vouched for.
  cp -R "$tmp/good" "$tmp/extra-asset"
  printf 'surprise' > "$tmp/extra-asset/magnetite-backend-0.0.0-linux-x86_64"
  expect_fail "an asset present but absent from SHA256SUMS" bash "$self" verify "$tmp/extra-asset"

  # (5) tampered bytes.
  cp -R "$tmp/good" "$tmp/tampered"
  printf 'binary-TWO' > "$tmp/tampered/magnetite-0.0.0-linux-x86_64"
  expect_fail "an asset whose bytes changed after SHA256SUMS was written" bash "$self" verify "$tmp/tampered"

  # emit itself refuses to vouch for nothing.
  expect_fail "emitting a manifest for an empty directory" bash "$self" emit "$tmp/empty"

  echo "selftest: all 9 cases behaved as specified."
}

case "${1:-}" in
  emit)     shift; cmd_emit "$@" ;;
  verify)   shift; cmd_verify "$@" ;;
  selftest) shift; cmd_selftest "$@" ;;
  *)
    echo "usage: release-checksums.sh {emit|verify} <dir>" >&2
    echo "       release-checksums.sh selftest" >&2
    exit 2
    ;;
esac
