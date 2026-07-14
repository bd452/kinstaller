#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESOLVER="$REPO_ROOT/scripts/kpm-dev"
ORIGINAL_PATH="$PATH"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

make_fake() {
    local directory="$1"
    mkdir -p "$directory"
    printf '%s\n' \
        '#!/usr/bin/env bash' \
        'if [[ "${1:-}" == "--version" ]]; then' \
        '    printf "kpm-dev %s\\n" "${FAKE_KPM_VERSION:-0.1.0}"' \
        'else' \
        '    printf "%s %s\\n" "$(basename "$(dirname "$0")")" "$*"' \
        'fi' > "$directory/kpm-dev"
    chmod +x "$directory/kpm-dev"
}

make_fake "$TMP_DIR/explicit"
make_fake "$TMP_DIR/path"

actual="$(
    KPM_DEV="$TMP_DIR/explicit/kpm-dev" \
        PATH="$TMP_DIR/path:$ORIGINAL_PATH" \
        "$RESOLVER" validate package
)"
if [[ "$actual" != "explicit validate package" ]]; then
    echo "error: KPM_DEV did not take precedence: $actual" >&2
    exit 1
fi

actual="$(
    env -u KPM_DEV \
        PATH="$TMP_DIR/path:$ORIGINAL_PATH" \
        "$RESOLVER" verify artifact.kpkg
)"
if [[ "$actual" != "path verify artifact.kpkg" ]]; then
    echo "error: PATH kpm-dev was not selected: $actual" >&2
    exit 1
fi

if KPM_DEV="$TMP_DIR/explicit/kpm-dev" FAKE_KPM_VERSION=9.9.9 \
    "$RESOLVER" validate package > "$TMP_DIR/mismatch.out" 2>&1; then
    echo "error: resolver accepted a mismatched devkit version" >&2
    exit 1
fi

if ! grep -q "expected kpm-dev 0.1.0" "$TMP_DIR/mismatch.out"; then
    echo "error: resolver did not explain the version mismatch" >&2
    exit 1
fi

echo "kpm-dev resolver tests passed"
