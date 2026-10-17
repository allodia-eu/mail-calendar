#!/usr/bin/env bash
# MailcalKit's own suite: plain logic, no UI. The page<->date mapping, the zoom clamps, the all-day
# overflow rule, the localised copy.
#
#   Scripts/test-kit.sh                              # on the host, under `swift test`
#   Scripts/test-kit.sh --ios                        # on an iPhone simulator, under `xcodebuild`
#   Scripts/test-kit.sh --ios --simulator "iPhone 17"
#
# `swift test` builds for the host, so on its own it never compiles an `#if os(iOS)` branch, and a
# rule the phone decides differently (the words of a new-mail notification, for one) would go
# untested. `--ios` runs the same suite on a simulator: `$SIMULATOR`, else the booted iPhone, else
# the first iPhone available, the same choice `test-ui.sh` makes. It needs the core's simulator
# slice, which `build-and-run.sh --iphone` builds.
#
# All this adds to `swift test` is where SwiftPM builds. Its default `.build` sits inside the
# package and nothing caches it, so the suite recompiled MailcalBindings and all 115 files of
# MailcalUI from scratch on every run, beside a warm Xcode cache. Building under
# clients/apple/build, the directory CI caches, makes an unchanged tree about a second.
#
# A scratch path of its own, and NOT the app's derived data. Pointed at that, SwiftPM's build of
# MailcalUI and Xcode's overwrite each other in Build/Products/Debug, and from then on neither the
# suite nor the macOS app is ever up to date: measured at 26-32s apiece on every run, against
# 1-3s when they are kept apart.

# Bash 5 or newer, like every script in this tree (AGENTS.md, "Building & verifying").
if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  echo "       macOS ships bash 3.2 as /bin/bash: \`brew install bash\` puts 5 ahead of it" >&2
  exit 1
fi

set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
ON_IOS=0
SIMULATOR="${SIMULATOR:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ios) ON_IOS=1 ;;
    --simulator) SIMULATOR="${2:?--simulator needs a name}"; shift ;;
    -h | --help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "error: unknown option '$1' (want: --ios, --simulator <name>)" >&2; exit 1 ;;
  esac
  shift
done

cd "$HERE/Packages/MailcalKit"
if [[ "$ON_IOS" -eq 0 ]]; then
  exec swift test --scratch-path "$HERE/build/KitSPM"
fi

ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=scripts/dev/lib.sh
source "$ROOT/scripts/dev/lib.sh"
is_macos || die "an iOS simulator needs macOS"

if [[ -n "$SIMULATOR" ]]; then
  UDID="$(sim_udid_by_name "$SIMULATOR")" || die "no available simulator named '$SIMULATOR'"
else
  UDID="$(booted_sim_udid iphone)" || UDID="$(first_iphone_sim)" ||
    die "no iPhone simulator available; pass --simulator <name>"
fi

# A derived-data directory of its own, for the reason KitSPM has one: the app build's products
# would overwrite these and the other way round. Under clients/apple/build so CI's cache carries
# it. `-collect-test-diagnostics never` for the reason test-ui.sh gives: on Xcode 27 a failure
# otherwise waits ten minutes for diagnostics it never collects.
ARGS=(
  -scheme MailcalKit-Package -destination "id=$UDID"
  -derivedDataPath "$HERE/build/KitIOS" COMPILER_INDEX_STORE_ENABLE=NO
)

# Built before the simulator boots, as test-ui.sh does and for its reason: a booting simulator
# takes a three-core runner for itself.
info "building MailcalKitTests for $UDID"
xcodebuild build-for-testing "${ARGS[@]}"
xcrun simctl bootstatus "$UDID" -b >/dev/null
exec xcodebuild test-without-building "${ARGS[@]}" -collect-test-diagnostics never
