#!/bin/bash
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE="$SCRIPT_DIR/../xmes-mobile-pwa"

cp -r "$MOBILE/assets/icons" "$SCRIPT_DIR/assets/"
cp "$MOBILE/assets/favicon.ico" "$SCRIPT_DIR/assets/"
cp "$MOBILE/assets/tailwind.css" "$SCRIPT_DIR/assets/"
cp -r "$MOBILE/public/icons" "$SCRIPT_DIR/public/"

echo "Assets copied from xmes-mobile-pwa."
