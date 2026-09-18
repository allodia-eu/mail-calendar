#!/usr/bin/env bash
# The App Store half of purchasing, against Xcode's own simulated store.
#
# Its own script, and its own Xcode target, because MailcalKit's suite cannot host this: that one
# runs under `swift test`, which has no application for StoreKit to resolve products against and
# no way to load a StoreKit Configuration. `SKTestSession` needs both. Run from the repo root or
# anywhere:
#
#     clients/apple/Scripts/test-storekit.sh
#
# macOS only, and deliberately not a simulator: the rules under test are the client's own (what is
# outstanding, what gets finished and when), none of them differ by platform, and booting a
# simulator per run would buy nothing.
#
# ⚠️ What this cannot prove is the round trip. A locally simulated transaction is signed by a local
# test certificate, so Apple's App Store Server API has never heard of it and the account service
# answers `unknown_purchase`. That needs products in App Store Connect and a sandbox tester.
set -euo pipefail

# Bash 5 or newer, like every script in this tree (AGENTS.md, "Building & verifying").
if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  echo "       macOS ships bash 3.2 as /bin/bash: \`brew install bash\` puts 5 ahead of it" >&2
  exit 1
fi

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
cd "$HERE"

PROJECT="AllodiaMail.xcodeproj"
if [ ! -d "$PROJECT" ]; then
  if command -v xcodegen >/dev/null 2>&1; then
    xcodegen generate
  else
    printf 'error: %s is missing and xcodegen is not installed\n' "$PROJECT" >&2
    exit 1
  fi
fi

# ⚠️ The destination names this host's **own** architecture, and leaving it off is not the same
# thing. A bare `platform=macOS` matches both the arm64 and the Rosetta x86_64 destination, and
# xcodebuild then runs the suite once per match: two runs against the one StoreKit test store,
# each clearing the other's transactions, failing on assertions that hold perfectly well alone.
ARCH="$(uname -m)"

# The same derived data the app build uses, so the host app and the bindings are not built twice.
# Not `-quiet`: it swallows the assertion that failed and leaves only the test's name, which is
# the half that does not tell you anything.
exec xcodebuild test \
  -project "$PROJECT" \
  -scheme StoreKitTests \
  -destination "platform=macOS,arch=$ARCH" \
  -derivedDataPath build/DerivedData
