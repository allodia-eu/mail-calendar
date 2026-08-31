#!/usr/bin/env bash
# The Android half of scripts/dev/showcase.sh: resolving (and booting) the device a screenshot set
# wants, normalising what the capture will show; the status bar, notifications, the display
# rotation; driving the app into each screen, and putting the device back as it was.
#
# Sourced by showcase.sh, not run directly. It is a separate file because Android carries by far the
# most device wrangling of any platform (the others hand it to simctl or to a PowerShell twin) and
# together they overflow the repo's 500-line file limit. It reads showcase.sh's $TARGET / $SERIAL /
# $AVD and the target_* tables, and defines the android_* helpers showcase.sh calls.
#
# Google Play has a phone slot and *two* tablet slots (7-inch and 10-inch), so this is where the
# `android-tablet-7` / `android-tablet-10` targets pick their AVD, boot it if it is not already
# running, and pin it to the portrait size that slot expects.

ADB=()
STAYON_WAS=""
DEMO_ALLOWED_WAS=""
AUTOROTATE_WAS=""
ROTATION_WAS=""
ORIENTATION_PINNED=""
EMULATOR_STARTED="" # the serial of an emulator *this run* booted, and must therefore shut down

# The console port a showcase run boots its emulator on. Deliberately not the default 5554: a
# tablet run must neither collide with, nor be mistaken for, whatever the developer already has
# running there.
SHOWCASE_EMULATOR_PORT=5580

# Point $SERIAL at the device this target wants: every Android target names an AVD, which is booted
# here if it isn't already running. An explicit --serial always wins; it is the escape hatch for a
# device these defaults don't describe, and it skips the boot entirely.
#
# The phone target's AVD has no built-in default and refuses rather than falling back to the
# attached device. That fallback is what this guard replaces: a developer's own machine usually has
# their real phone plugged in, holding their real mail and diary, and a capture run installs and
# drives a build on whatever it resolved. Being told to name a device costs one line in a
# git-ignored file; the other outcome is discovered after the fact.
android_resolve_device() {
  [[ -z "$SERIAL" ]] || return 0
  local avd
  avd="${AVD:-$(target_avd "$TARGET")}"
  [[ -n "$avd" ]] || die "no emulator is configured for the '$TARGET' target.
  Name one in scripts/dev/devices.local.sh (git-ignored, per-machine):
      cp scripts/dev/devices.local.sh.example scripts/dev/devices.local.sh
      MAILCAL_AVD_PHONE=<one of the names below>
  This machine's AVDs: $(list_avds | tr '\n' ' ')
  Or pass --avd <name> for one run, or --serial <serial> to photograph an already-attached device
  (which on your own machine may be your real phone)."
  avd_exists "$avd" || die "no AVD named '$avd'. Create it in Android Studio's Device Manager from
  the stock '$(target_device_profile "$TARGET")' device profile, or from the command line:
  avdmanager create avd -n '$avd' -d $(target_device_profile "$TARGET") -k 'system-images;android-35;google_apis_playstore_tablet;arm64-v8a'
  (available AVDs: $(list_avds | tr '\n' ' '))"
  if SERIAL="$(emulator_serial_for_avd "$avd")"; then
    info "reusing the running '$avd' emulator ($SERIAL)"
  else
    info "booting the '$avd' emulator on port $SHOWCASE_EMULATOR_PORT (a cold boot takes a minute)"
    SERIAL="$(emulator_boot "$avd" "$SHOWCASE_EMULATOR_PORT")"
    EMULATOR_STARTED="$SERIAL"
  fi
}

