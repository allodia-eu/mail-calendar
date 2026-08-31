#!/usr/bin/env bash
# Apple dev loop: rebuild the shared Rust core/bindings, build the app through Xcode, then launch
# it on macOS, a connected iPhone/iPad, or a simulator. The app renders the configured real
# account, so this script does not capture screenshots or dump UI logs.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"        # repo root
# Physical-device detection, the signing team, the Developer-Mode precondition and the devicectl
# launch live in scripts/dev/lib.sh, which scripts/dev/device.sh drives too, one copy of those
# traps rather than a second that drifts.
# shellcheck source=scripts/dev/lib.sh
source "$ROOT/scripts/dev/lib.sh"

PROJECT="$HERE/AllodiaMail.xcodeproj"
DERIVED_DATA="${DERIVED_DATA:-$HERE/build/DerivedData}"
CONFIGURATION="${CONFIGURATION:-Debug}"
BUNDLE_ID="$APPLE_BUNDLE_ID"   # from lib.sh, which resolves the brand (docs/branding.md)
PLATFORM="${PLATFORM:-macos}"
SIMULATOR="${SIMULATOR:-}"
# Where an iPhone/iPad build runs: auto (a connected device wins over a simulator), or pinned by
# --device / --simulator.
DESTINATION="auto"
DEVICE=""
BUILD_CORE=1
RUN_APP=1
LIST_SIMULATORS=0
LIST_DEVICES=0

usage() {
  cat <<'USAGE'
Usage: Scripts/build-and-run.sh [options]

On iphone/ipad the app runs on a CONNECTED iPhone/iPad when there is one, and on a simulator
otherwise. --device and --simulator pin that choice. --no-run always builds the simulator slice
unless --device says otherwise, so the gate's iOS build stays the one CI links.

Options:
  --platform <target>      Build target: macos, iphone, or ipad (default: macos).
  --macos                  Shortcut for --platform macos.
  --iphone                 Shortcut for --platform iphone.
  --ipad                   Shortcut for --platform ipad.
  --device [<udid>]        Use a physical iPhone/iPad (default: the only connected one).
  --simulator [<name|udid>] Use a simulator (default: first booted match of the platform's family).
  --list-devices           List connected iPhones/iPads and exit.
  --list-simulators        List available simulators and exit.
  --no-core                 Skip rebuilding the Rust XCFramework and generated Swift bindings.
  --no-run                  Build only; do not launch/install the app.
  --configuration <name>    Xcode configuration to build (default: Debug).
  --derived-data <path>     DerivedData path (default: clients/apple/build/DerivedData).
  -h, --help                Show this help.

Environment:
  PLATFORM                  Same as --platform.
  SIMULATOR                 Same as --simulator.
  MAILCAL_DEVICE            Same as --device <udid> (also read by scripts/dev/device.sh).
  CONFIGURATION             Same as --configuration.
  DERIVED_DATA              Same as --derived-data.

Examples:
  Scripts/build-and-run.sh
  Scripts/build-and-run.sh --iphone
  Scripts/build-and-run.sh --iphone --simulator            # a simulator even with a device plugged in
  Scripts/build-and-run.sh --ipad --simulator "iPad Pro 13-inch (M4)"
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --platform)
      PLATFORM="${2:?missing value for --platform}"
      shift 2
      ;;
    --macos)
      PLATFORM="macos"
      shift
      ;;
    --iphone)
      PLATFORM="iphone"
      shift
      ;;
    --ipad)
      PLATFORM="ipad"
      shift
      ;;
    # Both take an OPTIONAL value: with one they name the destination, without one they only say
    # which kind to use, which is how a simulator is asked for while a device is plugged in.
    --device)
      DESTINATION="device"
      if [[ $# -ge 2 && "$2" != -* ]]; then DEVICE="$2"; shift 2; else shift; fi
      ;;
    --simulator)
      DESTINATION="simulator"
      if [[ $# -ge 2 && "$2" != -* ]]; then SIMULATOR="$2"; shift 2; else shift; fi
      ;;
    --list-devices)
      LIST_DEVICES=1
      shift
      ;;
    --list-simulators)
      LIST_SIMULATORS=1
      shift
      ;;
    --no-core)
      BUILD_CORE=0
      shift
      ;;
    --no-run)
      RUN_APP=0
      shift
      ;;
    --configuration)
      CONFIGURATION="${2:?missing value for --configuration}"
      shift 2
      ;;
    --derived-data)
      DERIVED_DATA="${2:?missing value for --derived-data}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$PLATFORM" in
  macos|iphone|ipad) ;;
  ios)
    PLATFORM="iphone"
    ;;
  ipados)
    PLATFORM="ipad"
    ;;
  *)
    echo "Unknown platform: $PLATFORM" >&2
    usage >&2
    exit 2
    ;;
