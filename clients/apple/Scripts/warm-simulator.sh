#!/usr/bin/env bash
# A simulator for the UI suite that has booted before, so a CI runner does not pay for its first
# boot. A device's first boot runs a data migration that `simctl bootstatus` waits on; a device
# whose data directory has been through one boot and settled skips it. Measured on an 8-core M2:
# `bootstatus` returned in 5s against 41s, and the device spent ~400 CPU-seconds in its first three
# minutes against ~620. The rest is the ~230 daemons every boot starts, which no snapshot removes.
#
# So this keeps the data directory of a device that booted once, and copies it into a new device
# before `test-ui.sh` boots that. A device given another's data directory boots as that one would:
# the udid is not in it.
#
#   Scripts/warm-simulator.sh key              # prints key=<cache key>, for $GITHUB_OUTPUT
#   Scripts/warm-simulator.sh prepare <dir>    # a device seeded from <dir>, or <dir> filled from it
#
# `prepare` prints SIMULATOR=<name>, for $GITHUB_ENV, which `test-ui.sh` reads. The device is one of
# its own, created under that name and deleted again on the next run, so no existing simulator is
# ever written to. Its type and runtime are those of the simulator `test-ui.sh` would otherwise pick.
#
# The key names the Xcode build, the runtime build and the device type: a data directory from
# another runtime is a migration still to run, or a device that does not boot.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"        # repo root
# shellcheck source=scripts/dev/lib.sh
source "$ROOT/scripts/dev/lib.sh"

NAME="Mailcal UI suite"
DEVICES="$HOME/Library/Developer/CoreSimulator/Devices"
# How long a cold device idles after `bootstatus` before its data is kept. The first-boot work goes
# on after it returns, mostly in the first three minutes, and a directory kept before then is a
# device that still has some of it to do.
SETTLE_SECONDS=180

is_macos || die "a simulator needs macOS"

# The type and runtime of the simulator `test-ui.sh` picks when it is given none.
template() {
  local udid plist
  udid="$(first_iphone_sim)" || die "no iPhone simulator available"
  plist="$DEVICES/$udid/device.plist"
  DEVICE_TYPE="$(plutil -extract deviceType raw "$plist")"
  RUNTIME="$(plutil -extract runtime raw "$plist")"
}

cmd_key() {
  template
  local xcode runtime_build
  xcode="$(xcodebuild -version | sed -n 's/^Build version //p')"
  runtime_build="$(xcrun simctl list runtimes | sed -nE "s/.* - ([0-9A-Za-z]+)\\) - ${RUNTIME//./\\.}\$/\\1/p")"
  [[ -n "$xcode" && -n "$runtime_build" ]] || die "could not read the Xcode or runtime build"
  printf 'key=ios-warm-simulator-v1-%s-%s-%s\n' "$xcode" "$runtime_build" "${DEVICE_TYPE##*.}"
}

cmd_prepare() {
  local dir="${1:?prepare needs the snapshot directory}" udid
  template
  while udid="$(sim_udid_by_name "$NAME")"; do
    xcrun simctl shutdown "$udid" 2>/dev/null || true
    xcrun simctl delete "$udid"
  done
  udid="$(xcrun simctl create "$NAME" "$DEVICE_TYPE" "$RUNTIME")"

  if [[ -d "$dir/data" ]]; then
    info "seeding $udid from the snapshot ($(du -sh "$dir/data" | cut -f1))" >&2
    rsync -a --delete "$dir/data/" "$DEVICES/$udid/data/"
  else
    info "no snapshot: booting $udid cold, then idling ${SETTLE_SECONDS}s before keeping it" >&2
    local t0=$SECONDS
    xcrun simctl bootstatus "$udid" -b >/dev/null
    info "cold boot finished in $((SECONDS - t0))s" >&2
    sleep "$SETTLE_SECONDS"
    xcrun simctl shutdown "$udid"
    mkdir -p "$dir"
    rsync -a --delete "$DEVICES/$udid/data/" "$dir/data/"
    info "kept $(du -sh "$dir/data" | cut -f1)" >&2
  fi
  printf 'SIMULATOR=%s\n' "$NAME"
}

case "${1:-}" in
  key) cmd_key ;;
  prepare) shift; cmd_prepare "$@" ;;
  *) die "usage: ${0##*/} key | prepare <dir>" ;;
esac
