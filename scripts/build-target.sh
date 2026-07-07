#!/usr/bin/env bash
# Cross-build kinstaller for a Kindle platform.
#
# Device builds use the KindleModding koxtoolchain (glibc 2.20 sysroot, same as KPM).
# The kinstaller binary is dynamically linked so it can dlopen() the device libkpm.so.
#
# On macOS: runs inside OrbStack/Docker via build-in-container.sh (Linux + koxtoolchain).
# On Linux: runs natively with koxtoolchain gcc.
#
# Optional: --musl for UI-only testing on macOS (static musl; dlopen of libkpm fails).
# Optional: --native to force in-container/native kox build (used by build-in-container.sh).
# Optional: --dev for faster on-device iteration (dev-device profile; no LTO).
#
# Usage: build-target.sh [--musl|--native|--dev] <kindlehf|kindlepw2>
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    echo "Usage: build-target.sh [--musl|--native|--dev] <kindlehf|kindlepw2>" >&2
    exit 1
}

USE_MUSL=0
USE_NATIVE=0
USE_DEV=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --musl)
            USE_MUSL=1
            shift
            ;;
        --native)
            USE_NATIVE=1
            shift
            ;;
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

# macOS device builds go through OrbStack unless --musl or already in container.
if [[ "$USE_MUSL" -eq 0 && "$USE_NATIVE" -eq 0 && "$(uname -s)" == "Darwin" && "${KINSTALLER_IN_CONTAINER:-}" != "1" ]]; then
    if [[ "$USE_DEV" -eq 1 ]]; then
        exec "$REPO_ROOT/scripts/build-in-container.sh" --dev "$PLATFORM"
    else
        exec "$REPO_ROOT/scripts/build-in-container.sh" "$PLATFORM"
    fi
fi

glibc_audit() {
    local binary=$1
    local max_pin=$2
    local max_found
    max_found=$(strings "$binary" | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)?' | sort -Vu | tail -1 | sed 's/GLIBC_//')
    if [[ -z "$max_found" ]]; then
        echo "==> Glibc audit: no GLIBC_* version refs found (unexpected for dynamic glibc build)"
        return 0
    fi
    echo "==> Glibc audit: max symbol version GLIBC_$max_found (pin ≤ $max_pin)"
    local max_num pin_num
    max_num=$(printf '%s\n' "$max_found" | awk -F. '{ printf "%d%03d\n", $1, ($2 == "" ? 0 : $2) }')
    pin_num=$(printf '%s\n' "$max_pin" | awk -F. '{ printf "%d%03d\n", $1, ($2 == "" ? 0 : $2) }')
    if [[ "$max_num" -gt "$pin_num" ]]; then
        echo "error: binary requires GLIBC_$max_found but pin is $max_pin" >&2
        exit 1
    fi
}

build_musl() {
    case "$PLATFORM" in
        kindlehf)
            TARGET="armv7-unknown-linux-musleabihf"
            ;;
        kindlepw2)
            TARGET="armv7-unknown-linux-musleabi"
            ;;
    esac

    missing=()
    command -v cargo-zigbuild >/dev/null 2>&1 || missing+=("cargo-zigbuild")
    command -v zig >/dev/null 2>&1 || missing+=("zig")
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "error: missing required tools for --musl: ${missing[*]}" >&2
        echo "  cargo install cargo-zigbuild" >&2
        echo "  brew install zig   # or: pip install ziglang" >&2
        exit 1
    fi

    echo "==> Building kinstaller for $PLATFORM ($TARGET, static musl)"
    echo "warning: musl static binaries cannot dlopen libkpm.so — UI-only / smoke testing." >&2

    cargo zigbuild --release --no-default-features --features device -p kinstaller \
        --target "$TARGET" \
        --manifest-path "$REPO_ROOT/Cargo.toml"

    BINARY="$REPO_ROOT/target/$TARGET/release/kinstaller"
    if [[ ! -f "$BINARY" ]]; then
        echo "error: expected binary not found at $BINARY" >&2
        exit 1
    fi

    DIST_DIR="$REPO_ROOT/dist/$PLATFORM"
    mkdir -p "$DIST_DIR"
    cp "$BINARY" "$DIST_DIR/kinstaller"
    echo "Wrote $DIST_DIR/kinstaller"

    echo "==> Link audit"
    file "$DIST_DIR/kinstaller"
    if file "$DIST_DIR/kinstaller" | grep -q "statically linked"; then
        echo "OK: statically linked musl binary (dlopen unsupported)"
    else
        echo "error: expected a statically linked musl binary" >&2
        exit 1
    fi
}

build_kox() {
    local kox_root="${KOXTOOLCHAIN_ROOT:-$HOME/x-tools/x-tools}"
    case "$PLATFORM" in
        kindlehf)
            TARGET="armv7-unknown-linux-gnueabihf"
            KOX_PREFIX="arm-kindlehf-linux-gnueabihf"
            GLIBC_PIN="2.18"
            ;;
        kindlepw2)
            TARGET="armv7-unknown-linux-gnueabi"
            KOX_PREFIX="arm-kindlepw2-linux-gnueabi"
            GLIBC_PIN="2.7"
            ;;
    esac

    local kox_dir="$kox_root/$KOX_PREFIX"
    local gcc="$kox_dir/bin/${KOX_PREFIX}-gcc"

    if ! "$gcc" --version >/dev/null 2>&1; then
        echo "error: koxtoolchain gcc not runnable at $gcc" >&2
        echo "  Linux: ./scripts/setup-koxtoolchain.sh" >&2
        echo "  macOS: ./scripts/setup-orbstack.sh" >&2
        exit 1
    fi

    local profile=release
    [[ "$USE_DEV" -eq 1 ]] && profile=dev-device

    echo "==> Building kinstaller for $PLATFORM ($TARGET, koxtoolchain glibc, dlopen OK, profile=$profile)"
    echo "    linker: $gcc"

    local target_env
    target_env=$(echo "${TARGET//-/_}" | tr '[:lower:]' '[:upper:]')
    export "CC_${target_env}=$gcc"
    export "CXX_${target_env}=${kox_dir}/bin/${KOX_PREFIX}-g++"
    export "CARGO_TARGET_${target_env}_LINKER=$gcc"
    export "CARGO_TARGET_${target_env}_RUSTFLAGS=-C relocation-model=static"

    cargo build --profile "$profile" --no-default-features --features device -p kinstaller \
        --target "$TARGET" \
        --manifest-path "$REPO_ROOT/Cargo.toml"

    BINARY="$REPO_ROOT/target/$TARGET/$profile/kinstaller"
    if [[ ! -f "$BINARY" ]]; then
        echo "error: expected binary not found at $BINARY" >&2
        exit 1
    fi

    DIST_DIR="$REPO_ROOT/dist/$PLATFORM"
    mkdir -p "$DIST_DIR"
    cp "$BINARY" "$DIST_DIR/kinstaller"
    echo "Wrote $DIST_DIR/kinstaller"

    echo "==> Link audit"
    file "$DIST_DIR/kinstaller"
    if file "$DIST_DIR/kinstaller" | grep -q "statically linked"; then
        echo "error: expected a dynamically linked glibc binary" >&2
        exit 1
    fi
    glibc_audit "$DIST_DIR/kinstaller" "$GLIBC_PIN"
}

if [[ "$USE_MUSL" -eq 1 ]]; then
    build_musl
else
    build_kox
fi
