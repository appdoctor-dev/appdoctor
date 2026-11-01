#!/usr/bin/env bash
# Raises the app's version: [workspace.package] in Cargo.toml and the
# workspace crates' entries in Cargo.lock. Prints the new version. The Release
# workflow runs it when started by hand; it works locally too:
#
#   apps/desktop/scripts/bump-version.sh patch|minor|major|<x.y.z>
set -euo pipefail
cd "$(dirname "$0")/../../.."

# Windows checkouts may have CRLF line endings, hence the \r handling.
old=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1 | tr -d '\r')
IFS=. read -r major minor patch <<<"$old"
case ${1:-patch} in
  major) new="$((major + 1)).0.0" ;;
  minor) new="$major.$((minor + 1)).0" ;;
  patch) new="$major.$minor.$((patch + 1))" ;;
  [0-9]*.[0-9]*.[0-9]*) new=$1 ;;
  *) echo "usage: $0 patch|minor|major|<x.y.z>" >&2; exit 2 ;;
esac

# Only the first `version = ` line, which is [workspace.package]'s.
OLD=$old NEW=$new perl -0pi -e 's/^version = "\Q$ENV{OLD}\E"(?=\r?$)/version = "$ENV{NEW}"/m' Cargo.toml
# Workspace crates are the Cargo.lock entries without a `source`.
OLD=$old NEW=$new perl -0pi -e '$_ = join "", map {
  /^source = /m ? $_ : s/^version = "\Q$ENV{OLD}\E"(?=\r?$)/version = "$ENV{NEW}"/mr
} split /(?=^\[\[package\]\])/m' Cargo.lock

echo "$new"
