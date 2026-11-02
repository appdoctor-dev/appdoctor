#!/usr/bin/env bash
# Connects the admin's Analytics page to Google Search Console, which is where
# the searches that bring people to appdoctor.so come from. Once, beforehand:
#
# 1. Add appdoctor.so to Search Console as a Domain property
#    (search.google.com/search-console; Cloudflare can add the DNS record).
# 2. In Google Cloud (console.cloud.google.com), enable the "Google Search
#    Console API", create a service account and download a JSON key for it.
# 3. In Search Console → Settings → Users and permissions, add the service
#    account's email (the script prints it) with Restricted permission.
#
# Then, from the repository root:
#
#   apps/api/scripts/search-console.sh <key.json>
#
# The key becomes the API Worker's GSC_SERVICE_ACCOUNT secret. Delete the
# downloaded file afterwards.
set -euo pipefail
KEY=${1:?usage: apps/api/scripts/search-console.sh <service-account-key.json>}
KEY=$(cd "$(dirname "$KEY")" && pwd)/$(basename "$KEY")
cd "$(dirname "$0")/.."

EMAIL=$(node -e 'const k = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); if (!k.client_email || !k.private_key) process.exit(1); console.log(k.client_email)' "$KEY") ||
  { echo "$KEY isn't a service account key (it needs client_email and private_key)." >&2; exit 1; }

node -e 'process.stdout.write(JSON.stringify(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))))' "$KEY" |
  npx wrangler secret put GSC_SERVICE_ACCOUNT --env production

echo
echo "Done. If you haven't yet, add $EMAIL as a user of the appdoctor.so property in Search Console."
