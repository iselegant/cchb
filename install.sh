#!/bin/sh
# Install script for cchb - Claude Code History Browser
# Usage: curl -fsSL https://raw.githubusercontent.com/iselegant/cchb/main/install.sh | sh

set -eu

REPO="iselegant/cchb"
INSTALL_DIR="${CCHB_INSTALL_DIR:-${HOME}/.local/bin}"

info() {
  printf '[cchb] %s\n' "$1"
}

error() {
  printf '[cchb] ERROR: %s\n' "$1" >&2
  exit 1
}

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)

  case "${os}" in
    Darwin)
      case "${arch}" in
        arm64|aarch64) target="aarch64-apple-darwin" ;;
        x86_64)        target="x86_64-apple-darwin" ;;
        *)             error "Unsupported architecture: ${arch}" ;;
      esac
      ;;
    Linux)
      case "${arch}" in
        x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
        *)            error "Unsupported architecture: ${arch}" ;;
      esac
      ;;
    *)
      error "Unsupported OS: ${os}"
      ;;
  esac

  echo "${target}"
}

get_latest_version() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p'
}

main() {
  target=$(detect_target)
  info "Detected platform: ${target}"

  version=$(get_latest_version)
  if [ -z "${version}" ]; then
    error "Failed to fetch latest version"
  fi
  info "Latest version: ${version}"

  archive="cchb-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${version}/${archive}"

  info "Downloading ${url} ..."
  tmp=$(mktemp -d)
  trap 'rm -rf "${tmp}"' EXIT

  curl -fsSL "${url}" -o "${tmp}/${archive}"
  tar xzf "${tmp}/${archive}" -C "${tmp}"

  mkdir -p "${INSTALL_DIR}"
  mv "${tmp}/cchb" "${INSTALL_DIR}/cchb"
  chmod +x "${INSTALL_DIR}/cchb"

  info "Installed cchb to ${INSTALL_DIR}/cchb"

  if ! echo "${PATH}" | tr ':' '\n' | grep -qx "${INSTALL_DIR}"; then
    info ""
    info "NOTE: ${INSTALL_DIR} is not in your PATH."
    info "Add it to your shell profile:"
    info "  export PATH=\"${INSTALL_DIR}:\${PATH}\""
  fi

  info "Done! Run 'cchb' to get started."
}

main