# Both stock tablet AVDs are landscape-native, and Play's recommended tablet sizes (1200x1920 for
# 7-inch, 1600x2560 for 10-inch) are portrait; which is also the shape the rest of the committed
# set is in, and the one a single-pane layout reads best in. So rotate 90° from the device's natural
# orientation. `user_rotation` is ignored while auto-rotate is on, so both are set; both are also
# restored afterwards, because the emulator's userdata survives even a cold boot.
#
# On a portrait-native device this would rotate the wrong way; `require_capture_size` is what turns
# that into a loud failure instead of a sideways store asset.
android_orientation_pin() {
  [[ -n "$(target_size "$TARGET")" ]] || return 0
  ORIENTATION_PINNED=1
  AUTOROTATE_WAS="$("${ADB[@]}" shell settings get system accelerometer_rotation 2>/dev/null | tr -d '[:space:]')"
  ROTATION_WAS="$("${ADB[@]}" shell settings get system user_rotation 2>/dev/null | tr -d '[:space:]')"
  android_orientation_apply
}

# Re-assert the pin. Like the status bar, the rotation does not reliably stick when it is set once
# during the device's startup; a rotation written while the display is still asleep is undone as
# the device wakes and WindowManager settles, and the run then shoots a landscape frame for a
# portrait slot. Recording the previous values stays in `android_orientation_pin` above, so
# re-applying can never overwrite them with the value we ourselves pinned.
android_orientation_apply() {
  [[ "$ORIENTATION_PINNED" == "1" ]] || return 0
  "${ADB[@]}" shell settings put system accelerometer_rotation 0 >/dev/null 2>&1 || true
  "${ADB[@]}" shell settings put system user_rotation 1 >/dev/null 2>&1 || true
}

# Undo the pin; but only if we set it, so a plain `android` run never writes a rotation key it
# never read. Unlike every other setting this script touches, `delete` is not enough here:
# WindowManager re-persists both keys as the display settles back, so an unset pair comes back as
# 0/0; natural orientation (which is what "unset" looks like anyway) but auto-rotate *off*, which
# would leave the device locked to one orientation. So an unset auto-rotate is put back to
# Android's default of 1 rather than deleted, and it goes last, once the rotation has been released.
android_orientation_restore() {
  [[ "$ORIENTATION_PINNED" == "1" ]] || return 0
  android_restore_setting system user_rotation "$ROTATION_WAS"
  local autorotate="$AUTOROTATE_WAS"
  [[ -n "$autorotate" && "$autorotate" != "null" ]] || autorotate=1
  "${ADB[@]}" shell settings put system accelerometer_rotation "$autorotate" >/dev/null 2>&1 || true
}

android_prepare() {
  android_resolve_device
  ADB=("$(adb_bin)")
  if [[ -n "$SERIAL" ]]; then
    ADB+=(-s "$SERIAL")
    # Every `adb` honours ANDROID_SERIAL, including the one inside clients/android/build-and-run.sh
    #; which otherwise fails with "more than one device/emulator" when a phone and a tablet are
    # both attached, which a tablet screenshot run typically leaves them.
    export ANDROID_SERIAL="$SERIAL"
  fi
  "${ADB[@]}" wait-for-device
  # Hold the display on for the whole run. A capture pass is minutes long, so a physical phone's
  # screen timeout fires part-way through and every remaining `screencap` returns a solid black
  # frame; an emulator never sleeps, which is exactly why this hid on the tablet. `stayon true`
  # is honoured while charging, which a USB-attached device is. Remember the device's own setting
  # (emulators ship with it already on) so `android_cleanup` restores that, not a guessed default.
  STAYON_WAS="$("${ADB[@]}" shell settings get global stay_on_while_plugged_in 2>/dev/null | tr -d '[:space:]')"
  "${ADB[@]}" shell svc power stayon true >/dev/null 2>&1 || true
  android_wake
  android_orientation_pin
  android_demo_allow
  android_notifications_clear
  # Enter demo mode here, so the first *capture* is not also the first `enter`. SystemUI lays the
  # demo status bar out without its normal clock start-padding on the very first entry and corrects
  # it a moment later; which put a subtly different status bar on exactly one screenshot per run,
  # and always on `list`, the first screen in the loop and the one most likely to lead a store page.
  # Every later capture re-pins from an already-demo bar and never showed it.
  android_status_bar_pin
  sleep 3
  android_systemui_settle
}

