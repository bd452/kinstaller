#!/usr/bin/env bash
# Download KindleModding koxtoolchain cross-compilers (Linux x86_64 host only).
#
# Installs to ~/x-tools/x-tools/{arm-kindlehf-linux-gnueabihf,arm-kindlepw2-linux-gnueabi}
# matching KPM CI. Required for device builds with runtime dlopen of libkpm.so.
#
# Usage: setup-koxtoolchain.sh
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "koxtoolchain host tools are Linux x86_64 binaries." >&2
    echo "  macOS: ./scripts/setup-orbstack.sh" >&2
    echo "  Linux: re-run this script on a Linux host." >&2
    exit 1
fi

KOX_BASE="${KOXTOOLCHAIN_ROOT:-$HOME/x-tools}"
mkdir -p "$KOX_BASE"

kox_gcc() {
    local name=$1
    case "$name" in
        kindlehf) echo "$KOX_BASE/x-tools/arm-kindlehf-linux-gnueabihf/bin/arm-kindlehf-linux-gnueabihf-gcc" ;;
        kindlepw2) echo "$KOX_BASE/x-tools/arm-kindlepw2-linux-gnueabi/bin/arm-kindlepw2-linux-gnueabi-gcc" ;;
    esac
}

download() {
    local name=$1
    local url="https://github.com/KindleModding/koxtoolchain/releases/latest/download/${name}.tar.gz"
    if [[ -x "$(kox_gcc "$name")" ]] && "$(kox_gcc "$name")" --version >/dev/null 2>&1; then
        echo "==> $name already present under $KOX_BASE/x-tools"
        return 0
    fi
    echo "==> Downloading $name"
    wget -q "$url" -O - | tar -xzf - -C "$KOX_BASE"
}

download kindlehf
download kindlepw2

echo
echo "koxtoolchain ready under $KOX_BASE/x-tools"
echo "  kindlehf:  $KOX_BASE/x-tools/arm-kindlehf-linux-gnueabihf/bin/arm-kindlehf-linux-gnueabihf-gcc"
echo "  kindlepw2: $KOX_BASE/x-tools/arm-kindlepw2-linux-gnueabi/bin/arm-kindlepw2-linux-gnueabi-gcc"
