#!/usr/bin/env bash
# Regenerate crates/kpm-sys/src/compat_table.rs from the pinned official KPM
# release artifact. Review and commit the resulting diff.
set -euo pipefail

# ---- Pinned release -----------------------------------------------------
# Update these (and the vendor/KPM submodule + kpm-sys types, if the ABI
# changed!) when verifying a new KPM release.
# libkpm.so for 0.2.1 and 0.2.2 is identical (0.2.2 is a package-version bump only).
KPM_VERSIONS=("0.2.1" "0.2.2")
ARTIFACT_URL="https://repo.kindlemodding.org/packages/kpm/artifacts/kpm_0.2.1_kindlehf-kindlepw2-compat.kpkg"
PLATFORMS=(kindlehf kindlepw2)
# --------------------------------------------------------------------------

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_FILE="$REPO_ROOT/crates/kpm-sys/src/compat_table.rs"
SUBMODULE_COMMIT="$(git -C "$REPO_ROOT/vendor/KPM" rev-parse HEAD 2>/dev/null || echo unknown)"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "Downloading $ARTIFACT_URL ..."
curl -fsSL "$ARTIFACT_URL" -o "$WORKDIR/kpm.kpkg"
tar -xf "$WORKDIR/kpm.kpkg" -C "$WORKDIR"

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

{
    cat <<HEADER
//! GENERATED FILE — do not edit by hand.
//!
//! Regenerate with \`scripts/gen-compat-table.sh\`, which downloads the pinned
//! official KPM release artifact, extracts each platform's \`libkpm.so\`, and
//! records its SHA-256 here. Review and commit the diff.
//!
//! Source artifact: $ARTIFACT_URL

use crate::compat::CompatEntry;

/// libkpm builds verified ABI-compatible with this Kinstaller's bindings
/// (\`vendor/KPM\` @ $SUBMODULE_COMMIT, kpm.h v0.2.x).
pub const COMPAT_TABLE: &[CompatEntry] = &[
HEADER
    for kpm_version in "${KPM_VERSIONS[@]}"; do
        for platform in "${PLATFORMS[@]}"; do
            LIB="$WORKDIR/kmc/$platform/lib/libkpm.so"
            if [[ ! -f "$LIB" ]]; then
                echo "error: $LIB missing from artifact" >&2
                exit 1
            fi
            HASH="$(sha256_of "$LIB")"
            echo "    (found $kpm_version/$platform: $HASH)" >&2
            cat <<ENTRY
    CompatEntry {
        kpm_version: "$kpm_version",
        platform: "$platform",
        sha256: "$HASH",
    },
ENTRY
        done
    done
    echo "];"
} >"$OUT_FILE"

echo "Wrote $OUT_FILE"