esac

if [[ "$PLATFORM" == "macos" && "$DESTINATION" != "auto" ]]; then
  echo "error: --device and --simulator apply to --iphone/--ipad, not --macos" >&2
  exit 2
fi

print_simulators() {
  printf 'Available simulators:\n'
  list_available_simulators | awk -F '\t' '{printf "  %-38s %-8s %s\n", $1, $3, $2}'
}

print_devices() {
  local devices
  devices="$(list_connected_devices)"
  if [[ -z "$devices" ]]; then
    printf 'No iPhone/iPad is connected.\n'
    return
  fi
  printf 'Connected devices:\n'
  printf '%s\n' "$devices" | awk -F '\t' '{printf "  %-42s %s\n", $1, $2}'
}

simulator_field_for_udid() {
  local udid="$1"
  local field="$2"
  list_available_simulators | awk -F '\t' -v udid="$udid" -v field="$field" '
    $1 == udid {
      if (field == "name") print $2;
      else if (field == "state") print $3;
      exit;
    }
  '
}

select_simulator_udid() {
  local family="$1"
  local requested="$2"
  local uuid
  local name
  local state

  if [[ -n "$requested" && "$requested" =~ ^[0-9A-Fa-f-]{36}$ ]]; then
    printf '%s\n' "$requested"
    return
  fi

  while IFS=$'\t' read -r uuid name state; do
    if [[ -n "$requested" ]]; then
      [[ "$name" == "$requested" || "$name" == *"$requested"* ]] || continue
      [[ "$state" == "Booted" ]] || continue
    else
      case "$family" in
        iphone) [[ "$name" == *iPhone* ]] || continue ;;
        ipad) [[ "$name" == *iPad* ]] || continue ;;
      esac
      [[ "$state" == "Booted" ]] || continue
    fi
    printf '%s\n' "$uuid"
    return
  done < <(list_available_simulators)

  while IFS=$'\t' read -r uuid name state; do
    if [[ -n "$requested" ]]; then
      [[ "$name" == "$requested" || "$name" == *"$requested"* ]] || continue
    else
      case "$family" in
        iphone) [[ "$name" == *iPhone* ]] || continue ;;
        ipad) [[ "$name" == *iPad* ]] || continue ;;
      esac
    fi
    printf '%s\n' "$uuid"
    return
  done < <(list_available_simulators)

  return 1
}

ensure_simulator_booted() {
  local udid="$1"
  local state

  state="$(simulator_field_for_udid "$udid" state)"
  if [[ "$state" != "Booted" ]]; then
    echo "==> Booting simulator: $(simulator_field_for_udid "$udid" name) ($udid)"
    xcrun simctl boot "$udid" >/dev/null 2>&1 || true
    xcrun simctl bootstatus "$udid" -b
  fi

  open -a Simulator >/dev/null 2>&1 || true
}

if [[ "$LIST_DEVICES" -eq 1 ]]; then
  print_devices
  exit 0
fi

if [[ "$LIST_SIMULATORS" -eq 1 ]]; then
  print_simulators
  exit 0
fi

# Decide between a connected iPhone/iPad and a simulator. A device that is plugged in is the one
# the developer meant, so it wins, but only when the app is actually going to be launched:
# `--no-run` is what scripts/dev/gate.sh builds, and that must stay the SIMULATOR slice CI links.
# --iphone/--ipad pick the simulator FAMILY; a physical device is whichever one is connected (a
# device's name is user-chosen, so it says nothing reliable about which family it belongs to).
resolve_destination() {
  [[ "$PLATFORM" != "macos" && "$DESTINATION" == "auto" ]] || return 0
  if [[ "$RUN_APP" -eq 0 ]]; then DESTINATION="simulator"; return 0; fi
  local udid
  if udid="$(device_udid)"; then
    DEVICE="$udid"
    DESTINATION="device"
    echo "==> Connected device: $(device_name "$udid"), targeting it (--simulator overrides)"
  else
    DESTINATION="simulator"
  fi
}

