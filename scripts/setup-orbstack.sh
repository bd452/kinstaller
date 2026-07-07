#!/usr/bin/env bash
# One-time OrbStack / Docker setup for macOS device builds.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "OrbStack setup is for macOS. On Linux run ./scripts/setup-koxtoolchain.sh instead."
    exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
    echo "==> Docker CLI not found; installing OrbStack"
    if command -v brew >/dev/null 2>&1; then
        brew install --cask orbstack
    else
        echo "error: install OrbStack from https://orbstack.dev then re-run this script." >&2
        exit 1
    fi
fi

echo "==> Waiting for Docker daemon"
for _ in $(seq 1 60); do
    if docker info >/dev/null 2>&1; then
        break
    fi
    sleep 2
done

if ! docker info >/dev/null 2>&1; then
    echo "error: Docker daemon not reachable. Open OrbStack and retry." >&2
    exit 1
fi

echo "==> Pre-building Linux build image (linux/amd64)"
docker build --platform linux/amd64 -t kinstaller-build:latest -f "$REPO_ROOT/docker/Dockerfile" "$REPO_ROOT"

echo
echo "OrbStack ready for device builds:"
echo "  ./scripts/build-target.sh kindlehf"
echo "  ./scripts/build-target.sh kindlepw2"
