#!/usr/bin/env bash
# Stage the Rust release binary into npm platform packages for local dev or CI.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TARGET="${KEEL_TARGET:-}"
NPM_PKG="${KEEL_NPM_PKG:-}"

detect_host() {
  local arch os
  arch="$(uname -m)"
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  case "${os}-${arch}" in
    linux-x86_64)  echo "x86_64-unknown-linux-gnu linux-x64-gnu" ;;
    linux-aarch64|linux-arm64) echo "aarch64-unknown-linux-gnu linux-arm64-gnu" ;;
    darwin-x86_64) echo "x86_64-apple-darwin darwin-x64" ;;
    darwin-arm64)  echo "aarch64-apple-darwin darwin-arm64" ;;
    *) echo "unsupported platform: ${os}-${arch}" >&2; exit 1 ;;
  esac
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGET="$2"; shift 2 ;;
    --npm-pkg) NPM_PKG="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$TARGET" || -z "$NPM_PKG" ]]; then
  read -r TARGET NPM_PKG <<< "$(detect_host)"
fi

echo "Building keel for ${TARGET}..."
cargo build --release --target "$TARGET" 2>/dev/null || cargo build --release

BIN="target/${TARGET}/release/keel"
if [[ ! -f "$BIN" ]]; then
  BIN="target/release/keel"
fi

if [[ ! -f "$BIN" ]]; then
  echo "binary not found at target/${TARGET}/release/keel or target/release/keel" >&2
  exit 1
fi

PLATFORM_DIR="npm/platforms/${NPM_PKG}"
mkdir -p "${PLATFORM_DIR}/bin"
cp "$BIN" "${PLATFORM_DIR}/bin/keel"
chmod +x "${PLATFORM_DIR}/bin/keel"

# Local dev: also vendor into keel-cli for shim fallback
mkdir -p npm/keel-cli/vendor
cp "$BIN" npm/keel-cli/vendor/keel
chmod +x npm/keel-cli/vendor/keel

VERSION="$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
node npm/keel-cli/scripts/sync-version.js "$VERSION"

echo "Staged ${BIN} -> ${PLATFORM_DIR}/bin/keel"
echo "Version: ${VERSION}"
