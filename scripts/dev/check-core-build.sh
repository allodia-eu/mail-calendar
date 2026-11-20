#!/usr/bin/env bash
# Refuse a development core in what is about to be packaged (docs/ai.md rule 5).
#
#     scripts/dev/check-core-build.sh <library, executable, archive or directory>...
#
# A development core (a debug build, or one with `dev-harness`) offers the writing assistant to
# everyone, so no package may carry one. The core compiles in a marker saying which kind it is
# (crates/mailcal-bindings/src/core_build.rs), and this reads it from the bytes, so it needs no
# toolchain for the target and reads a shared library, a static archive, an executable or a whole
# bundle alike. Each argument must hold the production marker somewhere and the development marker
# nowhere: a core built before the marker existed, or one the linker dropped it from, is refused
# as well, because a check that passes on a missing marker passes on everything.
#
# The Android release build and the Windows packager read the same two strings
# (clients/android/app/build.gradle.kts, clients/windows/core-build.ps1).

if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  echo "       macOS ships bash 3.2 as /bin/bash: \`brew install bash\` puts 5 ahead of it" >&2
  exit 1
fi

set -euo pipefail

PRODUCTION="mailcal-core-build:production"
DEVELOPMENT="mailcal-core-build:development"

(( $# )) || { echo "usage: ${0##*/} <library, executable, archive or directory>..." >&2; exit 2; }

status=0
for target in "$@"; do
  if [[ ! -e "$target" ]]; then
    echo "error: $target does not exist, so the core it should hold cannot be checked" >&2
    status=1
  elif LC_ALL=C grep -r -a -q -F -- "$DEVELOPMENT" "$target"; then
    echo "error: $target holds a DEVELOPMENT core (a debug build, or one with dev-harness)." >&2
    echo "       It offers the writing assistant to everyone and must not be packaged: build the" >&2
    echo "       core with --release and without dev-harness (docs/ai.md rule 5)." >&2
    status=1
  elif ! LC_ALL=C grep -r -a -q -F -- "$PRODUCTION" "$target"; then
    echo "error: $target holds no production core marker: a core built before it existed, or" >&2
    echo "       one the linker dropped it from. Build the core again with --release." >&2
    status=1
  else
    echo "    production core: $target"
  fi
done
exit "$status"
