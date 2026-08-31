#!/usr/bin/env bash
# Physical iPhone/iPad debugging; the gap the simulator-only tooling leaves. Its headline use is
# exercising background mail sync + new-mail notifications on real hardware, where BGTaskScheduler
# actually runs (it can't on a simulator) and a banner can actually be seen. Debug builds only.
#
# The simulator boot path is scripts/dev/boot.sh; this is its physical-device sibling. It encodes
# the bits that otherwise cost a round of trial-and-error (all resolved by scripts/dev/lib.sh):
# the signing team (from the cert OU, not the cert id), the Developer-Mode precondition, and
# terminate-BEFORE-launch (a launch over a live process only re-foregrounds it, so no fresh onAppear
# fires the MAILCAL_* hook).
#
#   scripts/dev/device.sh doctor              # device / Developer Mode / signing team; run first
#   scripts/dev/device.sh build [--core]      # device-signed Debug build (--core rebuilds Rust)
#   scripts/dev/device.sh install
#   scripts/dev/device.sh run [KEY=VAL ...]   # terminate + FRESH launch (with MAILCAL_* hooks)
#   scripts/dev/device.sh logs [--grep RE]    # pull & print the on-device mailcal.log
#   scripts/dev/device.sh marks               # print notify_marks (background-sync high-water)
#   scripts/dev/device.sh bgsync              # one background pass; reports the mark before/after
#   scripts/dev/device.sh all                 # build + install + run
#
# Env: MAILCAL_DEVICE (udid), DEVELOPMENT_TEAM (team id), BGSYNC_WAIT (seconds, default 22).
# Note: the local Stalwart harness is loopback-only and NOT reachable from a physical device, so
# device testing uses a real stored account (add it in the app). Send it real mail for `bgsync` to
# detect; or use a simulator/emulator with `harness.sh deliver` for a fully self-serve loop.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

is_macos || die "physical iOS-device debugging needs macOS (Xcode + devicectl); this host is $(host_os)"

APPLE_DIR="$REPO_ROOT/clients/apple"
PROJECT="$APPLE_DIR/AllodiaMail.xcodeproj"
DERIVED_DATA="${DERIVED_DATA:-$APPLE_DIR/build/DerivedData}"
ARTIFACTS="$APPLE_DIR/Packages/MailcalKit/artifacts"
APP="$DERIVED_DATA/Build/Products/Debug-iphoneos/AllodiaMail.app"
BGSYNC_WAIT="${BGSYNC_WAIT:-22}"

DEVICE="$(device_udid)" || die "no physical iOS device found: connect one, or set MAILCAL_DEVICE=<udid>"
TEAM="$(signing_team)" || die "no signing team: set DEVELOPMENT_TEAM=<id> (or add an Apple Development identity in Xcode)"

cmd_doctor() {
  local model os dm
  model="$(xcrun devicectl device info details --device "$DEVICE" 2>&1 | sed -nE 's/.*marketingName: *(.+)/\1/p' | head -1)"
  os="$(xcrun devicectl device info details --device "$DEVICE" 2>&1 | sed -nE 's/.*osVersionNumber: *([0-9.]+).*/\1/p' | head -1)"
  dm="$(device_dev_mode "$DEVICE")"
  info "device:          $DEVICE"
  info "model / os:      ${model:-?} / ${os:-?}"
  info "developer mode:  ${dm:-unknown}$([[ "$dm" == enabled ]] || echo '  <-- must be ENABLED to install (Settings -> Privacy & Security -> Developer Mode)')"
  info "signing team:    $TEAM  (override with DEVELOPMENT_TEAM=<id>)"
  security find-identity -v -p codesigning 2>/dev/null | grep "Apple Development" | sed 's/^/      /' || true
  info "app built:       $([[ -d "$APP" ]] && echo "$APP" || echo '(not built yet: run: build)')"
}

