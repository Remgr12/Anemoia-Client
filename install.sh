#!/usr/bin/env bash
# Anemoia Client installer
# Usage: ./install.sh [install_dir]
# Default install dir: ~/.anemoia
set -e

INSTALL_DIR="${1:-$HOME/.anemoia}"
DIST_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Installing Anemoia to $INSTALL_DIR"
mkdir -p "$INSTALL_DIR"

for f in anemoia-inject anemoia-launcher libanemoia_client.so libagent_loader.so; do
    if [ -f "$DIST_DIR/$f" ]; then
        cp "$DIST_DIR/$f" "$INSTALL_DIR/"
    else
        echo "WARNING: $f not found in $DIST_DIR — skipping"
    fi
done

chmod +x "$INSTALL_DIR/anemoia-inject" "$INSTALL_DIR/anemoia-launcher" 2>/dev/null || true

if [ -d "$DIST_DIR/scripts" ]; then
    cp -r "$DIST_DIR/scripts" "$INSTALL_DIR/"
else
    echo "WARNING: scripts/ not found — modules will not load"
fi

echo
echo "Done. Installed to $INSTALL_DIR"
echo "Launch: $INSTALL_DIR/anemoia-launcher"
echo "Or inject manually: $INSTALL_DIR/anemoia-inject [--pid <pid>]"
