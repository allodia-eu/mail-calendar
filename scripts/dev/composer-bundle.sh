#!/usr/bin/env bash
# Rebuilds the shared rich-composer editor bundle (clients/composer/dist/editor.html) from its
# TypeScript sources, for a client build that is about to package it.
#
# The bundle is a committed build output: every host loads that one file, so generating it inside
# the client build would make bun a prerequisite of cargo, MSBuild and Gradle. This is the other
# half of that bargain. The client *dev* scripts may depend on bun, and they call this first, so an
# edit under clients/composer/src reaches the app you are about to run instead of the app silently
# running the previous artifact; the same failure `--no-core` used to produce on Apple: green
# build, app launches, and you verify the OLD behaviour believing you tested the new one.
#
# Without bun it says so and continues. Loudly, because a skip that reads like a pass is exactly how
# a stale editor gets signed off.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSER="$ROOT/clients/composer"

if ! command -v bun >/dev/null 2>&1; then
  echo "==> Composer editor: bun is not installed: using the committed dist/editor.html AS IS."
  echo "    If you changed clients/composer/src, install bun and rebuild, or you are testing the"
  echo "    previous bundle: https://bun.sh"
  exit 0
fi

# `build.ts` needs no dependencies (only Bun's own APIs), so this never touches the network,
# `bun install` belongs to the test/typecheck steps in gate.sh, not to a client build.
if (cd "$COMPOSER" && bun run build.ts --check >/dev/null 2>&1); then
  echo "==> Composer editor: dist/editor.html is up to date"
else
  (cd "$COMPOSER" && bun run build.ts >/dev/null)
  echo "==> Composer editor: REBUILT dist/editor.html from clients/composer/src: commit it"
fi
