#!/usr/bin/env bash
# Makes the Ed25519 key pair that signs AppDoctor updates. Run it once, before
# the first release, from the repository root:
#
#   apps/desktop/scripts/update-signing-key.sh [owner/repo]
#
# - The private key becomes the UPDATE_SIGNING_KEY secret of the GitHub repo,
#   where only the release workflow can read it. A copy goes in your login
#   keychain ("AppDoctor update signing key") in case you need it again.
# - The public key goes in apps/desktop/update-signing-key.pub. Commit it: the
#   app is built with it and installs only updates signed by the private key.
#
# Replacing the key means apps built with the old public key stop installing
# updates by themselves, so keep it.
set -euo pipefail
cd "$(dirname "$0")/../../.."

REPO=${1:-appdoctor-dev/appdoctor}
PUBLIC=apps/desktop/update-signing-key.pub

if [[ -s "$PUBLIC" ]]; then
  echo "$PUBLIC already has a key. Delete its contents first if you really want a new one." >&2
  exit 1
fi

DIR=$(mktemp -d)
trap 'rm -rf "$DIR"' EXIT

node --input-type=module -e '
  import { generateKeyPairSync } from "node:crypto"
  import { writeFileSync } from "node:fs"
  const { privateKey, publicKey } = generateKeyPairSync("ed25519")
  writeFileSync(process.argv[1], privateKey.export({ type: "pkcs8", format: "pem" }), { mode: 0o600 })
  writeFileSync(process.argv[2], publicKey.export({ format: "jwk" }).x + "\n")
' "$DIR/private.pem" "$PUBLIC"

gh secret set UPDATE_SIGNING_KEY --repo "$REPO" < "$DIR/private.pem"
security add-generic-password -U -a "$REPO" -s "AppDoctor update signing key" -w "$(cat "$DIR/private.pem")"

echo "Saved the private key to $REPO's secrets and your keychain."
echo "Public key, now in $PUBLIC: $(cat "$PUBLIC")"
