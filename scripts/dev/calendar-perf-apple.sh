#!/usr/bin/env bash
# Measure the Apple calendar grid's frame budget (docs/calendar.md §7 and §9): build Release on a
# release core, open the calendar in the trace's scripted-motion mode, and print the trace's lines.
#
#   scripts/dev/calendar-perf-apple.sh mac          # the Mac: one line per axis driven
#   scripts/dev/calendar-perf-apple.sh mac-profile  # the same run under the Time Profiler
#   scripts/dev/calendar-perf-apple.sh device       # the connected iPhone/iPad, unlocked
#
#   --no-build   measure what is already built (the last Release build of this checkout)
#
# Scripted motion (MAILCAL_CALENDAR_TRACE=drive) moves the hours, then the days, then a zoom on both
# axes, the same way on every run, so it compares two builds. It moves the grid's position directly,
# so it says nothing about the gesture that would have asked for it: whether the grid feels right is
# judged by hand with MAILCAL_CALENDAR_TRACE=1. Each line reads
#
#   <axis>  <frames> in motion at <Hz>, <n> dropped (<%>), gap p50 p90 p99 worst; inputs; weeks built
#
# where `settle` is the grid moving on its own (the calendar opening, a page landing).
#
# A Release app opens on the mailbox and runs against the developer's own accounts; drive mode
# opens the calendar itself. On the Mac it refuses to start while another copy of the app is
# running, because two processes would write one store.
#
# Instruments' command line cannot attach to an app on a phone, so `mac-profile` is Mac-only: the
# trace's lines are the phone's instrument. The app is launched from an empty environment, because
# `open` passes the caller's on and a trace stores the recorded app's, tokens included.
#
# Env: DRIVE_TIMEOUT (seconds to wait for the drive to finish, default 120), MAILCAL_DEVICE (udid).
set -euo pipefail
# shellcheck source=scripts/dev/lib.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

is_macos || die "the Apple frame budget is measured on macOS; this host is $(host_os)"

APPLE_DIR="$REPO_ROOT/clients/apple"
DERIVED_DATA="${DERIVED_DATA:-$APPLE_DIR/build/DerivedData}"
MAC_APP="$DERIVED_DATA/Build/Products/Release/$APPLE_PRODUCT_NAME.app"
MAC_BIN="$MAC_APP/Contents/MacOS/$APPLE_PRODUCT_NAME"
MAC_LOG="$(macos_log_path)"
DONE_MARKER='calendar frames: scripted motion done'
SESSION_MARKER='--- session start'
DRIVE_TIMEOUT="${DRIVE_TIMEOUT:-120}"
TMP_ROOT="${TMPDIR:-/tmp}"; TMP_ROOT="${TMP_ROOT%/}"

usage() { sed -nE '2,27{/^#/!q; s/^# ?//p;}' "$0"; }

MODE="${1:-}"; shift || true
BUILD=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build) BUILD=0 ;;
    *) die "unknown option: $1" ;;
  esac
  shift
done

# Builds in this checkout share one xcframework, one DerivedData and one target directory, and two
# cargo builds at once over them corrupt the incremental cache. So a build step holds this lock, and
# a second run (`mac` beside `device`) waits for the first rather than building beside it.
BUILD_LOCK="$APPLE_DIR/build/.calendar-perf-apple.lock"
with_build_lock() {
  mkdir -p "$(dirname "$BUILD_LOCK")"
  local waited=0
  until mkdir "$BUILD_LOCK" 2>/dev/null; do
    local holder; holder="$(cat "$BUILD_LOCK/pid" 2>/dev/null || true)"
    if [[ -n "$holder" ]] && ! kill -0 "$holder" 2>/dev/null; then
      rm -rf "$BUILD_LOCK"; continue
    fi
    (( waited++ % 30 == 0 )) && info "waiting for another build in this checkout (pid ${holder:-?})"
    sleep 1
  done
  echo $$ >"$BUILD_LOCK/pid"
  # Released by a trap rather than after a status check: `"$@" || …` would switch `set -e` off
  # inside the build, and a failed core build would carry on into the app build.
  trap 'rm -rf "$BUILD_LOCK"' EXIT
  "$@"
  rm -rf "$BUILD_LOCK"
  trap - EXIT
}

