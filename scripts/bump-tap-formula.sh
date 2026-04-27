#!/usr/bin/env bash
# Bump version and per-target sha256 fields in iselegant/homebrew-tap's
# Formula/cchb.rb in place.
#
# Required environment variables:
#   VERSION           - new version (without leading "v"), e.g. "0.10.0"
#   SHA_ARM_MAC       - sha256 for cchb-aarch64-apple-darwin.tar.gz
#   SHA_INTEL_MAC     - sha256 for cchb-x86_64-apple-darwin.tar.gz
#   SHA_INTEL_LINUX   - sha256 for cchb-x86_64-unknown-linux-gnu.tar.gz
#
# Usage: bump-tap-formula.sh <path-to-formula>

set -euo pipefail

: "${VERSION:?VERSION required}"
: "${SHA_ARM_MAC:?SHA_ARM_MAC required}"
: "${SHA_INTEL_MAC:?SHA_INTEL_MAC required}"
: "${SHA_INTEL_LINUX:?SHA_INTEL_LINUX required}"

FORMULA="${1:?formula path required}"

bump_version() {
  sed -i.bak \
    "s|^  version \"[^\"]*\"|  version \"${VERSION}\"|" \
    "${FORMULA}"
  rm "${FORMULA}.bak"
}

bump_sha() {
  local target="$1"
  local sha="$2"
  sed -i.bak \
    "/${target}\\.tar\\.gz/{n;s|sha256 \"[^\"]*\"|sha256 \"${sha}\"|;}" \
    "${FORMULA}"
  rm "${FORMULA}.bak"
}

bump_version
bump_sha "cchb-aarch64-apple-darwin" "${SHA_ARM_MAC}"
bump_sha "cchb-x86_64-apple-darwin" "${SHA_INTEL_MAC}"
bump_sha "cchb-x86_64-unknown-linux-gnu" "${SHA_INTEL_LINUX}"
