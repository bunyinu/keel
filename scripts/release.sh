#!/usr/bin/env bash
# Local release helper: test, build, stage npm, optional global install.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

INSTALL_GLOBAL=0
SKIP_TESTS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-global) INSTALL_GLOBAL=1; shift ;;
    --skip-tests) SKIP_TESTS=1; shift ;;
    *) echo "usage: $0 [--install-global] [--skip-tests]" >&2; exit 1 ;;
  esac
done

if cargo fmt --version &>/dev/null; then
  echo "==> cargo fmt --check"
  cargo fmt --all -- --check
else
  echo "==> skip cargo fmt (rustfmt not installed)"
fi

if [[ "$SKIP_TESTS" -eq 0 ]]; then
  echo "==> cargo test"
  cargo test --all-targets
fi

if cargo clippy --version &>/dev/null; then
  echo "==> cargo clippy"
  cargo clippy --all-targets -- -D warnings
else
  echo "==> skip cargo clippy (not installed)"
fi

echo "==> stage npm"
chmod +x scripts/stage-npm.sh
./scripts/stage-npm.sh

echo "==> verify npm shim"
node npm/keel-cli/scripts/verify-shim.js

if [[ "$INSTALL_GLOBAL" -eq 1 ]]; then
  # Remove legacy Python keel shim if it shadows npm (pip install -e .)
  if [[ -f "${HOME}/.local/bin/keel" ]] && head -1 "${HOME}/.local/bin/keel" 2>/dev/null | grep -q python; then
    echo "==> removing legacy Python keel at ~/.local/bin/keel"
    rm -f "${HOME}/.local/bin/keel"
  fi
  echo "==> npm install -g ./npm/keel-cli"
  npm install -g ./npm/keel-cli
  NPM_BIN="$(npm prefix -g)/bin"
  if [[ -x "${NPM_BIN}/keel" && "${NPM_BIN}/keel" != "${HOME}/.local/bin/keel" ]]; then
    mkdir -p "${HOME}/.local/bin"
    ln -sf "${NPM_BIN}/keel" "${HOME}/.local/bin/keel"
  fi
  echo "Installed: $(command -v keel)"
  keel --version
fi

echo ""
echo "Done. Next:"
echo "  npm install -g ./npm/keel-cli    # global install"
echo "  keel init                        # in your repo"
echo "  cargo install --path .           # alternative to npm"
