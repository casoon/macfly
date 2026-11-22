#!/usr/bin/env bash
# Direct binary installer for MacFly (macOS, Apple Silicon & Intel — no Cargo required).
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/casoon/macfly/main/install.sh | bash
#
# Optional environment variables:
#   MACFLY_VERSION=v0.4.0   Install a specific release tag (default: latest)
#   INSTALL_DIR=~/.local/bin  Target directory (default: existing binary dir or ~/.local/bin)
set -euo pipefail

REPO="casoon/macfly"
BINARY_NAME="macfly"
EXPLICIT_INSTALL_DIR="${INSTALL_DIR:-}"

if [ -t 1 ]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[1;33m'
  NC='\033[0m'
else
  RED=''
  GREEN=''
  YELLOW=''
  NC=''
fi

info() { printf "%b[INFO]%b %s\n" "$GREEN" "$NC" "$1"; }
warn() { printf "%b[WARN]%b %s\n" "$YELLOW" "$NC" "$1"; }
error() { printf "%b[ERROR]%b %s\n" "$RED" "$NC" "$1" >&2; exit 1; }

check_dependencies() {
  command -v curl >/dev/null 2>&1 || error "curl is required"
  command -v tar >/dev/null 2>&1 || error "tar is required"
  if ! command -v shasum >/dev/null 2>&1 && ! command -v sha256sum >/dev/null 2>&1; then
    error "shasum or sha256sum is required for checksum verification"
  fi
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  if [ "$os" != "Darwin" ]; then
    error "MacFly automates macOS apps and requires macOS (detected: $os)."
  fi

  case "$arch" in
    arm64|aarch64)
      echo "aarch64-apple-darwin"
      ;;
    x86_64|amd64)
      echo "x86_64-apple-darwin"
      ;;
    *)
      error "Unsupported macOS architecture: $arch"
      ;;
  esac
}

get_latest_version() {
  local final_url
  final_url="$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")"
  printf '%s\n' "${final_url##*/}"
}

verify_checksum() {
  local archive_path="$1"
  local sums_path="$2"
  local archive_name expected actual

  archive_name="$(basename "$archive_path")"
  expected="$(awk -v name="$archive_name" '$2 == name || $2 == "*" name { print $1; exit }' "$sums_path")"
  if [ -z "$expected" ]; then
    error "Checksum entry for $archive_name not found in SHA256SUMS"
  fi

  if command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$archive_path" | awk '{print $1}')"
  else
    actual="$(sha256sum "$archive_path" | awk '{print $1}')"
  fi

  if [ "$actual" != "$expected" ]; then
    error "SHA-256 verification failed for $archive_name (expected $expected, got $actual)"
  fi
}

resolve_install_dir() {
  if [ -n "$EXPLICIT_INSTALL_DIR" ]; then
    echo "$EXPLICIT_INSTALL_DIR"
    return
  fi

  local existing
  existing="$(command -v "$BINARY_NAME" 2>/dev/null || true)"
  if [ -n "$existing" ] && [ -w "$(dirname "$existing")" ]; then
    dirname "$existing"
  else
    echo "$HOME/.local/bin"
  fi
}

main() {
  check_dependencies

  local target version install_dir archive_name url sums_url
  target="$(detect_target)"
  version="${MACFLY_VERSION:-$(get_latest_version)}"
  [ -n "$version" ] || error "Could not determine latest MacFly release version"

  install_dir="$(resolve_install_dir)"
  archive_name="${BINARY_NAME}-${target}.tar.gz"
  url="https://github.com/$REPO/releases/download/$version/$archive_name"
  sums_url="https://github.com/$REPO/releases/download/$version/SHA256SUMS"

  info "Installing $BINARY_NAME $version for $target"

  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "${TMP_DIR:-}"' EXIT

  info "Downloading $archive_name"
  curl -fsSL "$url" -o "$TMP_DIR/$archive_name"

  info "Downloading SHA256SUMS"
  curl -fsSL "$sums_url" -o "$TMP_DIR/SHA256SUMS"

  info "Verifying SHA-256 checksum"
  verify_checksum "$TMP_DIR/$archive_name" "$TMP_DIR/SHA256SUMS"

  tar -xzf "$TMP_DIR/$archive_name" -C "$TMP_DIR"
  mkdir -p "$install_dir"
  mv "$TMP_DIR/$BINARY_NAME" "$install_dir/$BINARY_NAME"
  chmod +x "$install_dir/$BINARY_NAME"

  info "Installed $BINARY_NAME $version to $install_dir/$BINARY_NAME"

  if [[ ":$PATH:" != *":$install_dir:"* ]]; then
    warn "$install_dir is not in your PATH"
    printf '\nAdd this line to ~/.zshrc (or ~/.bashrc):\n\n  export PATH="%s:$PATH"\n\n' "$install_dir"
  fi

  printf '\nNext steps:\n  macfly doctor\n  macfly config init\n'
}

main "$@"
