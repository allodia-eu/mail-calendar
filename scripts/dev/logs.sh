#!/usr/bin/env bash
# Read a client's diagnostic log (docs/logging.md). By default it FOLLOWS the log (stream new
# lines); run it under a background job so the app's log streams while you drive it:
#
#   scripts/dev/logs.sh macos                 # follow (Ctrl-C to stop)
#   scripts/dev/logs.sh android --dump        # print the current log once and exit
#   scripts/dev/logs.sh ipad --device         # pull & print from a PHYSICAL device (see device.sh)
#
# The core logs only counts / ids / durations / events; never mail content, addresses, or
# credentials; so the stream is safe to read and attach to a support request.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

[[ $# -ge 1 ]] || die "usage: logs.sh <macos|iphone|ipad|android|windows|linux> [--dump] [--device]"
platform="$(normalize_platform "$1")"; shift
DUMP=0; DEVICE_MODE=0
for a in "$@"; do
  case "$a" in
    --dump)   DUMP=1 ;;
    --device) DEVICE_MODE=1 ;;
    *) die "unknown flag '$a' (--dump|--device)" ;;
  esac
done

# A physical iPhone/iPad: pull the log out of the app data container (can't be streamed; re-run
# for fresh lines). Simulators keep the file/tail path below.
if [[ "$DEVICE_MODE" == 1 ]]; then
  case "$platform" in iphone|ipad) ;; *) die "--device applies only to iphone|ipad" ;; esac
  udid="$(device_udid)" || die "no physical iOS device: connect one or set MAILCAL_DEVICE=<udid>"
  tmp="$(mktemp)"
  device_pull "$udid" "$APP_LOG_REL" "$tmp" || die "no log on device yet: install + launch via scripts/dev/device.sh"
  info "device ($udid) log:"
  cat "$tmp"
  exit 0
fi

case "$platform" in
  macos)
    log="$(macos_log_path)"
    info "macOS log: $log"
    if [[ "$DUMP" == 1 ]]; then [[ -f "$log" ]] && cat "$log" || warn "no log yet at $log"; else tail -n +1 -F "$log"; fi
    ;;
  iphone|ipad)
    udid="$(booted_sim_udid "$platform")" || die "no booted $platform simulator: boot one first: scripts/dev/boot.sh $platform"
    container="$(sim_app_container "$udid")"
    [[ -n "$container" ]] || die "the app isn't installed on the booted simulator yet: run scripts/dev/boot.sh $platform"
    log="$container/Library/Application Support/mailcal/mailcal.log"
    info "simulator ($udid) log: $log"
    if [[ "$DUMP" == 1 ]]; then [[ -f "$log" ]] && cat "$log" || warn "no log yet at $log"; else tail -n +1 -F "$log"; fi
    ;;
  android)
    adb="$(adb_bin)"
    if [[ "$DUMP" == 1 ]]; then
      # The on-device rotating file (docs/logging.md); the full core log, not just Logcat.
      info "Android file log: /data/data/$ANDROID_PKG/files/logs/app.log"
      "$adb" exec-out run-as "$ANDROID_PKG" cat files/logs/app.log 2>/dev/null || warn "no log yet (is the app installed and launched?)"
    else
      # Live: the core's records are teed to Logcat under the Mailcal tag.
      info "following Logcat (tag Mailcal); for the full on-device file use --dump"
      "$adb" logcat -s Mailcal
    fi
    ;;
  linux)
    log="$(linux_log_path)"
    info "Linux log: $log"
    if [[ "$DUMP" == 1 ]]; then
      [[ -f "$log" ]] && cat "$log" || warn "no log yet at $log (is the app built and launched?)"
    else
      [[ -f "$log" ]] || warn "no log yet at $log: following; it will stream once the app writes"
      tail -n +1 -F "$log"
    fi
    ;;
  windows)
    # We're on the Windows host (normalize_platform enforces it), so Git Bash can read the app-data
    # log directly. Normalise the backslashes in %LOCALAPPDATA% to forward slashes so tail/cat
    # accept the path. --dump prints once; the default follows (tail -F waits if it's not there yet).
    base="${LOCALAPPDATA:-$HOME/AppData/Local}"; base="${base//\\//}"
    log="$base/Allodia/MailCalendar/logs/app.log"
    info "Windows log: $log"
    if [[ "$DUMP" == 1 ]]; then
      [[ -f "$log" ]] && cat "$log" || warn "no log yet at $log (is the app built and launched?)"
    else
      [[ -f "$log" ]] || warn "no log yet at $log: following; it will stream once the app writes"
      tail -n +1 -F "$log"
    fi
    ;;
esac
