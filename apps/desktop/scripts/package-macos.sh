#!/usr/bin/env bash
# Builds the release AppDoctor.app from one or more release binaries (Apple
# silicon and Intel are merged into one universal binary) and wraps it in
# AppDoctor-macOS.dmg. Used by .github/workflows/release.yml.
#
# Usage: apps/desktop/scripts/package-macos.sh <version> <out dir> <binary>...
#
# Signs with $APPLE_SIGNING_IDENTITY ("Developer ID Application: …") when it's
# set. Without it the app is signed ad hoc: it runs, but macOS asks before
# opening it the first time.
set -euo pipefail

VERSION=$1
OUT=$2
shift 2

APP="$OUT/AppDoctor.app"
DMG="$OUT/AppDoctor-macOS.dmg"
rm -rf "$APP" "$DMG"
mkdir -p "$APP/Contents/MacOS"

lipo -create -output "$APP/Contents/MacOS/appdoctor" "$@"
# CI artifacts lose the executable bit on the way between jobs.
chmod +x "$APP/Contents/MacOS/appdoctor"
lipo -info "$APP/Contents/MacOS/appdoctor"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>appdoctor</string>
  <key>CFBundleIdentifier</key><string>app.appdoctor.desktop</string>
  <key>CFBundleName</key><string>AppDoctor</string>
  <key>CFBundleDisplayName</key><string>AppDoctor</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$APP"
else
  codesign --force --sign - "$APP"
fi
codesign --verify --strict "$APP"

# The disk image opens to the app next to an Applications link, so installing
# is a drag.
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname AppDoctor -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$DMG"
fi
echo "$DMG"
