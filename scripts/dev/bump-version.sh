#!/usr/bin/env bash
# Bump the one app version: write /VERSION and update its two committed mirrors in lockstep, then
# verify with the drift guard. Everything else (Android versionCode/name, the Windows assembly
# version + Store package version, the Apple marketing version + build number) is DERIVED from
# /VERSION at build time, so this touches only the source of truth and the two files that cannot
# read it. See docs/versioning.md.
#
#     scripts/dev/bump-version.sh 0.3.0
#
# Committing and tagging are left to you on purpose; a `vX.Y.Z` tag is what triggers the Windows
# release workflow (.github/workflows/windows-release.yml), so you tag when you mean to release:
#
#     git commit -am "Bump version to 0.3.0" && git tag v0.3.0
set -euo pipefail

new="${1:-}"
if [[ -z "$new" ]]; then
  echo "usage: scripts/dev/bump-version.sh X.Y.Z" >&2
  exit 2
fi
if [[ ! "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: version must be MAJOR.MINOR.PATCH (got '$new')." >&2
  exit 2
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

old="$(tr -d '[:space:]' <VERSION 2>/dev/null || true)"

# 1. The source of truth.
printf '%s\n' "$new" >VERSION

# 2. Cargo.toml [workspace.package] version; the `^version = "…"` line is unique to that section
#    (dependencies use inline `{ version = … }` or the `name = "x"` shorthand, never a line-start
#    `version =`). sed -i.bak works on both BSD (macOS) and GNU sed.
sed -i.bak -E 's/^version = "[0-9]+\.[0-9]+\.[0-9]+"/version = "'"$new"'"/' Cargo.toml && rm -f Cargo.toml.bak

# 3. project.yml MARKETING_VERSION (the `$(MARKETING_VERSION)` Info.plist reference has no
#    colon-space-quote, so this rewrites only the real setting).
sed -i.bak -E 's/(MARKETING_VERSION:[[:space:]]*)"[0-9]+\.[0-9]+\.[0-9]+"/\1"'"$new"'"/' \
  clients/apple/project.yml && rm -f clients/apple/project.yml.bak

# 4. Prove it.
scripts/ci/check-version-sync.sh

echo "Bumped ${old:-<none>} -> $new. Review 'git diff', then commit and tag (e.g. git tag v$new) when releasing."