build_mac() {
  info "building the Rust core (release; device and macOS slices)"
  "$APPLE_DIR/Scripts/build-core.sh" --release --slices device,macos
  info "building the Mac app (Release)"
  "$APPLE_DIR/Scripts/build-and-run.sh" --macos --no-core --no-run --configuration Release
}

# The trace's lines from the last launch in <log text>, one per burst, labelled with the axis the
# burst's inputs drove; nothing when that launch has not finished its drive.
run_lines() { # <log text>
  awk -v session="$SESSION_MARKER" -v done="$DONE_MARKER" '
    index($0, session) { n = 0; finished = 0; next }
    index($0, done) { finished = 1; next }
    /calendar frames: [0-9]+ in motion/ { line[++n] = $0 }
    END {
      if (!finished) exit
      for (i = 1; i <= n; i++) {
        s = line[i]; sub(/.*calendar frames: /, "", s)
        match(s, /days [0-9]+, hours [0-9]+, zoom [0-9]+/)
        split(substr(s, RSTART, RLENGTH), f, /[ ,]+/)
        axis = "settle"; best = 0
        if (f[2] + 0 > best) { axis = "days"; best = f[2] + 0 }
        if (f[4] + 0 > best) { axis = "hours"; best = f[4] + 0 }
        if (f[6] + 0 > best) { axis = "zoom"; best = f[6] + 0 }
        printf "  %-6s  %s\n", axis, s
      }
    }' <<<"$1"
}

mac_pid() { pgrep -f "$MAC_BIN" | head -1 || true; }

# Any other copy of the app: an installed one, or another checkout's build.
other_mac_instances() {
  pgrep -fl "/$APPLE_PRODUCT_NAME.app/Contents/MacOS/$APPLE_PRODUCT_NAME" | { grep -vF "$MAC_BIN" || true; }
}

quit_mac() {
  local pid; pid="$(mac_pid)"
  [[ -n "$pid" ]] || return 0
  kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do kill -0 "$pid" 2>/dev/null || return 0; sleep 0.5; done
  die "the Release app (pid $pid) did not quit; quit it and re-run"
}

# Launches the Release app in drive mode; prints its pid. The log offset is taken first, so a line
# from an earlier run never counts for this one.
launch_mac() {
  local others; others="$(other_mac_instances)"
  [[ -z "$others" ]] || die "another copy of the app is running, and two would write one store; quit it first:
$others"
  [[ -x "$MAC_BIN" ]] || die "no Release build at $MAC_APP: run without --no-build"
  quit_mac
  # `open` hands the app the caller's whole environment, which a trace then records, so the app is
  # launched from an empty one: no token reaches a recording, and no MAILCAL_* switch left in the
  # shell changes what is measured.
  env -i HOME="$HOME" PATH="$PATH" TMPDIR="$TMP_ROOT" \
    open -n --env MAILCAL_CALENDAR_TRACE=drive \
    --stdout "$TMP_ROOT/allodia-mail-launch.log" --stderr "$TMP_ROOT/allodia-mail-launch.log" "$MAC_APP"
  local pid=""
  for _ in $(seq 1 40); do pid="$(mac_pid)"; [[ -n "$pid" ]] && break; sleep 0.25; done
  [[ -n "$pid" ]] || die "the app did not start; see $TMP_ROOT/allodia-mail-launch.log"
  printf '%s' "$pid"
}

# Waits for the drive to finish; on a timeout or an exit, quits the app and says which.
wait_for_drive() { # <offset> <pid>
  local rc=0
  wait_for_log_marker "$MAC_LOG" "$1" "$DONE_MARKER" "$DRIVE_TIMEOUT" "$2" || rc=$?
  [[ "$rc" -eq 0 ]] && return 0
  quit_mac
  [[ "$rc" -eq 1 ]] && die "no '$DONE_MARKER' in ${DRIVE_TIMEOUT}s. The drive opens the calendar
       once the window is up; if it never showed the calendar, look at $MAC_LOG"
  die "the app exited before the drive finished; see $MAC_LOG"
}

