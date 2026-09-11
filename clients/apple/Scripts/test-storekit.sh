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

# The same derived data the app build uses, so the host app and the bindings are not built twice.
exec xcodebuild test \
  -project "$PROJECT" \
  -scheme StoreKitTests \
  -destination 'platform=macOS' \
  -derivedDataPath build/DerivedData \
  -quiet
