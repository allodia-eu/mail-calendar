#!/usr/bin/env bash
# Qualify the Linux calendar renderer in an optimised build, on the real display and GPU, against
# the pinned GNOME runtime. GDK's completed presentation timestamps measure compositor delivery;
# GTK tick times are used only to keep the grid moving and are not reported as frames.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sdk.sh"

[[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]] ||
  die "Linux calendar performance needs a real desktop display (not Xvfb)"
command -v jq >/dev/null || die "Linux calendar performance needs jq"
require_cmd dbus-run-session

OUT="$REPO_ROOT/target/linux-calendar-perf"
RESULT="$OUT/result.json"
mkdir -p "$OUT"
rm -f "$RESULT"

echo "==> building optimized Linux calendar fixture in the GNOME $(sdk_runtime_version) SDK"
sdk_cargo build --release -p mailcal-linux --features dev-harness

echo "==> measuring 600 in-motion frames with at least 125 events in the visible week"
# A private session bus gives this run its own GApplication name. Otherwise an already-running app
# accepts the activation and this instrumented process exits before writing a trace.
dbus-run-session -- "$REPO_ROOT/scripts/dev/sdk.sh" exec \
  --env=MAILCAL_SHOWCASE=1 \
  --env=MAILCAL_SHOWCASE_SCREEN=calendar \
  --env=MAILCAL_CALENDAR_VIEW=week \
  --env=MAILCAL_CALENDAR_PERF_EVENTS=160 \
  --env=MAILCAL_CALENDAR_PERF_RESULT="$RESULT" \
  --env=GSETTINGS_BACKEND=memory \
  "$(sdk_target_dir)/release/mailcal-linux" \
  >"$OUT/stdout.log" 2>"$OUT/stderr.log"

[[ -s "$RESULT" ]] || die "the optimized calendar wrote no presentation trace (see $OUT)"
jq -e '
  .optimized == true and
  .semantic_nodes == false and
  .events_in_week >= 125 and
  .presentation_samples >= 300 and
  .refresh_interval_us > 0 and
  .measured_gaps >= 299
' "$RESULT" >/dev/null || die "the presentation trace is incomplete (see $RESULT)"

refresh="$(jq -r '.refresh_interval_us' "$RESULT")"
p90="$(jq -r '.p90_gap_us' "$RESULT")"
dropped="$(jq -r '.dropped_frames' "$RESULT")"
gaps="$(jq -r '.measured_gaps' "$RESULT")"
drop_percent="$(jq -nr --argjson dropped "$dropped" --argjson gaps "$gaps" \
  '$dropped * 100 / $gaps')"

jq '{
  optimized,
  semantic_nodes,
  gtk,
  events_in_week,
  presentation_samples,
  refresh_interval_ms: (.refresh_interval_us / 1000),
  median_gap_ms: (.median_gap_us / 1000),
  p90_gap_ms: (.p90_gap_us / 1000),
  p99_gap_ms: (.p99_gap_us / 1000),
  dropped_frames,
  measured_gaps
}' "$RESULT"

# A missed frame is >1.5 refresh intervals, matching the calendar contract. The release bar keeps
# the p90 within one missed-frame boundary and no more than 5% of the motion over it.
(( p90 * 2 <= refresh * 3 )) ||
  die "p90 exceeds 1.5 frame intervals (see $RESULT)"
jq -ne --argjson dropped "$dropped" --argjson gaps "$gaps" \
  '$dropped * 100 <= $gaps * 5' >/dev/null ||
  die "${drop_percent}% of in-motion frames missed the budget (limit 5%; see $RESULT)"

echo "==> PASS: ${drop_percent}% dropped; full presentation trace: $RESULT"
