#!/bin/sh

set -e

chmod +x bin/kindlehf/kinstaller bin/kindlepw2/kinstaller

cat > /mnt/us/documents/Kinstaller.sh << 'EOF'
#!/bin/sh
exec sh /mnt/us/kmc/kpm/packages/kinstaller/launch.sh
EOF
chmod +x /mnt/us/documents/Kinstaller.sh

echo "Kinstaller installed. Open Kinstaller.sh from Documents to launch."