# The team a device build signs with, written where an Xcode build can read it too (project.yml
# reads MAILCAL_DEVELOPMENT_TEAM from clients/apple/signing.xcconfig). Xcode cannot run the
# detection this script does, so without this file its Run button fails on "requires a provisioning
# profile" however often the script has succeeded. Git-ignored: a team id is public, but per-machine.
write_signing_local_xcconfig() { # <team>
  local file="$HERE/signing.local.xcconfig" body
  body="// Written by Scripts/build-and-run.sh: the signing team a physical-device build uses.
// Git-ignored and per-machine; delete it to go back to no team (macOS and the simulator never
// read it). clients/apple/signing.local.sh holds the same id for packaging.
MAILCAL_DEVELOPMENT_TEAM = $1"
  # Only on a change: moving an xcconfig's mtime invalidates every build setting behind it, and
  # Xcode answers that by rebuilding everything.
  [[ -f "$file" && "$(cat "$file")" == "$body" ]] && return 0
  printf '%s\n' "$body" >"$file"
  echo "==> Wrote signing.local.xcconfig (team $1), Xcode's own device builds sign with it too"
}

resolve_destination

if [[ "$BUILD_CORE" -eq 1 ]]; then
  "$HERE/Scripts/build-core.sh"
fi

if command -v xcodegen >/dev/null 2>&1; then
  echo "==> Regenerating the Xcode project"
  (cd "$HERE" && xcodegen generate)
  brand_assert_expanded "$PROJECT/project.pbxproj"
  brand_assert_expanded "$HERE/App/Info.plist"
elif [[ ! -d "$PROJECT" ]]; then
  echo "error: $PROJECT is missing and xcodegen is not installed" >&2
  echo "Install XcodeGen or run this from a checkout that already has the generated project." >&2
  exit 1
fi

# Picks the STABLE local identity the built app is re-signed with. This is a Keychain concern, not
# a distribution one: a keychain ACL grant ("Always Allow") is bound to the accessing app's
# designated requirement, and an ad-hoc signature's requirement is its cdhash, which changes on
# every single rebuild. So an ad-hoc dev build re-prompts for every stored account, every time.
# Signing with a persistent certificate keeps one requirement across rebuilds, making the grant
# a one-off. The first "Apple Development" or "Developer ID" identity in the keychain is used;
# CODESIGN_IDENTITY pins a particular one, and CODESIGN_IDENTITY=- opts out deliberately.
find_codesign_identity() {
  local requested="${CODESIGN_IDENTITY:-}"
  local identities

  if [[ "$requested" == "-" ]]; then
    return
  fi

  identities="$(security find-identity -v -p codesigning 2>/dev/null || true)"

  if [[ -n "$requested" ]]; then
    if grep -qF "$requested" <<<"$identities"; then
      printf '%s\n' "$requested"
      return
    fi
    echo "warning: CODESIGN_IDENTITY was not found: $requested" >&2
  fi

  awk -F'"' '/Apple Development|Developer ID/ {print $2; exit}' <<<"$identities"
}

