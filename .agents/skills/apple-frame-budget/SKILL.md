---
name: apple-frame-budget
description: Measure the Apple calendar grid's frame budget on the Mac and on a PHYSICAL iPhone/iPad, from a Release build on a release core, with the in-app frame trace's scripted motion; and find where the main thread's time goes with the Time Profiler on the Mac. Wraps scripts/dev/calendar-perf-apple.sh (mac, mac-profile, device). Use when the calendar "feels laggy" on macOS or iOS, when comparing two builds' smoothness, or before claiming an Apple performance win. Covers the traps that measure the wrong build or nothing at all.
---

# apple-frame-budget: the Apple calendar's frames, in numbers

The instrument is the app's own frame trace (`CalendarFrameTrace.swift`): a display link times
every frame while the grid moves and writes one line per burst of motion, the gaps at p50, p90,
p99 and worst, the frames that missed their deadline, what moved the grid and how many weeks were
built. `MAILCAL_CALENDAR_TRACE=drive` opens the calendar and moves it itself: hours, then days, then
a zoom on both axes, identically on every run. The rule and the numbers it is held to are
[`docs/calendar.md`](../../../docs/calendar.md) §7 and §9.

```sh
scripts/dev/calendar-perf-apple.sh mac          # Release Mac app, one line per axis
scripts/dev/calendar-perf-apple.sh mac-profile  # the same run under the Time Profiler
scripts/dev/calendar-perf-apple.sh device       # the connected iPhone/iPad (unlocked)
```

Each builds first; `--no-build` measures what this checkout last built. Read **dropped %** and
**p99** per axis, never the median: a good grid and a bad one both sit at the refresh interval for
half their frames.

## The traps

- **A Debug core measures the wrong build.** A debug Rust core, or a Debug Swift build, is slower
  than what ships, so a number taken from either describes neither the app nor the change. The
  script always builds the core with `build-core.sh --release --slices device,macos`, so one core
  serves both the Mac app and the phone; `device.sh build --release` does the same. A simulator
  build afterwards rebuilds the core for its own slice.
- **A Release build opens on the mailbox**, against the developer's own accounts, never the harness
  (`MAILCAL_DEV_ACCOUNT` is a Debug-only switch). Drive mode opens the calendar itself, which is why
  the scripted run needs no hand. The plain trace (`MAILCAL_CALENDAR_TRACE=1`) opens nothing: go to
  the calendar yourself.
- **On the Mac, one copy of the app at a time.** The Release build and an installed copy write one
  store, so the script refuses while another copy runs, and quits its own after each run.
- **The phone must be unlocked, and stay so.** iOS refuses a launch on a locked phone, and a phone
  that locks mid-run sends the app to the background, which stops the drive. The script checks the
  lock before the build and again before the launch; the build takes long enough for auto-lock to
  catch you in between.
- **Instruments' command line cannot reach an app on the phone.** Attaching by name, by pid and by
  launch all fail, and a whole-device recording warms the phone and collects gigabytes that
  `xctrace export` cannot then read. On the phone the trace's lines are the instrument; find the
  cost on the Mac with `mac-profile`, where `xctrace record --attach` works, then confirm the fix
  on the phone with `device`.
- **A trace stores the environment of the process that recorded it**, tokens included. The script
  records with an empty environment; a recording made by hand from a shell does not, so do not hand
  one around.
- **Scripted motion compares builds; hand motion judges feel.** The drive moves the grid's position
  directly, so it costs exactly what drawing costs and nothing of the gesture that would have asked
  for it. Before and after a change, run the same verb on the same machine and compare. Whether it
  feels right is a hand on the trackpad or the glass with `MAILCAL_CALENDAR_TRACE=1`
  (`device.sh run MAILCAL_CALENDAR_TRACE=1`, then `device.sh logs --grep 'calendar frames'`), and a
  person's verdict is data.
- **Other work on the machine moves the numbers.** With a load average near 10 (another session's
  tests, a virtual machine), the zoom on a MacBook Air dropped 3.4% to 5.7% across runs of one
  build, against 0.4% quiet. The script prints the load before each run: compare builds on a quiet
  machine, two or three runs each, from the same week (the drive starts from today).
- **The Time Profiler costs frames itself.** The same build dropped 13.9% of its zoom frames under
  `mac-profile`. Read where the time goes from `mac-profile` and how many frames drop from `mac`.

## Reading `mac-profile`

It prints the trace's lines, then the main thread's busy milliseconds for each second of the
recording, then per axis the share of the four seconds the main thread was busy, the innermost
frames it was busy in, and the app's own frames on its stacks. A burst whose main thread is busy
for most of its four seconds has no room left for a frame's work. The summariser is
`scripts/dev/xctrace_summary.py`; the `.trace` it prints the path of opens in Instruments for the
full call tree.

## Related

- **`device-interaction`**: the same question on a physical Android device
  (`scripts/dev/calendar-perf.sh`).
- **`ios-device-bgsync`**: `scripts/dev/device.sh`, which `device` drives (signing team, Developer
  Mode, terminate before launch).
