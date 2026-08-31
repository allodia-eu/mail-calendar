#!/usr/bin/env bash
# Measure the calendar grid on a PHYSICAL Android device: does it drop frames, and does it lose
# swipes? The two numbers docs/calendar.md §6 and §7 are written in.
#
#   scripts/dev/calendar-perf.sh frames [--serial S] [--swipes N]   # in-motion frame gaps
#   scripts/dev/calendar-perf.sh flicks [--serial S] [--swipes N]   # weeks turned per flick thrown
#
# WHY THESE TWO, AND NOT THE OBVIOUS ONES
#
#   `gfxinfo`'s "Janky frames %" is the metric everyone reaches for, and for this comparison it is
#   worthless. Two grids do not render the same NUMBER of frames; a good one settles and goes idle,
#   a bad one keeps animating into the pause; so a ratio over two different denominators compares two
#   different questions. It rated the composable grid and the canvas within a point of each other when
#   one was dropping three times as many frames as the other. What the eye sees is the GAP between one
#   frame landing and the next, DURING the motion, which is what `frames` measures.
#
#   `mpdecimate` over a screen recording is the other tempting one, and it cannot resolve this either:
#   `screenrecord` caps at ~60fps and its encoder perturbs the very app it is measuring. Ours scored
#   *worse* than its own previous build on a recording that a hand could feel was three times smoother.
#   Record video to SEE behaviour; measure timing with framestats.
#
# RELEASE BUILDS ONLY. An unminified Compose build is several times slower, so measuring a debug build
# tells you about the debug build. (`clients/android/build-release.sh`. If the installed app is
# debug-signed, re-sign the release APK with the debug key and `install -r`; a signature mismatch
# forces an uninstall, and an uninstall takes the user's accounts with it.)
#
# IT REFUSES TO SWIPE UNLESS THE CALENDAR IS ON SCREEN. A swipe on the mail list is a swipe ACTION,
# and it archives mail. Ask me how I know.
set -euo pipefail

# The installed package name follows the brand (docs/branding.md), so it is read rather than
# written here; a hardcoded one would attach to nothing on an unbranded build and report the app
# as not running.
# shellcheck source=scripts/dev/lib.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
PKG="$ANDROID_PKG"
MODE="${1:?usage: calendar-perf.sh frames|flicks [--serial S] [--swipes N]}"; shift
SERIAL=""; SWIPES=12
while [[ $# -gt 0 ]]; do
  case "$1" in
    --serial) SERIAL="$2"; shift 2 ;;
    --swipes) SWIPES="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
ADB=(adb); [[ -n "$SERIAL" ]] && ADB=(adb -s "$SERIAL")

# The calendar header renders a month and a year; the mail list never does. It is a composable on both
# grids; the canvas draws the grid itself, so a UI dump of it is otherwise empty.
assert_calendar() {
  local dump
  dump=$("${ADB[@]}" exec-out uiautomator dump /dev/tty 2>/dev/null || true)
  grep -qiE "inbox|postvakken" <<<"$dump" &&
    { echo "REFUSING: the MAIL list is on screen: a swipe here archives mail." >&2; exit 1; }
  grep -qE "20[0-9]{2}" <<<"$dump" ||
    { echo "REFUSING: cannot confirm the calendar is on screen." >&2; exit 1; }
}

assert_calendar
read -r W H < <("${ADB[@]}" shell wm size | sed 's/.*: //' | tr 'x' ' ')
MIDY=$((H / 2)); LEFT=$((W / 5)); RIGHT=$((W * 4 / 5))
flick() { "${ADB[@]}" shell input swipe "$1" "$MIDY" "$2" "$MIDY" "${3:-90}"; }

case "$MODE" in
  frames)
    # §7: the bar is not the average frame; both grids hit 120Hz at rest. It is the frames MISSED
    # DURING MOTION. So reset Android's own counters, flick, and read the per-frame completion
    # timestamps straight back: essentially every frame in the window is an in-motion frame.
    "${ADB[@]}" shell setprop log.tag.MailcalCal INFO   # a trace inside the frame measures itself
    OUT=$(mktemp)
    for i in $(seq 1 "$SWIPES"); do
      "${ADB[@]}" shell dumpsys gfxinfo "$PKG" reset >/dev/null
      if (( i % 2 )); then flick "$RIGHT" "$LEFT"; else flick "$LEFT" "$RIGHT"; fi
      sleep 1.0
      "${ADB[@]}" shell dumpsys gfxinfo "$PKG" framestats \
        | grep -A200 "^---PROFILEDATA" | grep -E "^[0-9]" >> "$OUT"
      echo "---" >> "$OUT"
    done
    assert_calendar
    python3 - "$OUT" <<'PY'
