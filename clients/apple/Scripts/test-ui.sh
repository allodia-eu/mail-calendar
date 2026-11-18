#!/usr/bin/env bash
# The Apple UI suite: XCUITest against the running app on a simulator. The tests are in
# clients/apple/UITests; what each one is for is in its own header.
#
# Each test finds, reads and presses through the app's test agent (docs/test-agent.md), over the
# socket the simulator shares with the Mac, and uses XCUITest for what the agent cannot give: the
# launch, real keys and chords, and a touch or gesture whose landing place is the question. This is
# the only gate in the repository that can drive a **touch gesture** on Apple. It does not replace
# `control.sh iphone agent`, which answers questions about an app that is already in a state someone
# cares about, in milliseconds and with no code; this relaunches the app and runs a script, which is
# what makes it repeatable and what makes it useless for exploring.
#
#   Scripts/test-ui.sh                      # the booted iPhone simulator (boots one if none is)
#   Scripts/test-ui.sh --simulator "iPhone 17 Pro"
#   Scripts/test-ui.sh --only NavigationBarTests   # one class, or one test
#   Scripts/test-ui.sh --no-core            # skip the Rust rebuild
#   Scripts/test-ui.sh --with-kit           # MailcalKitTests too, in the same run on the same simulator
#   Scripts/test-ui.sh --build-only         # build the app and both test bundles; boot nothing
#   Scripts/test-ui.sh --parallel 3         # classes spread over three clones of the simulator
#
# `--with-kit` is the cheap way to run MailcalKit's suite on iOS: the scheme's build has already
# compiled MailcalUI for this destination, where `test-kit.sh --ios` compiles it again into a
# derived-data directory of its own.
#
# `--parallel` pays only where cores are spare. Each worker is a clone of the simulator, a full iOS
# with its own system services, which macOS schedules on the efficiency cores, and a copy of the
# device's data (4 to 5 GB for one in daily use). On an eight-core Mac two workers took as long as
# one, and three saved a tenth of the time and made a timing test fail.
#
# Every test but one drives the in-memory showcase dataset, so it needs no Docker harness, no stored
# account and no network. The one is `AccountPageTests`, which drives the harness's linked accounts
# and skips, saying why, without `scripts/dev/harness.sh up`.
#
# Simulators only: the agent's socket is on the Mac, and an app on a physical device cannot reach
# it. Background delivery and notifications on a device are scripts/dev/device.sh's.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"        # repo root
# shellcheck source=scripts/dev/lib.sh
source "$ROOT/scripts/dev/lib.sh"

PROJECT="$HERE/AllodiaMail.xcodeproj"
DERIVED_DATA="${DERIVED_DATA:-$HERE/build/DerivedData}"
TARGET=AllodiaMailUITests
BUILD_CORE=1
SIMULATOR="${SIMULATOR:-}"
ONLY=()
WITH_KIT=0
BUILD_ONLY=0
WORKERS=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --simulator) SIMULATOR="${2:-}"; shift ;;
    --only) ONLY+=("${2:?--only needs a test class or class/method}"); shift ;;
    --no-core) BUILD_CORE=0 ;;
    --with-kit) WITH_KIT=1 ;;
    --build-only) BUILD_ONLY=1 ;;
    --parallel) WORKERS="${2:?--parallel needs a number of workers}"; shift
      [[ "$WORKERS" =~ ^[1-9][0-9]*$ ]] || die "--parallel needs a number of workers, not '$WORKERS'" ;;
    -h | --help) sed -n '2,35p' "$0"; exit 0 ;;
    *) die "unknown option '$1' (want: --simulator <name>, --only <test>, --no-core, --with-kit," \
      "--build-only, --parallel <workers>)" ;;
  esac
  shift
done

is_macos || die "the Apple UI suite needs macOS (Xcode + a simulator)"

if [[ -n "$SIMULATOR" ]]; then
  UDID="$(sim_udid_by_name "$SIMULATOR")" || die "no available simulator named '$SIMULATOR'"
else
  UDID="$(booted_sim_udid iphone)" || UDID="$(first_iphone_sim)" ||
    die "no iPhone simulator available; pass --simulator <name>"
fi

if [[ "$BUILD_CORE" -eq 1 ]]; then
  "$HERE/Scripts/build-core.sh" --no-device
else
  "$HERE/Scripts/composer-resource.sh"
fi

