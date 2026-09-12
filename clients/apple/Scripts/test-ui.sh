#!/usr/bin/env bash
# The Apple UI suite: XCUITest against the running app, on a simulator or on a physical
# iPhone/iPad. The tests are in clients/apple/UITests; what each one is for is in its own header.
#
# This is the only gate in the repository that can drive a **touch gesture** on Apple, and the only
# one that can read the iOS navigation bar at all: `idb` reports the top bar as one unlabelled
# group (.agents/skills/debug-app), so Compose, More, Select, Send and Cancel are out of its reach.
# It does not replace idb. idb answers questions about an app that is already in a state someone
# cares about, in about a second and with no code; this relaunches the app and runs a script, which
# is what makes it repeatable and what makes it useless for exploring.
#
#   Scripts/test-ui.sh                      # the booted iPhone simulator (boots one if none is)
#   Scripts/test-ui.sh --simulator "iPhone 17 Pro"
#   Scripts/test-ui.sh --device             # the connected iPhone/iPad
#   Scripts/test-ui.sh --only NavigationBarTests   # one class, or one test
#   Scripts/test-ui.sh --no-core            # skip the Rust rebuild
#
# Every test drives the in-memory showcase dataset, so nothing here needs the Docker harness, a
# stored account or a network. That is what lets the device leg run the same suite: the harness is
# loopback-only and a device cannot reach it (scripts/dev/device.sh).
#
# ⚠️ A physical device needs Settings -> Privacy & Security -> Developer Mode on, as every device
# build does. It also has a **Settings -> Developer -> Enable UI Automation** switch that nothing
# else here uses; it was on for the runs this was written against, so whether the suite would fail
# without it is untested. If a device run installs and then cannot attach, that is the first switch
# to look at.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"        # repo root
# shellcheck source=scripts/dev/lib.sh
source "$ROOT/scripts/dev/lib.sh"

PROJECT="$HERE/AllodiaMail.xcodeproj"
DERIVED_DATA="${DERIVED_DATA:-$HERE/build/DerivedData}"
ARTIFACTS="$HERE/Packages/MailcalKit/artifacts"
TARGET=AllodiaMailUITests
BUILD_CORE=1
ON_DEVICE=0
SIMULATOR="${SIMULATOR:-}"
ONLY=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --device) ON_DEVICE=1 ;;
    --simulator) SIMULATOR="${2:-}"; shift ;;
    --only) ONLY+=("${2:?--only needs a test class or class/method}"); shift ;;
    --no-core) BUILD_CORE=0 ;;
    -h | --help) sed -n '2,26p' "$0"; exit 0 ;;
    *) die "unknown option '$1' (want: --device, --simulator <name>, --only <test>, --no-core)" ;;
  esac
  shift
done

# The first available iPhone simulator, newest name last so a runner picks the newest it was given.
# By family rather than by name: this has to work on a GitHub runner, whose installed set is
# whatever that image shipped with, and a hardcoded model is a gate that stops running the day the
# image moves.
first_iphone_sim() {
  local uuid name state pick=""
  while IFS=$'\t' read -r uuid name state; do
    [[ "$name" == *iPhone* ]] && pick="$uuid"
  done < <(list_available_simulators)
  [[ -n "$pick" ]] || return 1
  printf '%s\n' "$pick"
}

is_macos || die "the Apple UI suite needs macOS (Xcode + a simulator or a connected device)"

if [[ "$ON_DEVICE" -eq 1 ]]; then
  UDID="$(device_udid)" || die "no physical iOS device found: connect one, or set MAILCAL_DEVICE=<udid>"
  TEAM="$(signing_team)" || die "no signing team: set DEVELOPMENT_TEAM=<id> (or add an Apple Development identity in Xcode)"
  require_dev_mode "$UDID"
  info "device: $UDID ($(device_name "$UDID")), team $TEAM"
  # The device slice of the core, which build-core.sh skips only when asked to.
  CORE_ARGS=()
  # ⚠️ `--no-core` cannot mean "skip it" here when the slice a device needs is not there. A
  # simulator-only core (`build-core.sh --no-device`, which is what CI and the gate build) leaves
  # the xcframework without `ios-arm64`, and the failure is four copies of "no library for this
  # platform was found", named against MailcalUI and the app rather than against the missing slice.
  # scripts/dev/device.sh builds it in the same case for the same reason.
  if [[ ! -d "$ARTIFACTS/Mailcal.xcframework/ios-arm64" ]]; then
    [[ "$BUILD_CORE" -eq 1 ]] || info "the core has no device slice, building it despite --no-core"
    BUILD_CORE=1
  fi
else
  if [[ -n "$SIMULATOR" ]]; then
    UDID="$(sim_udid_by_name "$SIMULATOR")" || die "no available simulator named '$SIMULATOR'"
  else
    UDID="$(booted_sim_udid iphone)" ||
      UDID="$(first_iphone_sim)" ||
      die "no iPhone simulator available; pass --simulator <name>"
  fi
  # Boot it if it is not booted, and wait either way until it says it has finished.
  #
  # ⚠️ `simctl boot` returns as soon as the boot has STARTED, and a freshly booted (or freshly
  # erased) device accepts an install several seconds before the services behind one are up. The
  # install then fails with `Couldn't communicate with a helper application` naming
  # `com.apple.installcoordinationd`, and XCUITest reports it against the first test in the bundle,
  # so it reads as that test failing rather than as the device not being ready. `bootstatus` is the
  # wait, and it is a no-op on a device that has already settled.
  info "simulator: $UDID"
  xcrun simctl bootstatus "$UDID" -b >/dev/null
  TEAM=""
  CORE_ARGS=(--no-device)
fi

# `${a[@]+"${a[@]}"}`, because macOS ships bash 3.2, where an EMPTY array under `set -u` is an
# unbound variable rather than nothing at all. The device leg passes no core flags, so that is
# the leg it breaks, and only on the machine a device is plugged into.
[[ "$BUILD_CORE" -eq 1 ]] && "$HERE/Scripts/build-core.sh" ${CORE_ARGS[@]+"${CORE_ARGS[@]}"}

# The brand has to be in the environment before the project is generated, or every id in it is the
# literal `${MAILCAL_APP_ID}` (docs/branding.md); lib.sh has already loaded it.
if command -v xcodegen >/dev/null 2>&1; then
  (cd "$HERE" && xcodegen generate >/dev/null)
  brand_assert_expanded "$PROJECT/project.pbxproj"
elif [[ ! -d "$PROJECT" ]]; then
  die "$PROJECT is missing and xcodegen is not installed"
fi

ARGS=(
  -project "$PROJECT" -scheme AllodiaMail
  -destination "id=$UDID" -configuration Debug
  -derivedDataPath "$DERIVED_DATA" COMPILER_INDEX_STORE_ENABLE=NO
)
if [[ "${#ONLY[@]}" -gt 0 ]]; then
  for test in "${ONLY[@]}"; do ARGS+=(-only-testing:"$TARGET/$test"); done
else
  ARGS+=(-only-testing:"$TARGET")
fi
[[ -n "$TEAM" ]] && ARGS+=(DEVELOPMENT_TEAM="$TEAM")

info "running $TARGET $([[ "${#ONLY[@]}" -gt 0 ]] && printf '%s ' "${ONLY[@]}" || printf '(every test)')"
exec xcodebuild test "${ARGS[@]}"