import csv, sys
gaps, frames = [], 0
for chunk in open(sys.argv[1]).read().split('---'):
    rows = [r for r in csv.reader(chunk.strip().splitlines()) if len(r) > 13]
    done = sorted(int(r[13]) for r in rows if r[13].isdigit() and int(r[13]) > 0)
    frames += len(done)
    gaps += [(b - a) / 1e6 for a, b in zip(done, done[1:])]
# Over 60ms is the hand pausing, not a frame we lost. Counting that is how you chase idleness. (§7)
m = sorted(g for g in gaps if g < 60)
if not m:
    print("no frames captured"); raise SystemExit(1)
pct = lambda q: m[min(int(len(m) * q), len(m) - 1)]
drop = sum(1 for g in m if g > 12.5)   # >1.5 frames at 120Hz: a frame the eye lost
print(f"  frames delivered in motion : {frames}")
print(f"  median gap                 : {pct(.50):.1f} ms")
print(f"  p90 gap                    : {pct(.90):.1f} ms")
print(f"  p99 gap                    : {pct(.99):.1f} ms")
print(f"  DROPPED (gap > 12.5ms)     : {drop}/{len(m)}  ({100*drop/len(m):.1f}%)")
print()
print("  reference, same phone, same diary: composable grid 20% · canvas 7% ·")
print("  canvas with the week banked on decision 3.6% · Samsung Calendar 2.4%")
PY
    rm -f "$OUT"
    ;;

  flicks)
    # §6: a flick that arrives while the last turn is still SLIDING must still turn a week. It did not,
    # once: the turn was only committed when its animation finished, so a second flick cancelled a week
    # already won. Eight fast flicks turned three weeks. This counts what actually landed.
    #
    # This mode NEVER navigates. It relies on the trace, which is read once at class-load, so the app
    # must already be running with it on and the calendar already open:
    #
    #   adb shell setprop log.tag.MailcalCal DEBUG   # then relaunch, and open the calendar
    #
    # It used to relaunch the app itself; which lands on the MAIL list, where a horizontal swipe is a
    # swipe ACTION. The guard above caught it. Twice bitten.
    "${ADB[@]}" logcat -c
    for i in $(seq 1 "$SWIPES"); do flick "$RIGHT" "$LEFT" 60; done
    sleep 2
    # Nudge the grid so a frame is drawn: the trace flushes on a 1s tick, and a burst that ends with
    # the grid idle would otherwise never flush its tail; the instrument under-reporting its own way
    # into a bug that is not there.
    for i in 1 2 3; do
      "${ADB[@]}" shell input swipe $((W/2)) $((H*3/5)) $((W/2)) $((H*2/5)) 300; sleep 1.2
    done
    LINES=$("${ADB[@]}" logcat -d -s MailcalCal | grep -c "1s:" || true)
    if [[ "$LINES" -eq 0 ]]; then
      echo "  no trace output. Enable it and relaunch:" >&2
      echo "    adb shell setprop log.tag.MailcalCal DEBUG && adb shell am force-stop $PKG" >&2
      exit 1
    fi
    "${ADB[@]}" logcat -d -s MailcalCal \
      | grep -oE "pan_x=[0-9]+|turns=[0-9]+" | paste - - \
      | awk -v n="$SWIPES" '
          {split($1,a,"=");split($2,b,"=");g+=a[2];t+=b[2]}
          END {
            printf "  flicks thrown  : %d\n  gestures seen  : %d\n  WEEKS TURNED   : %d\n", n, g, t
            if (t < n) printf "\n  FAIL: %d flick(s) were swallowed. See docs/calendar.md §6.\n", n-t
            else print  "\n  PASS: every flick turned a week."
          }'
    ;;
  *) echo "unknown mode: $MODE (frames|flicks)" >&2; exit 2 ;;
esac