# Give SystemUI time to finish laying out its status bar after a cold boot, before the *first*
# capture goes through the shutter.
#
# `sys.boot_completed` (which emulator_boot waits for) is not "the status bar has settled": for tens
# of seconds after it, SystemUI is still applying the display's cutout and rounded-corner insets, and
# the clock is drawn hard against the left edge instead of at its normal start padding. Only the
# first screen of a run is exposed, so this shows up as *one* screenshot in the set whose status bar
# is subtly different from the other four; and `list`, being first in the loop, is the one you would
# actually put on a store page.
#
# This is a settle with margin, not a proof, and it is deliberately blunt: no shell command reports
# what SystemUI is rendering (the same limitation `android_demo_allow` documents), so there is
# nothing to poll. It costs nothing on the common path; a run against an already-running emulator
# or a physical device skips it entirely.
SYSTEMUI_SETTLE_SECONDS=45
android_systemui_settle() {
  [[ -n "$EMULATOR_STARTED" ]] || return 0
  info "letting SystemUI settle for ${SYSTEMUI_SETTLE_SECONDS}s after the cold boot"
  sleep "$SYSTEMUI_SETTLE_SECONDS"
}

# One SystemUI demo-mode command. `am broadcast` reads stdin, which would eat a caller's loop input.
android_demo() { # <command> [extra `am` args...]
  local command="$1"
  shift
  "${ADB[@]}" shell am broadcast -a com.android.systemui.demo \
    -e command "$command" "$@" </dev/null >/dev/null 2>&1 || true
}

# The Android counterpart of the simulators' `simctl status_bar override`: SystemUI's demo mode
# freezes the status bar, so every capture carries the same clock, battery and radios whatever time
# the run happens at, and on whatever hardware.
#
# Every element is set explicitly, none left to inherit. SystemUI keeps its demo state across
# `exit`/`enter`, so an element this function doesn't name is whatever the *last* run left behind,
# precisely the nondeterminism the pin exists to remove. (Left alone, wifi went on rendering the
# "connected, no internet" glyph that a stray earlier command had set.)
#
# Wifi is shown at full strength and cellular hidden; true of the tablet, which has no modem, and
# of a phone on wifi. Demo mode would just as happily paint four bars of cellular onto that tablet:
# freezing the clock is normalisation, inventing a radio the device hasn't got is a lie in a store
# asset.
#
# Demo mode is opt-in per device and SystemUI *silently drops* the broadcasts while it is off; so
# check the flag took, rather than discovering a live status bar in the screenshots. That read-back
# is as far as verification goes: no shell command reports what the bar is actually rendering
# (`dumpsys statusbar` shows an empty `mIcons=`), so unlike `require_showcase_launch` this cannot be
# a hard guarantee, only a loud failure to arm.
android_demo_allow() {
  DEMO_ALLOWED_WAS="$("${ADB[@]}" shell settings get global sysui_demo_allowed 2>/dev/null | tr -d '[:space:]')"
  "${ADB[@]}" shell settings put global sysui_demo_allowed 1 >/dev/null 2>&1 || true
  local allowed
  allowed="$("${ADB[@]}" shell settings get global sysui_demo_allowed 2>/dev/null | tr -d '[:space:]')"
  [[ "$allowed" == "1" ]] ||
    die "could not enable SystemUI demo mode (sysui_demo_allowed=${allowed:-unset}): the status bar would photograph live icons"
}

