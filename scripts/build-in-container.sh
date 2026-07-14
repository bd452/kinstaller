#!/usr/bin/env bash
# Run a device build inside the Linux container (OrbStack / Docker on macOS).
#
# Usage: build-in-container.sh [--dev] <kindlehf|kindlepw2>
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${KINSTALLER_BUILD_IMAGE:-kinstaller-build:kpm-devkit-0.1.0}"

usage() {
    echo "Usage: build-in-container.sh [--dev] <kindlehf|kindlepw2>" >&2
    exit 1
}

USE_DEV=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dev)
            USE_DEV=1
            shift
            ;;
        kindlehf | kindlepw2)
            PLATFORM=$1
            shift
            break
            ;;
        *)
            usage
            ;;
    esac
done

[[ $# -eq 0 ]] || usage
[[ -n "${PLATFORM:-}" ]] || usage

if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker not found." >&2
    echo "  Install OrbStack: https://orbstack.dev  (or: brew install --cask orbstack)" >&2
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    echo "error: docker daemon not running." >&2
    echo "  Start OrbStack, then retry." >&2
    exit 1
fi

if docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "==> Using existing container image $IMAGE"
else
    echo "==> Building container image $IMAGE (linux/amd64)"
    docker build --platform linux/amd64 -t "$IMAGE" -f "$REPO_ROOT/docker/Dockerfile" "$REPO_ROOT"
fi

if [[ "$USE_DEV" -eq 1 ]]; then
    build_cmd="./scripts/fetch-fonts.sh && ./scripts/build-target.sh --native --dev $PLATFORM"
else
    build_cmd="./scripts/fetch-fonts.sh && ./scripts/build-target.sh --native $PLATFORM"
fi

echo "==> Device build in Linux container: $PLATFORM"
docker run --rm --platform linux/amd64 \
    -v "$REPO_ROOT:/work" \
    -v kinstaller-cargo-registry:/usr/local/cargo/registry \
    -v kinstaller-cargo-git:/usr/local/cargo/git \
    -e KOXTOOLCHAIN_ROOT=/opt/x-tools/x-tools \
    -e KINSTALLER_IN_CONTAINER=1 \
    -w /work \
    "$IMAGE" \
    bash -lc "$build_cmd"

echo
echo "Built: $REPO_ROOT/dist/$PLATFORM/kinstaller"
