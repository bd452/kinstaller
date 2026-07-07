#!/usr/bin/env bash
# One-time (or repeatable) developer setup for the Kinstaller repo.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Fetching UI fonts"
"$REPO_ROOT/scripts/fetch-fonts.sh"

echo "==> Initialising git submodules"
git -C "$REPO_ROOT" submodule update --init --recursive

echo "==> Installing Rust targets (Kindle cross-build)"
rustup target add armv7-unknown-linux-gnueabihf armv7-unknown-linux-gnueabi \
    armv7-unknown-linux-musleabihf armv7-unknown-linux-musleabi 2>/dev/null || true

if [[ "$(uname -s)" == "Linux" ]]; then
    echo "==> Installing koxtoolchain (device builds with dlopen)"
    "$REPO_ROOT/scripts/setup-koxtoolchain.sh" || true
elif [[ "$(uname -s)" == "Darwin" ]]; then
    echo "==> OrbStack setup for macOS device builds (optional; run manually if needed)"
    echo "    ./scripts/setup-orbstack.sh"
fi

echo
echo "Setup complete."
echo "  UI dev:     cargo run -p kinstaller --features mock-backend"
if [[ "$(uname -s)" == "Linux" ]]; then
    echo "  Device HF:  ./scripts/build-target.sh kindlehf"
    echo "  Device PW2: ./scripts/build-target.sh kindlepw2"
else
    echo "  Device HF:  ./scripts/setup-orbstack.sh && ./scripts/build-target.sh kindlehf"
    echo "  Device PW2: ./scripts/build-target.sh kindlepw2"
    echo "  UI-only:    ./scripts/build-target.sh --musl kindlehf  (no libkpm dlopen)"
fi