# Pin the bar. This runs before *every* capture, not once per run, and it tears demo mode down with
# `exit` before re-entering; both for the same reason.
#
# `sys.boot_completed` is not "SystemUI is ready": pinning inside that startup window leaves the
# demo wifi icon drawn *next to* the live one SystemUI was still inflating, and the tablet
# screenshots then show two wifi glyphs. It reproduces on a cold boot and disappears on a settled
# device; CPU load widens the window rather than causing it; and `exit` before `enter` is what
# collapses the pair back to one. Re-pinning per capture also means the last capture is pinned as
# firmly as the first, whatever the device did in between.
android_status_bar_pin() {
  android_demo exit
  android_demo enter
  android_demo clock -e hhmm 0941 # 09:41, as on the simulators
  android_demo battery -e level 100 -e plugged false
  # `level` is shared between the `wifi` and `mobile` keys, so a single broadcast cannot give them
  # different ones; and `fully true` is what suppresses wifi's "no internet" exclamation mark.
  android_demo network -e wifi show -e level 4 -e fully true
  android_demo network -e mobile hide
}

# Notification icons are the one part of the status bar demo mode will not touch: its
# `notifications -e visible false` command is a no-op on current Android (verified on API 36), and
# the icons belong to *other* apps regardless. The phone emulator still held `new_mail`
# notifications posted by `nl.allodia.mailcaldemo`; this app's own retired package id; so an
# envelope from a build that no longer exists was photographed into every phone screenshot, next to
# a Play Store icon and a Safety Centre shield.
#
# There is no supported way to *hide* them (`icon_blacklist` covers system icons only), so they have
# to be cleared. `snooze` is the only lever the shell has, and despite the name it is a one-way door
# from here: `unsnooze` fails with a permission denial as the shell uid, and the snooze does not
# reliably repost when it expires. That is acceptable on an emulator, whose notifications are
# throwaway; and unacceptable on the developer's physical phone, so it is not done there.
SNOOZE_MS=3600000
android_is_emulator() {
  local qemu
  qemu="$("${ADB[@]}" shell getprop ro.boot.qemu 2>/dev/null | tr -d '[:space:]')"
  [[ "$qemu" == "1" ]]
}

# Like the status bar pin, this runs before *every* capture rather than once per run; clearing at
# prepare time is too early to work. `android_prepare` runs before `build_once`, and the build
# installs and launches the app, which posts a `new_mail` notification of its own; every capture
# after that photographed an envelope sitting next to the pinned 09:41 clock. Clearing once caught
# only what was already there, which is the state least likely to matter.
NOTIFICATIONS_SKIP_LOGGED=""

android_notifications_clear() {
  if ! android_is_emulator; then
    if [[ -z "$NOTIFICATIONS_SKIP_LOGGED" ]]; then
      info "physical device: leaving its notifications alone: stray icons may appear in the status bar"
      NOTIFICATIONS_SKIP_LOGGED=1
    fi
    return 0
  fi
  local key
  # `adb shell` reads stdin, and would swallow the rest of the key list on the first iteration.
  while read -r key; do
    [[ -n "$key" ]] || continue
    # Every key contains `|`, which the *device's* shell would read as a pipe unless quoted there.
    "${ADB[@]}" shell cmd notification snooze --for "$SNOOZE_MS" "'$key'" </dev/null >/dev/null 2>&1 || true
  done < <("${ADB[@]}" shell cmd notification list </dev/null 2>/dev/null | tr -d '\r' | sed '/^$/d')
}

# The composer focuses its editor, which raises the on-screen keyboard over the bottom half of the
# frame; burying the quoted message and the formatting toolbar, which is most of what the `reply`
# screenshot exists to show. Whether the shutter beats it is pure timing: the committed English set
# was shot a second before the keyboard rose, a slower run caught it a second after, and the two
# locales stopped matching. So dismiss it explicitly rather than tuning the delay.
#
# `dumpsys input_method` reports `mInputShown`, which makes this checkable instead of hopeful: BACK
# is sent **only** when the keyboard is actually up (with the IME hidden, the same key would go to
# the app and navigate out of the composer), and the state is re-read afterwards to confirm it
# closed. Disabling the IMEs outright does not work; Android re-enables a default as soon as the
# last one is gone, which is why an earlier attempt at that changed nothing.
android_keyboard_shown() {
  local shown
  shown="$("${ADB[@]}" shell dumpsys input_method 2>/dev/null |
    sed -nE 's/.*mInputShown=([a-z]+).*/\1/p' | head -1 | tr -d '[:space:]')"
  [[ "$shown" == "true" ]]
}