if [[ "$PLATFORM" == "macos" ]]; then
  SIGNING_IDENTITY="$(find_codesign_identity)"

  echo "==> Building AllodiaMail for macOS ($CONFIGURATION)"
  xcodebuild \
    -project "$PROJECT" \
    -scheme AllodiaMail \
    -destination "platform=macOS" \
    -configuration "$CONFIGURATION" \
    -derivedDataPath "$DERIVED_DATA" \
    build

  APP="$DERIVED_DATA/Build/Products/$CONFIGURATION/AllodiaMail.app"
  if [[ -n "$SIGNING_IDENTITY" ]]; then
    echo "==> Re-signing app with stable identity: $SIGNING_IDENTITY"
    codesign --force --deep --sign "$SIGNING_IDENTITY" --timestamp=none "$APP"
  else
    echo "warning: no persistent signing identity found; keeping Xcode's ad-hoc signature." >&2
    echo "         Its code requirement changes every rebuild, so macOS will re-prompt for" >&2
    echo "         Keychain access on each run. Install an Apple Development certificate," >&2
    echo "         or set CODESIGN_IDENTITY, to make the grant stick." >&2
  fi

  echo "==> Built: $APP"

  if [[ "$RUN_APP" -eq 1 ]]; then
    # Any MAILCAL_* dev var (the account switch, the deterministic launch hooks) must reach the
    # app, but `open` launches via LaunchServices, which strips the process environment. When one
    # is set, exec the bundle binary directly (in the background) so it inherits the environment.
    has_mailcal_env=0
    while IFS='=' read -r name _; do
      [[ "$name" == MAILCAL_* ]] && { has_mailcal_env=1; break; }
    done < <(env)
    # Replace any prior instance of THIS exact binary, so two processes never write one SQLite
    # store. A different-path instance, an installed Allodia Mail, is deliberately left alone.
    pkill -f "$APP/Contents/MacOS/AllodiaMail" 2>/dev/null || true
    if [[ "$has_mailcal_env" -eq 1 ]]; then
      echo "==> Launching AllodiaMail (forwarding MAILCAL_* dev env)"
      "$APP/Contents/MacOS/AllodiaMail" &
    else
      echo "==> Launching AllodiaMail"
      # `-n` is load-bearing. This bundle and an installed Allodia Mail share the identifier
      # `eu.allodia.mailcal`, and a plain `open <path>` asks LaunchServices for that identifier:
      # which can hand back the *installed* app. Observed exactly that: a run of this script left
      # /Applications/AllodiaMail.app running and the freshly built binary not running at all, so
      # the "dev build" under test was silently the shipped one. `-n` launches a new instance of
      # this bundle. (Direct exec would also be unambiguous, but it bypasses LaunchServices'
      # activation, the window can come up unfocused, or on another Space.)
      open -n "$APP"
    fi
    echo "==> Logs: $HOME/.local/share/mailcal/mailcal.log (rotates .1-.3, ~4 MB cap)"
  fi
elif [[ "$DESTINATION" == "device" ]]; then
  if [[ -z "$DEVICE" ]]; then
    DEVICE="$(device_udid)" ||
      die "no iPhone/iPad is connected, plug one in, name one with --device <udid>, or use --simulator"
  fi
  TEAM="$(signing_team)" ||
    die "no signing team for a device build, add an Apple Development identity (Xcode ▸ Settings ▸
Accounts ▸ Manage Certificates), or set DEVELOPMENT_TEAM=<id>"
  write_signing_local_xcconfig "$TEAM"
  TARGET_LABEL="$(device_name "$DEVICE")"
  TARGET_LABEL="${TARGET_LABEL:-$DEVICE}"

  # `id=<udid>` rather than `generic/platform=iOS` whenever the app will actually run: automatic
  # signing registers THAT device with the portal and reissues the development profile to include
  # it, which is what lets the install succeed on a device the portal has never seen. A build-only
  # pass needs none of that, and the generic destination does not wait for the device to be awake.
  if [[ "$RUN_APP" -eq 1 ]]; then
    require_dev_mode "$DEVICE" # otherwise xcodebuild only times out waiting for the destination
    BUILD_DESTINATION="id=$DEVICE"
  else
    BUILD_DESTINATION="generic/platform=iOS"
  fi

  echo "==> Building AllodiaMail for $TARGET_LABEL ($CONFIGURATION, team $TEAM)"
  # No CODE_SIGN_* on the command line: project.yml and signing.xcconfig carry them, so this build
  # and one started from Xcode's Run button are the same build, a signing problem cannot show up in
  # only one of them. -allowProvisioningUpdates is the one thing a command line has to add, so Xcode
  # may create or refresh the profile without a dialog nobody is there to answer.
  xcodebuild \
    -project "$PROJECT" \
    -scheme AllodiaMail \
    -destination "$BUILD_DESTINATION" \
    -configuration "$CONFIGURATION" \
    -derivedDataPath "$DERIVED_DATA" \
    -allowProvisioningUpdates \
    ARCHS=arm64 \
    build

  APP="$DERIVED_DATA/Build/Products/$CONFIGURATION-iphoneos/AllodiaMail.app"
  echo "==> Built: $APP"

  if [[ "$RUN_APP" -eq 1 ]]; then
    # The local Stalwart harness is loopback-only, so nothing on the device can reach it, a device
    # session runs against a real account added in the app (docs/debugging.md).
    case "${MAILCAL_DEV_ACCOUNT:-}" in
      stalwart*)
        warn "MAILCAL_DEV_ACCOUNT=$MAILCAL_DEV_ACCOUNT but the harness is loopback-only: a physical
         device cannot reach it. Add a real account in the app, or run on a simulator (--simulator)."
        ;;
    esac
    echo "==> Installing on $TARGET_LABEL"
    xcrun devicectl device install app --device "$DEVICE" "$APP" >/dev/null
    # Terminate BEFORE launching: a launch over a live process only re-foregrounds it, so no fresh
    # onAppear fires and a MAILCAL_* launch hook is silently ignored.
    device_terminate "$DEVICE"
    echo "==> Launching AllodiaMail on $TARGET_LABEL"
    # Forward any MAILCAL_* dev var (the account switch + the deterministic launch hooks); devicectl
    # takes them as a JSON object, which device_launch assembles.
    dev_env=()
    while IFS='=' read -r name _; do
      [[ "$name" == MAILCAL_* ]] && dev_env+=("$name=${!name}")
    done < <(env)
    device_launch "$DEVICE" ${dev_env[@]+"${dev_env[@]}"}
    echo "==> Logs: scripts/dev/device.sh logs   (the log stays in the app's container on the device)"
  fi