cmd_build() {
  if [[ "${1:-}" == "--core" || ! -d "$ARTIFACTS/Mailcal.xcframework/ios-arm64" ]]; then
    info "building the Rust core (device slice)"
    "$APPLE_DIR/Scripts/build-core.sh"
  fi
  info "building AllodiaMail for the device (team $TEAM, automatic signing)"
  xcodebuild -project "$PROJECT" -scheme AllodiaMail \
    -destination "id=$DEVICE" -configuration Debug -derivedDataPath "$DERIVED_DATA" \
    -allowProvisioningUpdates \
    CODE_SIGN_STYLE=Automatic DEVELOPMENT_TEAM="$TEAM" "CODE_SIGN_IDENTITY=Apple Development" \
    ARCHS=arm64 build
  info "built: $APP"
}

cmd_install() {
  require_dev_mode "$DEVICE"
  [[ -d "$APP" ]] || die "no built app at $APP: run: device.sh build"
  info "installing on $DEVICE"
  xcrun devicectl device install app --device "$DEVICE" "$APP" >/dev/null
  info "installed $APPLE_BUNDLE_ID"
}

cmd_run() {
  require_dev_mode "$DEVICE"
  device_terminate "$DEVICE"
  info "launching fresh${*:+ with env: $*}"
  device_launch "$DEVICE" "$@"
}

cmd_logs() {
  local tmp; tmp="$(mktemp)"
  device_pull "$DEVICE" "$APP_LOG_REL" "$tmp" || die "no log on device yet (install + launch first)"
  if [[ "${1:-}" == "--grep" && -n "${2:-}" ]]; then grep -E "$2" "$tmp"; else cat "$tmp"; fi
}

cmd_marks() {
  local tmp; tmp="$(mktemp)"
  if device_pull "$DEVICE" "$APP_PREFS_REL" "$tmp"; then
    local m; m="$(notify_marks_of "$tmp")"
    info "notify_marks: ${m:-<none yet>}"
  else
    info "notify_marks: <no preferences.toml on device yet>"
  fi
}

# One background-sync pass via the MAILCAL_RUN_BGSYNC launch hook, reporting the per-account mark
# before/after so the outcome is unambiguous: SEEDED (first pass, no notification) vs DETECTED (a
# mark advanced -> new mail found -> a notification was posted; watch the device).
cmd_bgsync() {
  require_dev_mode "$DEVICE"
  local tmp before after; tmp="$(mktemp -d)"
  device_pull "$DEVICE" "$APP_PREFS_REL" "$tmp/before.toml" || true
  before="$(notify_marks_of "$tmp/before.toml")"
  info "marks before: ${before:-<none>}"
  info "triggering a pass (terminate + fresh launch, MAILCAL_RUN_BGSYNC=1)"
  device_terminate "$DEVICE"
  device_launch "$DEVICE" MAILCAL_RUN_BGSYNC=1
  info "waiting ${BGSYNC_WAIT}s (connect + the ~6s-delayed pass + sync)…"
  sleep "$BGSYNC_WAIT"
  device_pull "$DEVICE" "$APP_PREFS_REL" "$tmp/after.toml" || true
  after="$(notify_marks_of "$tmp/after.toml")"
  info "marks after:  ${after:-<none>}"
  if [[ -z "$before" && -n "$after" ]]; then
    info "RESULT: SEEDED: first pass set the high-water mark and reported nothing (by design)."
    info "        Send a new email to the account, then run 'bgsync' again to detect it."
  elif [[ -n "$after" && "$before" != "$after" ]]; then
    info "RESULT: DETECTED: a mark advanced, so new mail was found and a notification was posted."
    info "        Watch the device: the banner shows even in-foreground via the DEBUG presenter."
  else
    warn "RESULT: no mark change: no new mail since the last pass, or the pass didn't run."
    warn "        Inspect: device.sh logs --grep 'session start|refresh_mail'"
  fi
}

cmd="${1:-}"; shift || true
case "$cmd" in
  doctor)  cmd_doctor "$@" ;;
  build)   cmd_build "$@" ;;
  install) cmd_install "$@" ;;
  run)     cmd_run "$@" ;;
  logs)    cmd_logs "$@" ;;
  marks)   cmd_marks "$@" ;;
  bgsync)  cmd_bgsync "$@" ;;
  all)     cmd_build; cmd_install; cmd_run "$@" ;;
  ""|-h|--help) sed -nE '2,30{/^#/!q; s/^# ?//p}' "$0" ;;
  *) die "unknown command '$cmd' (doctor|build|install|run|logs|marks|bgsync|all)" ;;
esac