android_keyboard_hide() {
  local attempt
  for attempt in 1 2 3; do
    android_keyboard_shown || return 0
    "${ADB[@]}" shell input keyevent KEYCODE_BACK >/dev/null 2>&1 || true
    sleep 1
  done
  android_keyboard_shown &&
    info "warning: the on-screen keyboard would not close: it will be in this capture"
  return 0
}

# Wake the display and get past the lock screen. Cheap, so it runs before every capture rather
# than once; `stayon` needs the device charging, and nothing guarantees that.
android_wake() {
  "${ADB[@]}" shell input keyevent KEYCODE_WAKEUP >/dev/null 2>&1 || true
  "${ADB[@]}" shell wm dismiss-keyguard >/dev/null 2>&1 || true
}

# `screencap` reports success on a sleeping display and hands back an all-black PNG, so the only
# evidence is the image itself; which a size check will not catch (a black 1080x2316 frame is a
# valid 15 kB PNG). Assert the display is actually on instead of discovering it in the screenshot.
android_require_awake() {
  local state
  state="$("${ADB[@]}" shell dumpsys power 2>/dev/null |
    sed -nE 's/.*mWakefulness=([A-Za-z]+).*/\1/p' | head -1)"
  [[ "$state" == "Awake" ]] ||
    die "the Android display is '${state:-unknown}', not Awake: screencap would return a black frame"
}

# A system dialog sits *on top of* the app and dims everything behind it, so the frame under it is
# the right app, in the right language, on the right screen; and unusable as a store asset. Every
# guard we had waved it through: `require_showcase_launch` reads the app's own log (the app is fine;
# it is SystemUI that died), and `require_real_capture` weighs the PNG (a scrim plus a white dialog
# weighs the same as the screen it covers; the contaminated frames matched their clean peers to
# within 0.4% of mean luminance, because the dim and the dialog cancel out).
#
# This cost a full 42-frame Android phone set: SystemUI ANR'd under load early in a run, the "System
# UI isn't responding" dialog stayed up for the *whole* run, and all 42 frames were shot through it.
# The set was only caught because the composed feature graphic put one of them in front of a human.
#
# So ask the window manager instead of the pixels. An ANR/crash dialog is a window whose title names
# it, and it is gone the moment the dialog is dismissed; a check that fails exactly when the frame
# would be wrong, and cannot fail when it wouldn't.
android_require_no_system_dialog() {
  local windows
  # `|| true`: no dialog means grep matches nothing and exits 1, which under `set -e` would abort
  # the run on the *healthy* path; a guard that fails when everything is fine is worse than none.
  windows="$("${ADB[@]}" shell dumpsys window windows 2>/dev/null |
    grep -iE "Window\{.*(Application Not Responding|Application Error|Crash)" | head -3 || true)"
  [[ -z "$windows" ]] ||
    die "a system dialog is covering the screen: refusing to take a screenshot:
$windows
  Everything behind it is dimmed, so the capture would be unusable as a store asset.
  This is usually SystemUI ANR-ing under load: reboot the emulator and re-run this target."
}