print_run() { # <log text>
  local lines; lines="$(run_lines "$1")"
  [[ -n "$lines" ]] || die "the run finished but logged no 'calendar frames' line"
  printf '%s\n' "$lines"
}

# Other work on the machine moves these numbers, so each run says how busy the machine was.
report_load() { info "load average: $(sysctl -n vm.loadavg | tr -d '{}' | xargs)"; }

cmd_mac() {
  [[ "$BUILD" -eq 0 ]] || with_build_lock build_mac
  report_load
  local offset pid
  offset="$(log_size "$MAC_LOG")"
  pid="$(launch_mac)"
  info "driving the calendar (pid $pid)"
  wait_for_drive "$offset" "$pid"
  print_run "$(log_slice_since "$MAC_LOG" "$offset")"
  quit_mac
}

cmd_mac_profile() {
  require_cmd python3
  [[ "$BUILD" -eq 0 ]] || with_build_lock build_mac
  report_load
  local work offset pid xc
  work="$(mktemp -d "$TMP_ROOT/calendar-perf-apple.XXXXXX")"
  offset="$(log_size "$MAC_LOG")"
  pid="$(launch_mac)"
  info "recording the Time Profiler on pid $pid"
  # Stopped with SIGINT once the drive is done; the time limit only bounds a run that never ends.
  xcrun xctrace record --template 'Time Profiler' --attach "$pid" \
    --time-limit "$((DRIVE_TIMEOUT + 10))s" --output "$work/run.trace" >"$work/record.log" 2>&1 &
  xc=$!
  # A failed drive dies here, leaving the recording to its time limit's end.
  trap 'kill -INT "$xc" 2>/dev/null || true' EXIT
  wait_for_drive "$offset" "$pid"
  trap - EXIT
  kill -INT "$xc" 2>/dev/null || true
  wait "$xc" || true
  [[ -d "$work/run.trace" ]] || die "xctrace wrote no trace; see $work/record.log"
  local slice; slice="$(log_slice_since "$MAC_LOG" "$offset")"
  print_run "$slice"
  quit_mac
  printf '%s\n' "$slice" >"$work/run.log"
  xcrun xctrace export --input "$work/run.trace" --toc | grep -E '<start-date>' >"$work/toc.xml"
  xcrun xctrace export --input "$work/run.trace" \
    --xpath '//trace-toc[1]/run[1]/data[1]/table[@schema="time-profile"]' >"$work/profile.xml"
  echo
  python3 "$DEV_LIB_DIR/xctrace_summary.py" "$work/profile.xml" --toc "$work/toc.xml" --log "$work/run.log"
  echo
  info "trace: $work/run.trace (open it in Instruments for the call tree)"
}

cmd_device() {
  local device dev=("$DEV_LIB_DIR/device.sh")
  device="$(device_udid)" || die "no physical iOS device found: connect one, or set MAILCAL_DEVICE=<udid>"
  require_unlocked "$device"
  if [[ "$BUILD" -eq 1 ]]; then
    with_build_lock "${dev[@]}" build --release
    "${dev[@]}" install --release
  fi
  # The build takes minutes, long enough for the phone to lock itself again.
  require_unlocked "$device"
  "${dev[@]}" run MAILCAL_CALENDAR_TRACE=drive
  info "driving the calendar; reading the log back from the device"
  local tmp lines="" waited=0
  tmp="$(mktemp)"
  sleep 15
  while (( waited < DRIVE_TIMEOUT )); do
    if device_pull "$device" "$APP_LOG_REL" "$tmp"; then
      lines="$(run_lines "$(cat "$tmp")")"
      [[ -n "$lines" ]] && break
    fi
    sleep 5; waited=$((waited + 5))
  done
  rm -f "$tmp"
  [[ -n "$lines" ]] || die "no '$DONE_MARKER' from the device in ${DRIVE_TIMEOUT}s. A phone that
       locks mid-run sends the app to the background, which stops the drive: keep it unlocked"
  printf '%s\n' "$lines"
}

case "$MODE" in
  mac) cmd_mac ;;
  mac-profile) cmd_mac_profile ;;
  device) cmd_device ;;
  ""|-h|--help) usage ;;
  *) die "unknown mode '$MODE' (mac|mac-profile|device)" ;;
esac