# The brand has to be in the environment before the project is generated, or every id in it is the
# literal `${MAILCAL_APP_ID}` (docs/branding.md); lib.sh has already loaded it.
if command -v xcodegen >/dev/null 2>&1; then
  (cd "$HERE" && xcodegen generate >/dev/null)
  brand_assert_expanded "$PROJECT/project.pbxproj"
elif [[ ! -d "$PROJECT" ]]; then
  die "$PROJECT is missing and xcodegen is not installed"
fi

# `-collect-test-diagnostics never`, because on Xcode 27 the default (`on-failure`) does not
# work and is not cheap about it: a failing test is followed by "Failure collecting diagnostics
# from simulator: Timed out after 600.0 seconds", so every red run costs ten extra minutes and then
# produces nothing. On the runner that is the difference between a job that reports a failed test
# and one the 20-minute bound cancels, which says nothing about what broke. Nothing here reads a
# sysdiagnose anyway: no result bundle is kept and no artefact is uploaded.
ARGS=(
  -project "$PROJECT" -scheme AllodiaMail
  -destination "id=$UDID" -configuration Debug
  -derivedDataPath "$DERIVED_DATA" COMPILER_INDEX_STORE_ENABLE=NO
)
TEST_ARGS=(-collect-test-diagnostics never)
if [[ "${#ONLY[@]}" -gt 0 ]]; then
  for test in "${ONLY[@]}"; do TEST_ARGS+=(-only-testing:"$TARGET/$test"); done
else
  TEST_ARGS+=(-only-testing:"$TARGET")
fi
[[ "$WITH_KIT" -eq 1 ]] && TEST_ARGS+=(-only-testing:MailcalKitTests)
if [[ "$WORKERS" -gt 1 ]]; then
  TEST_ARGS+=(-parallel-testing-enabled YES -parallel-testing-worker-count "$WORKERS")
else
  TEST_ARGS+=(-parallel-testing-enabled NO)
fi

# Built before the simulator boots, never while it does. A booting simulator takes a three-core
# runner for itself: booted first, as one `xcodebuild test` needs, it put ten minutes on GitHub's
# `xcode-27` image into a build the macOS leg finishes in under two. `test-without-building`
# reuses these products, so the split costs one more package resolution and nothing else.
info "building $TARGET"
xcodebuild build-for-testing "${ARGS[@]}"
[[ "$BUILD_ONLY" -eq 1 ]] && exit 0

# The keyboard switch below, compiled before the boot rather than by `xcrun swift` after it,
# when the new device's daemons hold a runner's three cores: there the step took 143s, against
# 3s to compile beforehand and 45s to run, which is the device finishing its boot. Kept in the
# build directory by the source's hash, so CI's cache of that directory carries it forward.
KEYBOARD_TOOL="$HERE/build/tools/sim-keyboard-$(shasum -a 256 "$ROOT/scripts/dev/sim-keyboard.swift" | cut -c1-16)"
if [[ ! -x "$KEYBOARD_TOOL" ]]; then
  t0=$SECONDS
  mkdir -p "$(dirname "$KEYBOARD_TOOL")"
  xcrun swiftc -o "$KEYBOARD_TOOL" "$ROOT/scripts/dev/sim-keyboard.swift"
  info "keyboard switch compiled in $((SECONDS - t0))s"
fi

info "simulator: $UDID (booting it if it is not already)"
t0=$SECONDS
xcrun simctl bootstatus "$UDID" -b >/dev/null
info "simulator ready after $((SECONDS - t0))s"
t0=$SECONDS
# The on-screen keyboard up, as on a phone: a simulated hardware keyboard keeps it down, and a
# simulator last driven by `idb ui text` or a Mac key press simulates one, so the suite would
# pass over a button the keyboard covers on every real iPhone.
"$KEYBOARD_TOOL" "$UDID" off >/dev/null ||
  echo "warning: could not switch the simulated hardware keyboard off; tests run as the simulator is" >&2
info "hardware keyboard off after $((SECONDS - t0))s"
# The harness's certificate, for the one class that drives the harness rather than the
# showcase (UITests/AccountPageTests.swift). xcodebuild hands a `TEST_RUNNER_` variable to the
# runner without its prefix, and the runner passes it on to the app it launches.
export TEST_RUNNER_MAILCAL_EXTRA_CA="$HARNESS_CA"

info "running $TARGET $([[ "${#ONLY[@]}" -gt 0 ]] && printf '%s ' "${ONLY[@]}" || printf '(every test)')"
exec xcodebuild test-without-building "${ARGS[@]}" "${TEST_ARGS[@]}"