else
  SIMULATOR_UDID=""
  BUILD_DESTINATION="generic/platform=iOS Simulator"
  TARGET_LABEL="iOS Simulator"

  if [[ "$RUN_APP" -eq 1 ]]; then
    if ! SIMULATOR_UDID="$(select_simulator_udid "$PLATFORM" "$SIMULATOR")"; then
      echo "error: no matching $PLATFORM simulator found" >&2
      print_simulators >&2
      exit 1
    fi
    ensure_simulator_booted "$SIMULATOR_UDID"
    TARGET_LABEL="$(simulator_field_for_udid "$SIMULATOR_UDID" name)"
    BUILD_DESTINATION="platform=iOS Simulator,id=$SIMULATOR_UDID"
  fi

  echo "==> Building AllodiaMail for $TARGET_LABEL ($CONFIGURATION)"
  xcodebuild \
    -project "$PROJECT" \
    -scheme AllodiaMail \
    -destination "$BUILD_DESTINATION" \
    -configuration "$CONFIGURATION" \
    -derivedDataPath "$DERIVED_DATA" \
    ARCHS=arm64 \
    build

  APP="$DERIVED_DATA/Build/Products/$CONFIGURATION-iphonesimulator/AllodiaMail.app"
  echo "==> Built: $APP"

  if [[ "$RUN_APP" -eq 1 ]]; then
    echo "==> Installing on $TARGET_LABEL"
    xcrun simctl install "$SIMULATOR_UDID" "$APP"
    echo "==> Launching AllodiaMail on $TARGET_LABEL"
    # Forward any MAILCAL_* dev var (the account switch + the deterministic launch hooks) into the
    # app: simctl passes SIMCTL_CHILD_-prefixed variables to the launched process (prefix stripped).
    child_env=()
    while IFS='=' read -r name _; do
      [[ "$name" == MAILCAL_* ]] && child_env+=("SIMCTL_CHILD_$name=${!name}")
    done < <(env)
    if [[ ${#child_env[@]} -gt 0 ]]; then
      env "${child_env[@]}" xcrun simctl launch "$SIMULATOR_UDID" "$BUNDLE_ID"
    else
      xcrun simctl launch "$SIMULATOR_UDID" "$BUNDLE_ID"
    fi
    if CONTAINER="$(xcrun simctl get_app_container "$SIMULATOR_UDID" "$BUNDLE_ID" data 2>/dev/null)"; then
      echo "==> Logs: $CONTAINER/Library/Application Support/mailcal/mailcal.log (rotates .1-.3, ~4 MB cap)"
    fi
  fi
fi