# Android has no env vars, so the flags ride in as intent extras (as MAILCAL_DEV_ACCOUNT does), and
# the chrome's language is the per-app locale; which only the OS can set, so `cmd locale` sets it
# before launch. MainActivity is singleTask, so a plain `am start` on a running app would deliver
# onNewIntent and never re-read the extras: force-stop first.
android_capture() { # <locale> <screen> <out>
  local offset
  offset="$(client_log_size)"
  "${ADB[@]}" shell cmd locale set-app-locales "$ANDROID_PKG" --locales "$1" >/dev/null
  "${ADB[@]}" shell am force-stop "$ANDROID_PKG"
  sleep 1
  android_wake
  android_orientation_apply
  android_status_bar_pin
  # Android has no env vars, so the appearance showcase.sh exported rides in as an extra like the
  # rest. Still conditional: `android_capture` is also reachable from a hand-run of this file's
  # helpers, and an empty extra would be parsed as a typo'd theme and ignored rather than absent.
  local appearance=()
  [[ -n "${MAILCAL_APPEARANCE:-}" ]] && appearance=(-e MAILCAL_APPEARANCE "$MAILCAL_APPEARANCE")
  "${ADB[@]}" shell am start -n "$ANDROID_ACTIVITY" \
    -e MAILCAL_SHOWCASE "$1" -e MAILCAL_SHOWCASE_SCREEN "$2" \
    ${appearance[@]+"${appearance[@]}"} >/dev/null
  sleep "$(settle_for "$2")"
  android_require_awake
  require_showcase_launch "$1" "$offset"
  # Normalise the status bar a second time, now that everything that perturbs it has happened.
  # Pinning only *before* the launch leaves two ways for a live icon to reach the frame, and this
  # run hit both: the app posts its new-mail notification during the settle above (an envelope next
  # to the pinned clock), and on a cold boot SystemUI is still inflating its own icons when the
  # first pin lands, so the demo wifi glyph is drawn beside the live one and the frame shows *two*.
  # `exit` before `enter` is what collapses that pair, so the re-pin has to be a full one.
  # A build between boot and first capture used to hide this by accident; `--no-build` removes it.
  android_status_bar_pin
  android_notifications_clear
  android_keyboard_hide
  sleep 3 # let SystemUI re-render the demo bar and animate the notification icon away
  # Last thing before the shutter, so a dialog that appeared *during* the settle is still caught.
  android_require_no_system_dialog
  "${ADB[@]}" exec-out screencap -p >"$3"
}

# Leave the device as we found it: the per-app locale is a persisted user setting, and a stale one
# would silently change the language of the developer's next normal run; so is `stayon`, which
# would otherwise keep the developer's screen awake forever; so is the display rotation, which
# survives even the emulator's cold boot (userdata does, the snapshot is what `-no-snapshot` skips);
# and demo mode outlives this process entirely, freezing the developer's own status bar at 09:41
# until SystemUI restarts. An emulator this run booted is shut down last, after those restores.
android_cleanup() {
  [[ ${#ADB[@]} -gt 0 ]] || return 0
  "${ADB[@]}" shell cmd locale set-app-locales "$ANDROID_PKG" --locales "" >/dev/null 2>&1 || true
  android_demo exit
  android_orientation_restore
  android_restore_setting global sysui_demo_allowed "$DEMO_ALLOWED_WAS"
  if [[ -n "$STAYON_WAS" && "$STAYON_WAS" != "null" ]]; then
    "${ADB[@]}" shell settings put global stay_on_while_plugged_in "$STAYON_WAS" >/dev/null 2>&1 || true
  fi
  if [[ -n "$EMULATOR_STARTED" ]]; then
    info "shutting down the '$EMULATOR_STARTED' emulator this run booted"
    emulator_shutdown "$EMULATOR_STARTED"
  fi
}

# Put a `settings` key back to the value it had, where "" / "null" mean it was never set; which
# most of them weren't, so restoring that state means deleting the key, not writing a 0 we invented.
android_restore_setting() { # <namespace> <key> <previous>
  if [[ -z "$3" || "$3" == "null" ]]; then
    "${ADB[@]}" shell settings delete "$1" "$2" >/dev/null 2>&1 || true
  else
    "${ADB[@]}" shell settings put "$1" "$2" "$3" >/dev/null 2>&1 || true
  fi
}
