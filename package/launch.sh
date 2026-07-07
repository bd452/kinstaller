#!/bin/sh

cd "$(dirname "$0")" || exit 1

if [ -f /lib/ld-linux-armhf.so.3 ]; then
    PLAT=kindlehf
else
    PLAT=kindlepw2
fi

LOG=/mnt/us/kmc/kpm/tmp/kinstaller.log
: >"$LOG"

# Hand the framebuffer to kinstaller. Do not STOP awesome/cvm/KPPMainApp — a stopped
# process keeps its EVIOCGRAB on the touch device and kinstaller cannot grab it.
lipc-set-prop com.lab126.pillow disableEnablePillow disable 2>/dev/null
usleep 300000

./bin/"$PLAT"/kinstaller >>"$LOG" 2>&1
EXIT_CODE=$?

lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null

exit "$EXIT_CODE"
