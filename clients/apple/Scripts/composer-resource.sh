#!/usr/bin/env bash
# Puts the current rich-composer editor into MailcalUI's SPM resources, for a build that skips the
# Rust core (`--no-core`). The bundle belongs to the composer, not to the core, so skipping the core
# must not skip it: without this, an edit under clients/composer/src builds green and the app runs
# the previous editor. build-core.sh does the same as part of a full build.
#
# The copy is written only when its bytes moved, so an unchanged editor recompiles nothing.

# Bash 5 or newer, like every script in this tree (AGENTS.md, "Building & verifying").
if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  echo "       macOS ships bash 3.2 as /bin/bash: \`brew install bash\` puts 5 ahead of it" >&2
  exit 1
fi

set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"        # repo root

bash "$ROOT/scripts/dev/composer-bundle.sh"
src="$ROOT/clients/composer/dist/editor.html"
dst="$HERE/Packages/MailcalKit/Sources/MailcalUI/composer/editor.html"
mkdir -p "$(dirname "$dst")"
cmp -s "$src" "$dst" || cp "$src" "$dst"
