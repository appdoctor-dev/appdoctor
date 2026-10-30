#!/usr/bin/env bash
# Wraps the debug binary in a minimal AppDoctor.app so macOS treats it as an
# app: it gets a bundle identifier (accessibility, automation, keychain
# prompts name "AppDoctor"), a Dock icon and a proper menu bar name.
# Usage: apps/desktop/scripts/bundle-dev.sh [--features no-license-check]
set -euo pipefail
cd "$(dirname "$0")/../../.."

cargo build -p appdoctor-desktop "$@"
APP=target/debug/AppDoctor.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp apps/desktop/assets/app-icon/AppIcon.icns "$APP/Contents/Resources/"
# Copy then rename: overwriting a running binary in place can get it killed
# for an invalid code signature; a rename leaves the running copy intact.
cp target/debug/appdoctor "$APP/Contents/MacOS/appdoctor.new"
mv -f "$APP/Contents/MacOS/appdoctor.new" "$APP/Contents/MacOS/appdoctor"
VERSION=$(cargo pkgid -p appdoctor-desktop | sed 's/.*[#@]//')
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>appdoctor</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleIdentifier</key><string>app.appdoctor.desktop.dev</string>
  <key>CFBundleName</key><string>AppDoctor</string>
  <key>CFBundleDisplayName</key><string>AppDoctor</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
echo "$APP"
