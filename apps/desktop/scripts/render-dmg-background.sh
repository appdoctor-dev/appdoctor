#!/usr/bin/env bash
# Renders the .dmg window background from assets/dmg/background.html at
# 1x and 2x (for Retina) with headless Chrome. The PNGs are committed, so the
# release workflow doesn't need Chrome; run this after changing the HTML.
set -euo pipefail
cd "$(dirname "$0")/../assets/dmg"

CHROME=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
for scale in 1 2; do
  out=background.png
  [[ $scale == 2 ]] && out=background@2x.png
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=$scale \
    --window-size=660,420 --screenshot="$PWD/$out" "file://$PWD/background.html" >/dev/null 2>&1
  echo "$out: $(sips -g pixelWidth -g pixelHeight "$out" | awk '/pixel/ {printf "%s ", $2}')"
done
