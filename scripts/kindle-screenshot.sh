#!/usr/bin/env bash
# Capture the Kindle e-ink framebuffer over SSH and write a PNG on the Mac.
#
# Reads geometry from FBIOGET_VSCREENINFO via /tmp/fbinfo on the device when
# present (see scripts/build-fbinfo.sh). Falls back to sysfs (less reliable).
#
# Usage: kindle-screenshot.sh [host] [output.png]
set -euo pipefail

HOST="${1:-root@192.168.1.231}"
OUT="${2:-/tmp/kindle-screenshot.png}"
PASS="${KINDLE_SSH_PASS:-kindle}"

ssh_cmd() {
    sshpass -p "$PASS" ssh -o StrictHostKeyChecking=no "$HOST" "$@"
}

read -r WIDTH HEIGHT STRIDE XOFF YOFF ROTATE <<<"$(ssh_cmd '
if [ -x /tmp/fbinfo ]; then
  /tmp/fbinfo
elif [ -x /usr/local/bin/fbinfo ]; then
  /usr/local/bin/fbinfo
else
  MODE=$(head -1 /sys/class/graphics/fb0/modes)
  S=$(cat /sys/class/graphics/fb0/stride)
  R=$(cat /sys/class/graphics/fb0/rotate)
  W=$(echo "$MODE" | sed -n "s/U:\\([0-9]*\\)x\\([0-9]*\\).*/\\1/p")
  H=$(echo "$MODE" | sed -n "s/U:\\([0-9]*\\)x\\([0-9]*\\).*/\\2/p")
  if [ "$R" = "1" ] || [ "$R" = "3" ]; then
    echo "$H $W $S 0 0 $R"
  else
    echo "$W $H $S 0 0 $R"
  fi
fi
')"

echo "Framebuffer: ${WIDTH}x${HEIGHT} stride=${STRIDE} offset=${XOFF},${YOFF} rotate=${ROTATE}"

BYTES=$((STRIDE * HEIGHT))
SKIP=$((STRIDE * YOFF + XOFF))
RAW="$(mktemp /tmp/kindle-fb.XXXXXX.raw)"
trap 'rm -f "$RAW"' EXIT

ssh_cmd "dd if=/dev/fb0 bs=${STRIDE} skip=${YOFF} count=${HEIGHT} 2>/dev/null" >"$RAW"

python3 - "$RAW" "$OUT" "$WIDTH" "$HEIGHT" "$STRIDE" <<'PY'
import sys
from pathlib import Path

raw_path, out_path, w, h, stride = sys.argv[1:6]
w, h, stride = int(w), int(h), int(stride)

data = Path(raw_path).read_bytes()
expected = stride * h
if len(data) < expected:
    raise SystemExit(f"short read: got {len(data)} bytes, expected {expected}")

rows = []
for y in range(h):
    row = data[y * stride : y * stride + w]
    rows.append(row)

import struct, zlib

def chunk(tag, payload):
    return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

png = bytearray()
png += b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0))
raw = b"".join(b"\x00" + rows[y] for y in range(h))
png += chunk(b"IDAT", zlib.compress(raw, 9))
png += chunk(b"IEND", b"")

Path(out_path).write_bytes(png)
print(f"Wrote {out_path} ({w}x{h})")
PY
