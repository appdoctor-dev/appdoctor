#!/usr/bin/env bash
# Saves the Apple secrets the release workflow needs to sign and notarize the
# Mac app as secrets of the GitHub repo. Nothing secret is printed or written
# to disk. Run it from the repository root:
#
#   apps/desktop/scripts/apple-signing-secrets.sh <certificate.p12> <AuthKey_XXXXXXXXXX.p8> [owner/repo]
#
# You need:
# - certificate.p12: your "Developer ID Application" certificate with its
#   private key, exported from Keychain Access with a password.
# - AuthKey_….p8: an App Store Connect team API key (Developer access or
#   more), and its issuer ID, which the script asks for.
set -euo pipefail

if [[ $# -lt 2 ]]; then
  sed -n '6p' "$0" | sed 's/^#   //' >&2
  exit 1
fi
P12=$1
P8=$2
REPO=${3:-appdoctor-dev/appdoctor}
for file in "$P12" "$P8"; do
  [[ -f "$file" ]] || { echo "Can't find $file" >&2; exit 1; }
done

# The certificate's name, as codesign wants it.
IDENTITY=$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: .*\)"/\1/p' | head -1)
if [[ -z "$IDENTITY" ]]; then
  echo "No Developer ID Application certificate in your keychain. Create one in Xcode first." >&2
  exit 1
fi
echo "Certificate: $IDENTITY"

read -rsp "Password you gave the .p12: " P12_PASSWORD
echo

# Check the password opens the .p12 now rather than in a failed release.
CHECK=$(mktemp -d)
trap 'security delete-keychain "$CHECK/check.keychain-db" 2>/dev/null || true; rm -rf "$CHECK"' EXIT
security create-keychain -p check "$CHECK/check.keychain-db"
if ! security import "$P12" -k "$CHECK/check.keychain-db" -P "$P12_PASSWORD" >/dev/null 2>&1; then
  echo "That password doesn't open $P12." >&2
  exit 1
fi

KEY_ID=$(basename "$P8" .p8)
KEY_ID=${KEY_ID#AuthKey_}
read -rp "Issuer ID (App Store Connect → Users and Access → Integrations): " ISSUER

base64 -i "$P12" | gh secret set APPLE_CERTIFICATE --repo "$REPO"
printf '%s' "$P12_PASSWORD" | gh secret set APPLE_CERTIFICATE_PASSWORD --repo "$REPO"
printf '%s' "$IDENTITY" | gh secret set APPLE_SIGNING_IDENTITY --repo "$REPO"
gh secret set APPLE_API_KEY --repo "$REPO" < "$P8"
printf '%s' "$KEY_ID" | gh secret set APPLE_API_KEY_ID --repo "$REPO"
printf '%s' "$ISSUER" | gh secret set APPLE_API_ISSUER --repo "$REPO"

echo "Saved. The next release signs and notarizes the Mac app."
