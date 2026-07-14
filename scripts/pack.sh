#!/usr/bin/env bash
# Build the kinstaller .kpkg from cross-compiled binaries and package/ templates.
# Usage: pack.sh [--skip-build]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KPM_DEV_WRAPPER="$REPO_ROOT/scripts/kpm-dev"

if ! command -v python3 >/dev/null 2>&1; then
    echo "error: pack.sh requires python3 to synchronize the package version" >&2
    exit 1
fi

SKIP_BUILD=0
if [[ $# -eq 0 ]]; then
    :
elif [[ $# -eq 1 && "$1" == "--skip-build" ]]; then
    SKIP_BUILD=1
else
    echo "Usage: pack.sh [--skip-build]" >&2
    exit 1
fi

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

workspace_version() {
    awk '
        /^\[workspace\.package\]/ { in_ws=1; next }
        /^\[/ { in_ws=0 }
        in_ws && /^version = / {
            gsub(/^version = "|"$/, "")
            print
            exit
        }
    ' "$REPO_ROOT/Cargo.toml"
}

if [[ "$SKIP_BUILD" -eq 0 ]]; then
    "$REPO_ROOT/scripts/build-target.sh" kindlehf
    "$REPO_ROOT/scripts/build-target.sh" kindlepw2
fi

HF_BIN="$REPO_ROOT/dist/kindlehf/kinstaller"
PW2_BIN="$REPO_ROOT/dist/kindlepw2/kinstaller"
for bin in "$HF_BIN" "$PW2_BIN"; do
    if [[ ! -f "$bin" ]]; then
        echo "error: missing $bin (run without --skip-build or build targets first)" >&2
        exit 1
    fi
done

if [[ ! -d "$REPO_ROOT/package" ]]; then
    echo "error: package/ directory not found at $REPO_ROOT/package" >&2
    exit 1
fi

VERSION="$(workspace_version)"
if [[ -z "$VERSION" ]]; then
    echo "error: could not parse workspace.package.version from Cargo.toml" >&2
    exit 1
fi

PKG_DIR="$REPO_ROOT/dist/pkg"
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/bin/kindlehf" "$PKG_DIR/bin/kindlepw2"

echo "==> Staging package in $PKG_DIR"
cp -R "$REPO_ROOT/package/." "$PKG_DIR/"
cp "$HF_BIN" "$PKG_DIR/bin/kindlehf/kinstaller"
cp "$PW2_BIN" "$PKG_DIR/bin/kindlepw2/kinstaller"

echo "==> Syncing manifest.json version to $VERSION"
python3 - "$PKG_DIR/manifest.json" "$VERSION" <<'PY'
import json
import sys

manifest_path, version = sys.argv[1:3]
with open(manifest_path, encoding="utf-8") as f:
    manifest = json.load(f)
manifest["version"] = [int(part) for part in version.split(".")]
with open(manifest_path, "w", encoding="utf-8") as f:
    json.dump(manifest, f, indent=2)
    f.write("\n")
PY

OUTPUT="$REPO_ROOT/dist/kinstaller_${VERSION}_kindlehf-kindlepw2.kpkg"
RELEASE_METADATA="$REPO_ROOT/dist/release-metadata.json"
RELEASE_TAG="${KPM_RELEASE_TAG:-v${VERSION}}"
SOURCE_COMMIT="${GITHUB_SHA:-$(git -C "$REPO_ROOT" rev-parse HEAD)}"
RELEASE_BASE_URL="https://github.com/bd452/kinstaller/releases/download/${RELEASE_TAG}"

echo "==> Validating staged package"
"$KPM_DEV_WRAPPER" validate "$PKG_DIR"

echo "==> Packing $OUTPUT"
rm -f "$OUTPUT" "$RELEASE_METADATA"
"$KPM_DEV_WRAPPER" pack "$PKG_DIR" --output "$OUTPUT"

echo "==> Verifying $OUTPUT"
"$KPM_DEV_WRAPPER" verify "$OUTPUT"

echo "==> Writing $RELEASE_METADATA"
"$KPM_DEV_WRAPPER" release-metadata "$OUTPUT" \
    --base-url "$RELEASE_BASE_URL" \
    --repository bd452/kinstaller \
    --commit "$SOURCE_COMMIT" \
    --tag "$RELEASE_TAG" \
    --output "$RELEASE_METADATA"

HASH="$(sha256_of "$OUTPUT")"
echo
echo "Package: $OUTPUT"
echo "SHA-256: $HASH"
echo "Release metadata: $RELEASE_METADATA"
