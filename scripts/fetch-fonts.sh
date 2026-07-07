#!/usr/bin/env bash
# Download bundled UI fonts into crates/kinstaller/fonts/.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FONTS_DIR="$REPO_ROOT/crates/kinstaller/fonts"
ARCHIVE_URL="https://github.com/liberationfonts/liberation-fonts/files/7261482/liberation-fonts-ttf-2.1.5.tar.gz"

REGULAR="$FONTS_DIR/LiberationSans-Regular.ttf"
BOLD="$FONTS_DIR/LiberationSans-Bold.ttf"
if [[ -f "$REGULAR" && -f "$BOLD" ]]; then
    echo "Fonts already present in $FONTS_DIR, skipping download"
    exit 0
fi

mkdir -p "$FONTS_DIR"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "Downloading Liberation Sans fonts…"
curl -fsSL "$ARCHIVE_URL" -o "$WORKDIR/fonts.tar.gz"
tar -xzf "$WORKDIR/fonts.tar.gz" -C "$WORKDIR"

cp "$WORKDIR/liberation-fonts-ttf-2.1.5/LiberationSans-Regular.ttf" "$FONTS_DIR/"
cp "$WORKDIR/liberation-fonts-ttf-2.1.5/LiberationSans-Bold.ttf" "$FONTS_DIR/"
cp "$WORKDIR/liberation-fonts-ttf-2.1.5/LICENSE" "$FONTS_DIR/FONT-LICENSE"

echo "Fonts installed to $FONTS_DIR"
