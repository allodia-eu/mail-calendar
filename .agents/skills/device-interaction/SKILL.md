---
name: device-interaction
description: Judge and measure interaction quality (scroll, swipe/paging, pinch-zoom, fling) on a PHYSICAL Android device against the developer's real data, where an emulator and a synthetic swipe cannot tell you the truth. Use when tuning or debugging gesture handling or frame budget, when a user reports "it feels laggy / it skipped my swipe / it stutters", or before claiming any performance win. Covers the data-loss traps (signing, swipe-actions on real mail) and the two standard instruments that return confidently wrong answers.
---

# device-interaction: measuring what a hand can feel

An emulator cannot tell you whether a gesture feels right, and **a synthetic swipe cannot reproduce
the bugs that matter**, because every scripted gesture politely waits for the last animation to
finish. The interesting failures live exactly where a real hand goes: a finger arriving *while the
grid is still moving*.

This skill is the loop that found, on a real phone in one session, a swipe that was silently
swallowed, a pinch that cost 3.4× what it should, and two measurements that were confidently wrong.
Everything here was paid for.

---

## 0. Before you touch the device: the data-loss checks

The developer's phone has **their real mail and their real diary** on it. Two ways to destroy that,
both easy:

### A signature mismatch means an uninstall, and an uninstall takes the accounts with it

```sh
adb shell dumpsys package <pkg> | grep -E "pkgFlags|versionName"   # DEBUGGABLE = a debug build
adb pull "$(adb shell pm path <pkg> | sed 's/package://')" /tmp/installed.apk
apksigner verify --print-certs /tmp/installed.apk | grep "SHA-256"  # what is ON the phone
```

Build the **release** APK, then sign it with **whatever key the installed app already carries**
(often the Android debug key, even for a release build), and `install -r` updates in place:

```sh
apksigner sign --ks ~/.android/debug.keystore --ks-pass pass:android \
  --ks-key-alias androiddebugkey --key-pass pass:android app-release.apk
adb install -r app-release.apk          # fails SAFELY on mismatch; it never uninstalls
```

`INSTALL_FAILED_UPDATE_INCOMPATIBLE` is the good outcome: nothing was touched. **Never "solve" it
with `adb uninstall`.** Ask the user first, every time.

### A swipe on the mail list is a swipe ACTION

Horizontal `input swipe` on the wrong screen **archives or deletes real mail**. This is not
hypothetical; it happened. So:

- **Assert what is on screen before every injection**, and refuse otherwise:

  ```sh
  adb exec-out uiautomator dump /dev/tty | grep -qE "20[0-9]{2}" || exit 1   # the calendar header
  adb exec-out uiautomator dump /dev/tty | grep -qi "inbox" && exit 1        # NOT the mail list
  ```

- **Stop the screen locking mid-run**: a lock, then an unlock, silently drops you back on the
  default screen and the rest of your script lands on mail.

  ```sh
  adb shell svc power stayon usb
  ```

- Note a canvas-drawn UI has **no text nodes** to dump. Assert on chrome that is still a real
  composable (a header title), or on the accessibility overlay.

### Never log content

Tracing runs against a real diary. Log **counts, durations, ids**, never a title, a time, an
attendee, a subject or an address. `docs/logging.md`'s never-log-content rule does not bend because a
title would have been convenient.

---

## 1. Release builds only

An unminified Compose build is **several times slower**. Measuring a debug build tells you about the
debug build: it is how the old grid came to be judged against a competitor's release build while
running as debug. `clients/android/build-release.sh`, then re-sign as above.

---

## 2. The two obvious instruments both lie

**`gfxinfo`'s "Janky frames %"** rated two grids within one point of each other while one was dropping
**three times** as many frames as the other. A ratio needs a denominator, and two builds do not render
the same number of frames: a good one settles and goes idle, a bad one keeps animating into the
pause. It is comparing two different questions.

**`mpdecimate` over a `screenrecord`** scored the *fixed* build **worse** than the broken one, on a
recording a hand could feel was three times smoother. `screenrecord` caps at ~60fps and its encoder
perturbs the very app it is measuring.

> **Record video to see *behaviour*. Measure timing with `framestats`.**

### What to measure instead: the gap between frames, during motion

`dumpsys gfxinfo <pkg> framestats` gives per-frame completion timestamps. Diff them, drop gaps over
60ms (that is the hand pausing; counting idleness as jank is how you chase ghosts), and report the
distribution:

```sh
scripts/dev/calendar-perf.sh frames    # gaps between frames, in motion
scripts/dev/calendar-perf.sh flicks    # weeks turned per flick thrown: must be 1:1
```

A gap over ~12.5ms at 120Hz is a frame the eye lost. Report **p90 / p99 / % dropped**, not a median:
both a good and a bad grid sit at the refresh rate for half their frames, which is why a median hides
everything.

---

## 3. Read the gesture trace, not just the pixels

`adb shell setprop log.tag.MailcalCal DEBUG`, then relaunch. A log tag rather than a debug flag, so it
works in the only build worth measuring, and costs one cached boolean when off.

The line that matters is **what the gesture owner decided each finger was** (`pan_x` / `pan_y` /
`zoom` / `tap`). That is how you catch a *real hand's* pinch being misread as a pan, something no
script will ever show you, because `adb` cannot inject two fingers.

**Cost going up while work goes down is a clue, not a performance problem.** A pinch frame costing
3.4× a swipe frame *while drawing half the blocks* was not "the pinch is heavy": it was the shaper's
text cache missing on every frame, because the pinch moves the width the cache is keyed on.

**And check the instrument itself.** The trace reported six of eight gestures, and the two it
swallowed were its own: it only flushed on the frame path, so a burst ending with the app idle never
flushed its tail. An instrument that under-reports invents a bug that is not there.

---

## 4. The bug you cannot reproduce with a script

**A gesture arriving while the previous gesture's animation is still running.** Every synthetic swipe
waits for the settle, so every test passes and the phone still eats swipes.

Reproduce it in a JVM test by **taking the clock away from Compose**:

```kotlin
compose.mainClock.autoAdvance = false
repeat(8) {
    flickLeft()
    compose.mainClock.advanceTimeBy(16)   // ONE frame: the last turn is still mid-slide
}
compose.mainClock.autoAdvance = true
compose.waitForIdle()
assertEquals(8, state.week)               // eight flicks, eight weeks
```

That turns a gesture-versus-animation race into a deterministic, millisecond-exact test that gates
every PR (`clients/android/app/src/test/.../CalendarFlickTest.kt`). Before the fix it failed exactly
as the phone did: twenty flicks, one week.

> **If you are tempted to test a gesture by letting it settle first, you are testing the case that
> already worked.**

---

## 5. Ask the human for the two things you cannot do

`adb` can inject a swipe. It **cannot** inject two fingers, and it cannot feel anything. So:

- **Pinch / zoom must be tried by hand.** Ask.
- **Ask for a screen recording** (`adb shell screenrecord /sdcard/x.mp4`), to *watch behaviour*, not
  to measure timing (§2).
- Their verdict is data. "It skips my swipes" was precise, correct, and reproducible; it just needed
  the right instrument to see.

---

## Related

- [`docs/calendar.md`](../../../docs/calendar.md) §6 (one gesture, one owner), §7 (frame budget),
  §9 (how this is kept honest), §10 (the bar a new platform must clear).
- **`debug-app`** for the emulator/simulator loop and the `MAILCAL_*` launch hooks.
- **`mail-harness`** for the seeded server: its **living week** re-anchors calendar fixtures on the
  current Monday at every seed, so the grid opens on a week with something in it.
