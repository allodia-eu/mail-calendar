#!/usr/bin/env bash
# Captures the store/marketing screenshot set from the seeded in-memory showcase dataset; two
# fictional accounts, no real mail, no network; so a listing's screenshots can be regenerated on
# demand and can never leak personal mail. The app is relaunched once per (locale, screen,
# appearance) with the MAILCAL_SHOWCASE launch flags, so nothing is driven by fragile taps on
# coordinates.
#
# Usage:
#   scripts/dev/showcase.sh <target> [options]
#
#   target                   macos | iphone | ipad | android | android-tablet-7 |
#                            android-tablet-10 | windows | linux
#   --locale <code>|all      Language of chrome *and* sample mail: any locale the catalog ships
#                            (en|nl|de|fr|es|it|pt), or all of them (default: all)
#   --screen <name>|all      list | reply | settings | signatures | add-account | calendar |
#                            invitation
#                            (default: all; which does NOT include `signatures`; see below)
#   --out <dir>              Where the PNGs land (default: showcase-screenshots/)
#   --simulator <name>       Apple simulator to use (default: a store-sized one per platform, which
#                            is NOT the one boot.sh picks; see "what this run is photographing")
#   --serial <serial>        Android device/emulator to use (default: the target's AVD)
#   --avd <name>             Android AVD to use (default: the target's, from devices.local.sh)
#   --no-build               Skip the build; reuse what is already installed. Refused when that
#   --hidpi                    Linux only: capture at 2000x1400, scale 1.5, for Flathub's
#                              screenshot cap. The default is the store set's 1280x720.
#                            build is older than the sources it would be photographing, or when
#                            there is none on the target device (macOS, iPhone, iPad, Linux; see
#                            `report_capture_target`)
#
# Examples:
#   scripts/dev/showcase.sh macos                       # 49 PNGs: 7 captures x 7 languages
#   scripts/dev/showcase.sh iphone --locale nl
#   scripts/dev/showcase.sh android --screen reply --no-build
#   scripts/dev/showcase.sh ipad --simulator "iPad Air 13-inch (M4)"
#   scripts/dev/showcase.sh android-tablet-10           # Google Play's 10-inch tablet slot
#   scripts/dev/showcase.sh android-tablet-7 --locale nl
#   scripts/dev/showcase.sh windows --screen settings    # Windows host only
#   scripts/dev/showcase.sh macos --screen calendar      # the headline calendar grid
#   scripts/dev/showcase.sh macos --screen list          # BOTH appearances: en-list + en-list-dark
#   scripts/dev/showcase.sh linux --locale de            # Linux host only; 6 screens (7 captures)
#
# The two `android-tablet-*` targets are not separate clients; they are the same APK on a different
# emulator, because Google Play has a **7-inch and a 10-inch tablet screenshot slot** and each wants
# its own set. The AVD is booted if it isn't already running and shut down again afterwards, the
# display is pinned to portrait, and the capture's pixel size is asserted against the slot: an
# unrotated 1920x1200 landscape frame filed under a portrait slot is exactly the kind of mistake a
# store reviewer finds for you.
#
# The Windows client builds only on Windows, so `showcase.sh windows` runs on a Windows host and
# delegates the launch + shutter to clients/windows/showcase.ps1 (which relaunches the exe per
# screen, exactly as clients/windows/control.ps1 does for the MAILCAL_* hooks).
set -euo pipefail
SHOWCASE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SHOWCASE_DIR/lib.sh"
# Android's device wrangling; booting the target's AVD, the status bar, rotation, cleanup; is far
# bulkier than any other platform's and lives in its own file, as the Windows half does.
source "$SHOWCASE_DIR/showcase-android.sh"

# Which emulators THIS machine uses. Git-ignored, because an AVD name is per-machine; see
# devices.local.sh.example. Absent is fine until an Android target actually needs a name, which is
# where the error explains it; every value is optional and `--avd` / `--serial` still win.
DEVICES_CONFIG="$SHOWCASE_DIR/devices.local.sh"
if [[ -f "$DEVICES_CONFIG" ]]; then
  # shellcheck source=/dev/null
  source "$DEVICES_CONFIG"
fi
: "${MAILCAL_AVD_PHONE:=}"
: "${MAILCAL_AVD_TABLET_7:=}"
: "${MAILCAL_AVD_TABLET_10:=}"

# The screens `--screen all` captures on every platform. `signatures` is deliberately NOT here:
# only the Windows and Linux clients have a showcase driver for it so far, and putting it in this
# list would make an Apple or Android run ask for a screen that client cannot reach; which fails
# as a mystifying wrong-screen capture rather than an error. Ask for it explicitly
# (`--screen signatures`), and move it into this list once every client can drive it.
ALL_SCREENS=(list reply settings add-account calendar invitation)
EXTRA_SCREENS=(signatures)

# The store screens a platform can actually drive to, for the same reason `doc_screens_for` below
# is per platform: a client that does not know a screen name has nothing to show but the mailbox
# list, and a capture of the inbox filed under another screen's name passes every later check.
#
# Every platform now reaches the whole set; Linux was the exception until it drew the invitation
# card, and it stays spelled out rather than folded into the default arm: `check-showcase-flag.sh`
# reads this arm to hold what Linux is offered against what `showcase.rs` accepts, and a list it
# cannot parse is a check that cannot fail. Linux's client *refuses* a name it has no surface for,
# so the failure this guards against is loud; but loud partway through a 35-shot run.
store_screens_for() { # <platform>: what `--screen all` captures
  case "$1" in
    linux) printf '%s\n' list reply settings add-account calendar invitation ;;
    *) printf '%s\n' "${ALL_SCREENS[@]}" ;;
  esac
}
store_extra_for() { # <platform>: offered only when named explicitly
  printf '%s\n' "${EXTRA_SCREENS[@]}"
}

# ---- the appearance axis ------------------------------------------------------------------------
#
# Which light/dark appearances a screen is shot in. This is NOT a screen: `dark` is a launch
# environment (MAILCAL_APPEARANCE, docs/debugging.md), so no client learns a new
# MAILCAL_SHOWCASE_SCREEN word for it and the cross-client screen contract is untouched. The dark
# capture lands beside its light twin as `<locale>-<screen>-dark.png`.
#
# **Every store capture states an appearance, including the light ones.** Left unstated, a set shot
# on a developer whose desktop is dark is a *dark* set; and nothing downstream can tell, because a
# dark screenshot of the right screen in the right language passes the size floor, the pixel-size
# assertion and the showcase-launch proof alike. It would simply reach the store looking like a
# different product than the one the other six locales show.
#
# One screen carries the dark set, and it is the mailbox list: it is what a listing opens on, so the
# pair reads as the same inbox in either theme rather than as two unrelated screens. Adding a second
# is one entry here; mind Google Play's ceiling of 8 images per slot, which 7 now sits one under.
# The push enforces that ceiling where it lives, which is not this repository.
DARK_SCREENS=(list)

appearances_for() { # <set> <screen>: the appearances one screen is captured in
  # The documentation set stays light-only: those images sit inside the help pages
  # (docs/user-docs.md), which are a light web surface, so a dark capture would read as a defect.
  if [[ "$1" == "store" && " ${DARK_SCREENS[*]} " == *" $2 "* ]]; then
    printf 'light\ndark\n'
  else
    printf 'light\n'
  fi
}

# The file-name suffix an appearance contributes. Light is the unsuffixed default so that every
# name already in a store gallery, a publisher's SCREEN_ORDER and this repo's docs keeps meaning
# what it meant; the dark shot is an addition, never a rename.
appearance_suffix() { # <appearance>
  [[ "$1" == "light" ]] || printf -- '-%s' "$1"
}

# ---- the documentation set (--set docs) ---------------------------------------------------------
#
# A second, separate set of screens, captured for the user guides rather than for a store listing
# (docs/user-docs.md). It is a *set*, not more screens on the store list, because the two have
# different destinations, different sizes, and different lifetimes: the store set is uploaded to
# three consoles at their exact pixel sizes, while a doc image is downscaled and content-addressed
# by the tooling that publishes the help pages.
#
# The four `setup-*` screens are one flow photographed at four moments. They are driven by the
# address the client types and by the core's *scripted* detection; in a showcase build
# `detect_account_settings` answers from a domain table instead of the network, so which screen
# appears is decided by the core rather than by a client faking a result. That is also what makes
# them capturable at all: detection is a network call, and this dataset has no network.
DOC_SCREENS_SHARED=(setup-email setup-detected setup-untrusted setup-manual)
# Agent access is desktop-only **by construction**; a mobile host passes no endpoint and compiles
# no listener (docs/mcp.md); so these exist on macOS alone today.
DOC_SCREENS_MACOS_ONLY=(mcp-off mcp-on mcp-accounts mcp-send)

# The doc screens a platform can actually drive to. Kept per platform rather than as one list,
# because a client that does not know a screen name silently falls back to the mailbox list; so
# asking Android for `mcp-on` would not error, it would file a photograph of the *inbox* under an
# agent-access filename, and every check in this script would pass it.
doc_screens_for() { # <platform>
  case "$1" in
    macos) printf '%s\n' "${DOC_SCREENS_SHARED[@]}" "${DOC_SCREENS_MACOS_ONLY[@]}" ;;
    # Linux drives none of the walkthrough yet: its setup flow takes no seeded address, so the four
    # `setup-*` moments cannot be reached without a client that fakes a detection result; which is
    # the one thing this set is designed not to do.
    linux) ;;
    *) printf '%s\n' "${DOC_SCREENS_SHARED[@]}" ;;
  esac
}
# Every locale the shared catalog ships (messages/*.json + project.inlang/settings.json), each with
# its own showcase seed; so a store listing gets chrome *and* sample mail in the same language.
ALL_LOCALES=(en nl de fr es it pt)

# Seconds to let the app settle before the shutter. The reply screen has the furthest to go: sync,
# open the message, fetch its body, then open the composer WebView and seed the quote.
settle_for() { # <screen>
  case "$1" in
    # Furthest to go: sync, open the message, fetch its body, open the composer WebView, seed the
    # quote. 11s was enough most of the time, which is the worst kind of enough; one Android run
    # shot the composer mid-load and filed a blank body with a spinner into the store set.
    reply) printf '16\n' ;;
    calendar) printf '9\n' ;; # sync events, then let the Win2D grid settle its first paint
    # The setup sheet has to appear, take the seeded address, and run detection; which answers
    # from the core's script, so it returns immediately rather than waiting on DNS.
    setup-*) printf '9\n' ;;
    # Settings opens on Advanced with the agent panel's state already applied.
    mcp-*) printf '8\n' ;;
    # Sync, find the invitation's row, open it, fetch its body, parse the iTIP payload, and lay
    # out the meeting-day preview under the card. One step short of `reply`, and a step past
    # every other screen.
    invitation) printf '12\n' ;;
    *) printf '7\n' ;;
  esac
}

# A capture of a blank screen is still a perfectly valid PNG; an all-black 1080x2316 frame is
# ~15 kB, where a real screenshot of the app's chrome and text is 100 kB and up. So check that the
# shutter caught *something*, rather than only that a file was written. A floor, not a proof: it
# catches a blank or black frame, never a subtly wrong one; and in particular it CANNOT tell
# showcase mail from real mail. `require_showcase_launch` below is what proves that.
#
# The floor is per screen, because a *whole-frame* floor is blind to the failure that actually
# happened: the composer's WebView had not finished loading, so `reply` photographed real app
# chrome above an empty white body. That still weighs 63 kB and sailed past a 40 kB floor. So
# `reply` gets its own floor, set between that failure and the smallest genuine capture; which is
# 142 kB (a keyboard-free 10-inch tablet; a phone is ~204 kB and a Mac ~551 kB). 100 kB sits with
# real margin on both sides: 1.6x above the half-loaded frame, 1.4x below the leanest good one.
min_capture_bytes() { # <screen>
  case "$1" in
    reply) printf '100000\n' ;;
    *) printf '40000\n' ;;
  esac
}

require_real_capture() { # <path> <screen>
  [[ -s "$1" ]] || die "capture produced no image: $1"
  local bytes floor
  bytes="$(wc -c <"$1" | tr -d '[:space:]')"
  floor="$(min_capture_bytes "$2")"
  if [[ "$bytes" -lt "$floor" ]]; then
    # Leave no under-filled PNG behind under a store asset's name, as require_capture_size does.
    rm -f "$1"
    die "capture looks blank or half-loaded ($bytes bytes, expected >= $floor): $1
  For 'reply' this usually means the composer WebView had not finished loading: raise settle_for."
  fi
}

# The pixel size a target's store slot expects, asserted on every capture where one is defined.
# A rotation that silently didn't take produces a perfectly valid PNG of the right app in the wrong
# shape, and the size floor above waves it through; the store is then the thing that tells you.
# `file` reports "PNG image data, <w> x <h>," on every host these scripts run on.
require_capture_size() { # <path> <WxH>
  local got
  got="$(file "$1" | sed -nE 's/.*PNG image data, ([0-9]+) x ([0-9]+).*/\1x\2/p')"
  if [[ "$got" != "$2" ]]; then
    # Leave no wrong-shaped PNG behind under a name that says which slot it is for; a failed run
    # writes no asset, the same rule the showcase interlock follows.
    rm -f "$1"
    die "capture is ${got:-unreadable}, expected $2: $1
  The display rotation or the device is wrong for this target: see --avd / --serial.
  On Windows: a monitor that is not at 200% scale, or a crop that took a different number of
  rows than it took on the capture before this one."
  fi
}

# ---- the showcase interlock ---------------------------------------------------------------------
#
# Every capture proves, before the shutter, that this launch really came up on the fictional
# dataset; see the long note in lib.sh. Without it a binary built before showcase mode existed
# ignores MAILCAL_SHOWCASE, opens the developer's real accounts, and photographs them; the size
# floor waves that through, because a real mailbox makes a big, healthy-looking PNG.
#
# Windows asserts inside clients/windows/showcase.ps1 (which owns its own launch), so it never
# reaches these helpers.

# Where the client's diagnostic log lives (docs/logging.md); for Android, a description, since the
# file is on the device rather than the host.
client_log_path() {
  case "$platform" in
    macos) macos_log_path ;;
    iphone | ipad) printf '%s/%s\n' "$(sim_app_container "$SIM_UDID")" "$APP_LOG_REL" ;;
    android) printf '/data/data/%s/files/logs/app.log (on device)\n' "$ANDROID_PKG" ;;
    linux) linux_log_path ;;
  esac
}

# The on-device Android log, copied to a host temp file so the shared byte-offset helpers apply.
android_log_copy() { # <dest>
  "${ADB[@]}" exec-out run-as "$ANDROID_PKG" cat files/logs/app.log >"$1" 2>/dev/null || true
}

# The client log's size in bytes right now; 0 when it doesn't exist yet (a first-ever launch).
client_log_size() {
  local path tmp size
  case "$platform" in
    macos | iphone | ipad | linux)
      path="$(client_log_path)"
      if [[ -f "$path" ]]; then wc -c <"$path" | tr -d '[:space:]'; else printf '0'; fi
      ;;
    android)
      tmp="$(mktemp)"
      android_log_copy "$tmp"
      size="$(wc -c <"$tmp" | tr -d '[:space:]')"
      rm -f "$tmp"
      printf '%s' "$size"
      ;;
  esac
}

# The bytes this launch appended to the client log.
client_log_since() { # <offset>
  local tmp
  case "$platform" in
    macos | iphone | ipad | linux) log_slice_since "$(client_log_path)" "$1" ;;
    android)
      tmp="$(mktemp)"
      android_log_copy "$tmp"
      log_slice_since "$tmp" "$1"
      rm -f "$tmp"
      ;;
  esac
}

# Stop the client, so a failed assertion never leaves a window full of real mail on screen.
stop_client() {
  case "$platform" in
    macos) pkill -f "$MACOS_BIN" 2>/dev/null || true ;;
    iphone | ipad) xcrun simctl terminate "$SIM_UDID" "$APPLE_BUNDLE_ID" 2>/dev/null || true ;;
    android) "${ADB[@]}" shell am force-stop "$ANDROID_PKG" >/dev/null 2>&1 || true ;;
    linux) pkill -f "$LINUX_BIN" 2>/dev/null || true ;;
  esac
}

# How long to keep waiting for the showcase marker after `settle_for` has already elapsed. It is
# generous because the cost of it being too small is not a slow run; it is a FALSE diagnosis.
SHOWCASE_LAUNCH_GRACE=45

require_showcase_launch() { # <locale> <log-offset-before-launch> <screen>
  local marker fresh waited=0
  marker="$(showcase_marker_for "$1")" || die "no showcase marker for locale '$1'"
  # WAIT for the marker; don't assert it once. `settle_for` is a fixed sleep, and a cold launch,
  # the first after a build, or any launch while the machine is busy (a few simulators running
  # apps is enough); can take many seconds longer to put its window up. Nothing in the app runs
  # until the scene appears, so the marker simply isn't there yet.
  #
  # A single assertion turns that delay into "did NOT enter showcase mode", whose own advice is to
  # rebuild; so the reader goes looking for a stale binary, a bundle-id clash, anything but a slow
  # start. That misdiagnosis cost an afternoon once; the check was right to fire, it just could not
  # tell "not in showcase mode" from "not there YET".
  while :; do
    fresh="$(client_log_since "$2")"
    if text_has_marker "$fresh" "$marker"; then
      # If it arrived late, the app has only just booted: give the screen the same settle it would
      # have had, so a slow launch is never photographed mid-drive.
      [[ "$waited" -eq 0 ]] || sleep "$(settle_for "$3")"
      return 0
    fi
    [[ "$waited" -lt "$SHOWCASE_LAUNCH_GRACE" ]] || break
    sleep 1
    waited=$((waited + 1))
  done
  stop_client
  die "this $platform launch did NOT enter showcase mode for '$1': refusing to take a screenshot.
  Whatever was on screen may be real mail; the app has been stopped.
  looked for: $marker
  in:         $(client_log_path)
  (${#fresh} bytes were appended to the log in $(( $(settle_for "$3") + SHOWCASE_LAUNCH_GRACE ))s)
  A slow start is already ruled out: it waited ${SHOWCASE_LAUNCH_GRACE}s past the settle. Likely a
  binary built before showcase mode existed (drop --no-build and rebuild), or a second copy of the
  app holding the bundle id $APPLE_BUNDLE_ID so this one never gets a window (quit it and re-run)."
}

[[ $# -ge 1 ]] || die "usage: showcase.sh <macos|iphone|ipad|android|android-tablet-7|android-tablet-10|windows> [--set store|docs] [--locale <code>|all] [--screen <name>|all] [--out <dir>] [--simulator <name>] [--serial <serial>] [--avd <name>] [--no-build]"
platform_raw="$1"
shift

LOCALE_ARG="all"
SCREEN_ARG="all"
SET_ARG="store"
OUT_DIR="$REPO_ROOT/showcase-screenshots"
SIMULATOR=""
SERIAL=""
AVD=""
# The compositor's output, as "<width>x<height> <scale>", and what it actually hands back. The
# default is the store set's, matching every other client's captures.
#
# `--hidpi` is Flathub's shape: it caps a screenshot at 1000x700, or 2000x1400 for HiDPI, and
# 1280x720 meets neither. Scale 1.5 rather than 2 because at 2 the output is 1000x700 logical, the
# three panes need 960, and the reading pane is clipped with nothing to say so. The two sizes differ
# because 1400 does not divide by 1.5: sway floors the logical height and returns 2000x1399. Asking
# for 1998 instead aborts inside pixman and grim captures nothing.
LINUX_OUTPUT="1280x720 1"
LINUX_CAPTURED="1280x720"
BUILD=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --set) SET_ARG="${2:?missing value for --set}"; shift 2 ;;
    --locale) LOCALE_ARG="${2:?missing value for --locale}"; shift 2 ;;
    --screen) SCREEN_ARG="${2:?missing value for --screen}"; shift 2 ;;
    --out) OUT_DIR="${2:?missing value for --out}"; shift 2 ;;
    --simulator) SIMULATOR="${2:?missing value for --simulator}"; shift 2 ;;
    --serial) SERIAL="${2:?missing value for --serial}"; shift 2 ;;
    --avd) AVD="${2:?missing value for --avd}"; shift 2 ;;
    --no-build) BUILD=0; shift ;;
    --hidpi) LINUX_OUTPUT="2000x1400 1.5"; LINUX_CAPTURED="2000x1399"; shift ;;
    *) die "unknown option '$1'" ;;
  esac
done

# TARGET names the screenshot *set*; it labels every PNG and picks the tablet AVD. `platform` stays
# the client platform every case statement below switches on, so a tablet run reuses the Android
# path wholesale rather than forking it.
TARGET=""
case "$platform_raw" in
  android-tablet-7 | android-tablet-10) TARGET="$platform_raw"; platform_raw="android" ;;
esac
case "$SET_ARG" in
  store | docs) ;;
  *) die "unknown --set '$SET_ARG' (store|docs)" ;;
esac
platform="$(normalize_platform "$platform_raw")"
: "${TARGET:=$platform}"

# The AVD each Android target is photographed on. The tablets fall back to the stock SDK profile
# names (`avdmanager create avd -d small_tablet` / `-d pixel_tablet`), which most machines have;
# the phone has no fallback on purpose, because the alternative is photographing whatever is
# plugged in; see devices.local.sh.example. `--avd` overrides any of them.
target_avd() { # <target>
  case "$1" in
    android) printf '%s\n' "$MAILCAL_AVD_PHONE" ;;
    android-tablet-7) printf '%s\n' "${MAILCAL_AVD_TABLET_7:-Small_Tablet}" ;;
    android-tablet-10) printf '%s\n' "${MAILCAL_AVD_TABLET_10:-Pixel_Tablet}" ;;
  esac
}
target_size() { # <target>
  case "$1" in
    android-tablet-7) printf '1200x1920\n' ;;
    android-tablet-10) printf '1600x2560\n' ;;
    # What the compositor actually hands back, which the run states rather than inherits, so a
    # frame that is not this size means it did not come up the way the run assumed. It is not
    # always what was asked for: see LINUX_OUTPUT.
    linux) printf '%s\n' "$LINUX_CAPTURED" ;;
    # The Windows client pins 1440x900 LOGICAL, so its physical frame follows the capturing
    # monitor's scale and only a 200% display gives the size the rest of the set is in. Every scale
    # sits inside the Store's own bounds, so nothing downstream objects to a set shot at 150%; it
    # simply looks smaller than the six languages beside it. This also holds a screen's light and
    # dark captures to one shape: a crop that comes out a row shorter in one appearance clears the
    # byte floor, the Store's pixel bounds and the showcase-launch proof alike.
    windows) printf '2880x1800\n' ;;
  esac
}
target_device_profile() { # <target>: the SDK device profile the default AVD is made from
  case "$1" in
    android) printf 'medium_phone\n' ;;
    android-tablet-7) printf 'small_tablet\n' ;;
    android-tablet-10) printf 'pixel_tablet\n' ;;
  esac
}
# The filename prefix a target's captures carry inside its platform directory. Play's three Android
# form factors share ONE directory (its picker is flat), so they are told apart by this prefix; every
# other platform has a directory to itself and needs none. See docs/store-screenshots.md.
target_prefix() { # <target>
  case "$1" in
    android) printf 'phone-\n' ;;
    android-tablet-7) printf 'tablet-7-\n' ;;
    android-tablet-10) printf 'tablet-10-\n' ;;
  esac
}

case "$LOCALE_ARG" in
  all) LOCALES=("${ALL_LOCALES[@]}") ;;
  *)
    # shellcheck disable=SC2076  # a literal match on the padded list, not a regex
    if [[ " ${ALL_LOCALES[*]} " =~ " $LOCALE_ARG " ]]; then
      LOCALES=("$LOCALE_ARG")
    else
      die "unknown --locale '$LOCALE_ARG' (${ALL_LOCALES[*]}|all)"
    fi
    ;;
esac
# The screens this run may ask for, which depends on the set; and, for docs, on the platform.
# Resolved after `platform` is known, so `--set docs --screen mcp-on` on Android is refused rather
# than silently photographing the inbox under an agent-access name.
if [[ "$SET_ARG" == "docs" ]]; then
  # shellcheck disable=SC2207  # newline-separated names, none of which can contain whitespace
  DEFAULT_SCREENS=($(doc_screens_for "$platform"))
  AVAILABLE_SCREENS=("${DEFAULT_SCREENS[@]}")
  [[ ${#AVAILABLE_SCREENS[@]} -gt 0 ]] || die "no documentation screens are driven on $platform yet
  (docs/user-docs.md names the set; doc_screens_for in this file names who can reach it)."
else
  # shellcheck disable=SC2207  # newline-separated names, none of which can contain whitespace
  DEFAULT_SCREENS=($(store_screens_for "$platform"))
  # shellcheck disable=SC2207
  AVAILABLE_SCREENS=("${DEFAULT_SCREENS[@]}" $(store_extra_for "$platform"))
fi
case "$SCREEN_ARG" in
  all) SCREENS=("${DEFAULT_SCREENS[@]}") ;;
  *)
    # Derived from the two lists above, the way --locale is derived from ALL_LOCALES. It used to
    # be a literal `list | reply | …)` arm; a *third* copy of the screen names, and the one that
    # fails worst: adding a screen everywhere else still left `--screen <new>` rejected by this
    # line, with an error message that listed the very name it had just refused.
    # shellcheck disable=SC2076  # a literal match on the padded list, not a regex
    if [[ " ${AVAILABLE_SCREENS[*]} " =~ " $SCREEN_ARG " ]]; then
      SCREENS=("$SCREEN_ARG")
    else
      die "unknown --screen '$SCREEN_ARG' for --set $SET_ARG on $platform (${AVAILABLE_SCREENS[*]}|all)"
    fi
    ;;
esac

# The set each store publisher reads is `showcase-screenshots/<platform>/`. No publisher is in this
# repository: all three are pointed at one of these directories from a checkout of it, and that
# directory IS the set (there is no copy or rename step). Keyed on `platform`, not `TARGET`, so the
# three Android form factors land together under their prefixes.
if [[ "$SET_ARG" == "docs" ]]; then
  PLATFORM_OUT_DIR="$OUT_DIR/docs/$platform"
else
  PLATFORM_OUT_DIR="$OUT_DIR/$platform"
fi
mkdir -p "$PLATFORM_OUT_DIR"

# ---- macOS -------------------------------------------------------------------------------------

MACOS_APP="$REPO_ROOT/clients/apple/build/DerivedData/Build/Products/Debug/AllodiaMail.app"
MACOS_BIN="$MACOS_APP/Contents/MacOS/AllodiaMail"

# LaunchServices strips the environment from `open`, so the binary is exec'd directly (as
# clients/apple/Scripts/build-and-run.sh does). `-AppleLanguages` pins the chrome for this launch
# only, via the NSArgumentDomain; the developer's stored Settings language is never rewritten.
macos_capture() { # <locale> <screen> <out>
  local offset
  offset="$(client_log_size)"
  pkill -f "$MACOS_BIN" 2>/dev/null || true
  sleep 1
  MAILCAL_SHOWCASE="$1" MAILCAL_SHOWCASE_SCREEN="$2" \
    "$MACOS_BIN" -AppleLanguages "($1)" >/dev/null 2>&1 &
  # Detach it, so the next iteration's pkill doesn't print a "Terminated" job notice over the log.
  disown
  sleep "$(settle_for "$2")"
  require_showcase_launch "$1" "$offset" "$2"
  # The helper separates "no window at all" from "windows, but not on this Space", because the two
  # look identical from here and want opposite responses; read the log, versus switch desktops.
  # Guessing wrong costs an afternoon: the log of an app on another Space says the scene appeared.
  local id status=0
  # --activate: `screencapture` photographs a window whether or not it is key, and an inactive one
  # draws grey traffic lights, a grey default button and a grey selection. Every later check,
  # file present, right size, not blank; passes on that image, so the run must refuse here or the
  # set silently ships looking disabled.
  id="$(macos_window_id --activate)" || status=$?
  case "$status" in
    0) ;;
    3) die "the macOS app is running, but its window is on another Space (Mission Control desktop),
     so it cannot be photographed. It did not crash. Check first whether the installed Allodia Mail
     is open on another desktop: it shares this bundle id, and a new instance joins wherever the
     existing one lives. Quit it, or switch to that desktop, and re-run." ;;
    4) die "the macOS app could not be brought to the front, so it would be photographed inactive
     (grey buttons, grey selection). Something else is holding focus: a modal dialog, a full-screen
     app, or the login window. Clear it and re-run." ;;
    *) die "the macOS app has no window (did it crash? see $(macos_log_path))" ;;
  esac
  # -l captures that window alone (not the desktop behind it); -o drops the drop-shadow.
  screencapture -x -o -l "$id" "$3"
}

# ---- Apple simulators --------------------------------------------------------------------------

# Store-valid screenshot sizes: iPhone 6.9" is 1320x2868, iPad 13" is 2064x2752.
default_simulator() { # <iphone|ipad>
  if [[ "$1" == "iphone" ]]; then printf 'iPhone 17 Pro Max\n'; else printf 'iPad Pro 13-inch (M5)\n'; fi
}

SIM_UDID=""
sim_prepare() {
  [[ -n "$SIMULATOR" ]] || SIMULATOR="$(default_simulator "$platform")"
  SIM_UDID="$(sim_udid_by_name "$SIMULATOR")" || die "no available simulator named '$SIMULATOR'"
  xcrun simctl bootstatus "$SIM_UDID" -b >/dev/null
  # The canonical marketing status bar, so every screenshot shows the same clock and full bars.
  xcrun simctl status_bar "$SIM_UDID" override \
    --time "09:41" --batteryState charged --batteryLevel 100 --cellularBars 4 --wifiBars 3
  # Mark iOS's keyboard tutorials as already seen. The reply screen focuses the composer, which
  # raises the keyboard; and on a fresh simulator that drags a full-width "Type English and Dutch"
  # (or QuickPath) introduction sheet across the bottom half of the screenshot.
  local seen
  for seen in DidShowContinuousPathIntroduction KeyboardDidShowInternationalInfoIntroduction \
    KeyboardDidShowProductivityTutorial DidShowGestureKeyboardIntroduction; do
    xcrun simctl spawn "$SIM_UDID" defaults write com.apple.Preferences "$seen" -bool true
  done
}

sim_capture() { # <locale> <screen> <out>
  local offset
  offset="$(client_log_size)"
  xcrun simctl terminate "$SIM_UDID" "$APPLE_BUNDLE_ID" 2>/dev/null || true
  sleep 1
  # simctl strips the SIMCTL_CHILD_ prefix and passes the rest to the app as its environment.
  # A simulator is the one target that does NOT inherit this shell's environment, so the
  # ambient MAILCAL_APPEARANCE has to be handed over explicitly like the two flags beside it.
  local appearance=()
  [[ -n "${MAILCAL_APPEARANCE:-}" ]] &&
    appearance=("SIMCTL_CHILD_MAILCAL_APPEARANCE=$MAILCAL_APPEARANCE")
  env "SIMCTL_CHILD_MAILCAL_SHOWCASE=$1" "SIMCTL_CHILD_MAILCAL_SHOWCASE_SCREEN=$2" \
    ${appearance[@]+"${appearance[@]}"} \
    xcrun simctl launch "$SIM_UDID" "$APPLE_BUNDLE_ID" -AppleLanguages "($1)" >/dev/null
  sleep "$(settle_for "$2")"
  require_showcase_launch "$1" "$offset" "$2"
  xcrun simctl io "$SIM_UDID" screenshot "$3" >/dev/null 2>&1
}

# ---- Windows -----------------------------------------------------------------------------------

# Everything Windows-specific lives in clients/windows/showcase.ps1: it relaunches the built exe
# with the MAILCAL_SHOWCASE flags (the app is single-instanced, so a flag needs a fresh process),
# asserts the launch really entered showcase mode, and shoots the window via PrintWindow. We're on
# the Windows host; normalize_platform enforces it.
WINDOWS_PS=""
windows_prepare() {
  WINDOWS_PS="$(pwsh_bin)"
  [[ -n "$WINDOWS_PS" ]] || die "no PowerShell (pwsh/powershell) found to drive the Windows client"
}

windows_capture() { # <locale> <screen> <out> <appearance>
  "$WINDOWS_PS" -NoProfile -File "$(to_win_path "$REPO_ROOT/clients/windows/showcase.ps1")" \
    -Locale "$1" -Screen "$2" -Out "$(to_win_path "$3")" -Appearance "$4" \
    -SettleSeconds "$(settle_for "$2")" >/dev/null
}

# ---- Linux -------------------------------------------------------------------------------------

LINUX_BIN="$REPO_ROOT/target/debug/mailcal-linux"


# The Wayland sockets that exist now, each as `name:inode`. The compositor names its own socket
# (`wl_display_add_socket_auto`), so a run identifies it by taking the one that was not there
# before: the developer's session already owns wayland-0, and a stale compositor from an
# interrupted run can own more.
#
# The inode is what makes that work across a sweep. The compositor releases its number when it
# exits and the
# next capture is handed the *same name* back; comparing names alone then finds nothing new and the
# second screenshot of every run fails. A reused name is still a newly created file, so it has a new
# inode.
linux_wayland_sockets() {
  local runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" sock
  for sock in "$runtime"/wayland-*; do
    [[ -S "$sock" ]] || continue
    printf '%s:%s ' "$(basename "$sock")" "$(stat -c '%i' "$sock" 2>/dev/null)"
  done
}

linux_compositor_socket() { # <linux_wayland_sockets output from before the compositor started>
  local waited=0 entry
  while [[ "$waited" -lt 20 ]]; do
    for entry in $(linux_wayland_sockets); do
      case " $1 " in *" $entry "*) continue ;; esac
      printf '%s\n' "${entry%:*}"
      return 0
    done
    sleep 1
    waited=$((waited + 1))
  done
  return 1
}

# GTK registers the application id on the session bus and hands a second launch off to the first,
# which then exits without ever reading its own environment; so a flag only takes effect in a fresh
# process, and the previous one must be gone before the next starts. An *installed* build owns the
# same name: a running Flatpak silently swallows every launch here, and the run photographs it
# instead. `flatpak kill <app-id>` first.
#
# Debug, not release: the whole of clients/linux/src/showcase.rs is `#![cfg(debug_assertions)]`.
#
# The capture in flight, for `linux_cleanup`. Between starting the compositor and killing it every
# way out is a `die` or an interrupt, and without this each one leaves a headless sway and its
# client running until the machine is rebooted. `INT`/`TERM` as well as `EXIT`, because Ctrl-C is
# how a long set actually gets abandoned.
LINUX_COMPOSITOR_PID=""
LINUX_COMPOSITOR_CONFIG=""

linux_cleanup() {
  [[ -n "$LINUX_COMPOSITOR_PID" ]] && kill -KILL "$LINUX_COMPOSITOR_PID" 2>/dev/null
  [[ -n "$LINUX_COMPOSITOR_CONFIG" ]] && rm -f "$LINUX_COMPOSITOR_CONFIG"
  return 0
}

# Captured on a headless compositor, never on the developer's session: sway tiles one client
# full-bleed with no border, so the output *is* the window, and grim reads it over wlr-screencopy
# with no portal permission and no focus. The app runs on the Wayland backend with the GL renderer,
# which is what ships; Xvfb would run it on X11 through GSK's cairo fallback and redraw every
# shadow and rounded corner in the set.
#
# sway rather than cage because the size has to be chosen, and cage offers no way: WLR_HEADLESS_OUTPUTS
# sets a count, not a size, and it implements no wlr-output-management. Weston can be sized and
# implements no wlr-screencopy, so grim cannot read it.
#
# `--unsupported-gpu` is required: sway refuses to start under the proprietary Nvidia driver, over a
# GPU the headless backend never touches.
#
# ⚠️ SIGKILL the compositor. wlroots aborts inside `wl_display_terminate`, and each abort files an
# apport crash report: 49 of them across a full set. Taking its client away first reaches the same
# call. SIGKILL has no core-dump action, and grim has already read the pixels.
linux_capture() { # <locale> <screen> <out>
  local offset before sock compositor_pid config
  offset="$(client_log_size)"
  stop_client
  sleep 1
  # `exec` in the config rather than an argument: sway takes no client on its command line, and a
  # child it starts itself inherits the showcase environment set on sway.
  config="$(mktemp)"
  LINUX_COMPOSITOR_CONFIG="$config"
  cat >"$config" <<CONFIG
output HEADLESS-1 resolution ${LINUX_OUTPUT% *} scale ${LINUX_OUTPUT##* }
default_border none
default_floating_border none
gaps inner 0
gaps outer 0
exec $LINUX_BIN
CONFIG
  before="$(linux_wayland_sockets)"
  MAILCAL_SHOWCASE="$1" MAILCAL_SHOWCASE_SCREEN="$2" \
    env -u DISPLAY -u WAYLAND_DISPLAY \
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 \
    sway --unsupported-gpu -c "$config" >/dev/null 2>&1 &
  compositor_pid=$!
  LINUX_COMPOSITOR_PID="$compositor_pid"
  # Detach it, so the next iteration's pkill doesn't print a "Terminated" job notice over the log.
  disown
  sock="$(linux_compositor_socket "$before")" ||
    die "sway opened no Wayland socket: it did not start, or $LINUX_BIN exited immediately"
  sleep "$(settle_for "$2")"
  require_showcase_launch "$1" "$offset" "$2"
  WAYLAND_DISPLAY="$sock" grim "$3" ||
    die "grim could not capture the compositor's output on $sock"
  kill -KILL "$compositor_pid" 2>/dev/null || true
  rm -f "$config"
  LINUX_COMPOSITOR_PID=""
  LINUX_COMPOSITOR_CONFIG=""
  stop_client
}

# ---- build + run -------------------------------------------------------------------------------

# Build once, in showcase mode, so the build's own launch already avoids the real accounts.
build_once() {
  [[ "$BUILD" == "1" ]] || { info "skipping the build (--no-build)"; return 0; }
  info "building the $platform client"
  case "$platform" in
    macos) MAILCAL_SHOWCASE=en "$REPO_ROOT/clients/apple/Scripts/build-and-run.sh" --macos >/dev/null ;;
    iphone | ipad)
      [[ -n "$SIMULATOR" ]] || SIMULATOR="$(default_simulator "$platform")"
      MAILCAL_SHOWCASE=en "$REPO_ROOT/clients/apple/Scripts/build-and-run.sh" \
        "--$platform" --simulator "$SIMULATOR" >/dev/null
      ;;
    android) MAILCAL_SHOWCASE=en "$REPO_ROOT/clients/android/build-and-run.sh" >/dev/null ;;
    # -NoRun: each capture launches the exe itself with its own flags. Debug (the default) is
    # required, not merely conventional; ShowcaseMode.IsOn is `#if DEBUG`-gated.
    windows) "$WINDOWS_PS" -NoProfile -File "$(to_win_path "$REPO_ROOT/clients/windows/build-and-run.ps1")" -NoRun >/dev/null ;;
    # Default features on purpose: `dev-harness` is the harness trust path, which a screenshot run
    # of an offline in-memory dataset has no use for.
    linux) (cd "$REPO_ROOT" && cargo build -p mailcal-linux >/dev/null) ;;
  esac
}

# ---- what this run is actually going to photograph ----------------------------------------------
#
# Both halves of "which app, on which device" have silently produced a wrong screenshot here, and
# neither is visible in the result: a capture of the wrong build, or of the wrong device, is a
# clean, correctly-sized, showcase-mode PNG of the right screen in the right language. Every check
# below this line passes it, and so does the eye.
#
# **The device**, because `boot.sh <platform>` and this script resolve DIFFERENT defaults. boot.sh
# takes whichever simulator is booted; this one takes the store-sized `default_simulator` (an iPhone
# 17 Pro Max, not the iPhone 17 you may have just been driving). Run one, then the other with
# `--no-build`, and you photograph a device you never looked at; measured here, and it cost an
# afternoon chasing a fix that was already working.
#
# **The build**, because `--no-build` shoots whatever is installed, which can predate the change
# being photographed by whole branches.

# The binary a capture will actually run, or empty where this script cannot name one.
#
# Android and Windows are deliberately absent: Android's APK lives on the device, where the file
# times come from the device's own clock, and Windows' exe is found by showcase.ps1's own newest-
# first search. A staleness check that has to guess is worse than none; it either cries wolf until
# it is ignored, or reassures wrongly.
installed_app_binary() { # <platform>
  case "$1" in
    macos) [[ -x "$MACOS_BIN" ]] && printf '%s\n' "$MACOS_BIN" ;;
    iphone | ipad)
      local container
      container="$(xcrun simctl get_app_container "$SIM_UDID" "$APPLE_BUNDLE_ID" app 2>/dev/null)" ||
        return 0
      [[ -x "$container/AllodiaMail" ]] && printf '%s\n' "$container/AllodiaMail"
      ;;
    linux) [[ -x "$LINUX_BIN" ]] && printf '%s\n' "$LINUX_BIN" ;;
  esac
}

# The source trees whose changes reach that binary. Over-inclusive on purpose: being told to rebuild
# when you did not need to costs a minute, and the failure it prevents costs a store submission.
client_sources_for() { # <platform>
  case "$1" in
    macos | iphone | ipad)
      printf '%s\n' "$REPO_ROOT/clients/apple/App" \
        "$REPO_ROOT/clients/apple/Packages/MailcalKit/Sources" \
        "$REPO_ROOT/clients/apple/project.yml" "$REPO_ROOT/crates" "$REPO_ROOT/messages"
      ;;
    linux) printf '%s\n' "$REPO_ROOT/clients/linux/src" "$REPO_ROOT/crates" "$REPO_ROOT/messages" ;;
  esac
}

# The device a capture lands on, for the report below.
capture_device_for() { # <platform>
  case "$1" in
    iphone | ipad) printf '%s (%s)\n' "$SIMULATOR" "$SIM_UDID" ;;
    android) printf '%s\n' "${SERIAL:-the only attached device}" ;;
    *) printf 'this host\n' ;;
  esac
}

# A file's modification time, readably. Every host these scripts run on takes `-r <filename>`,
# macOS documents it beside `-r seconds`, and GNU date has only the filename form. Never hand it a
# path that is entirely digits: BSD would read that as an epoch instead, and print 1970 rather than
# fail.
file_mtime() { # <path>
  date -r "$1" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || printf 'unknown\n'
}

# The first file under <root...> newer than <binary>, or nothing.
#
# Its own function so scripts/dev/tests can drive it over a temp tree. The decision it carries is
# "refuse this run", and a comparison that silently never fires is indistinguishable from a fresh
# build; which is the whole failure this section exists to stop.
#
# `find -newer` rather than arithmetic on timestamps: it stops at the first hit, and the file it
# names is the one that makes the case to the reader.
#
# Missing roots are dropped first, and then an empty list returns early; that pair is the guard,
# not the dropping on its own. `find` walks the roots it *can* reach and only complains about the
# rest, so a missing path costs nothing; `find` with **no** path at all searches the working
# directory, which here is the repo, and would name the first file it meets as "newer" than the
# build. That is a refusal nobody could act on.
newer_source_than() { # <binary> <root...>
  local binary="$1"
  shift
  local -a roots=()
  local path
  for path in "$@"; do
    [[ -e "$path" ]] && roots+=("$path")
  done
  [[ ${#roots[@]} -gt 0 ]] || return 0
  find "${roots[@]}" -type f -newer "$binary" -print -quit 2>/dev/null || true
}

# Name the device and the build before the first shutter, and; under `--no-build`; refuse a build
# older than the sources it is supposed to be showing.
report_capture_target() {
  local binary newer path
  local -a sources
  binary="$(installed_app_binary "$platform")"
  info "shooting $TARGET on $(capture_device_for "$platform")"

  if [[ -z "$binary" ]]; then
    if [[ "$BUILD" != "1" ]]; then
      case "$platform" in
        macos | iphone | ipad | linux)
          die "nothing is installed to photograph on $(capture_device_for "$platform").
  --no-build reuses an existing build, and there is none here. Drop --no-build."
          ;;
      esac
    fi
    return 0
  fi

  info "app built $(file_mtime "$binary")"
  [[ "$BUILD" != "1" ]] || return 0

  # A `while read` rather than `mapfile`: macOS ships bash 3.2, which has no `mapfile` at all, and
  # `#!/usr/bin/env bash` finds it long before any newer one.
  sources=()
  while IFS= read -r path; do
    sources+=("$path")
  done < <(client_sources_for "$platform")
  [[ ${#sources[@]} -gt 0 ]] || return 0

  newer="$(newer_source_than "$binary" "${sources[@]}")"
  [[ -n "$newer" ]] || return 0
  die "the installed build is older than the source it would be photographing.
  built:  $(file_mtime "$binary")  ${binary#"$REPO_ROOT"/}
  newer:  $(file_mtime "$newer")  ${newer#"$REPO_ROOT"/}
  Re-run without --no-build. A capture of a stale build looks exactly like a capture of a fresh
  one: right screen, right language, right pixel size, so nothing after this point can tell.
  A GENERATED file here (an Info.plist, a bindings or L10n source) is a real answer, not a false
  alarm: it was rewritten because its inputs moved, and this build predates that."
}

if [[ "$platform" == "macos" ]]; then
  require_cmd screencapture
fi
if [[ "$platform" == "linux" ]]; then
  require_cmd sway
  require_cmd grim
  trap linux_cleanup EXIT INT TERM
fi
# Windows resolves its PowerShell before build_once, which shells out through it.
if [[ "$platform" == "windows" ]]; then
  windows_prepare
fi

# Android prepares *before* the build, not after: the build installs and launches on a device, and
# on a tablet target that device is an emulator this script still has to boot. (The Apple
# simulators are the other way round; build-and-run.sh boots the one it is given.)
if [[ "$platform" == "android" ]]; then
  android_prepare
  trap android_cleanup EXIT
fi

build_once

if [[ "$platform" == "iphone" || "$platform" == "ipad" ]]; then
  sim_prepare
fi

# After every prepare, so `$SIM_UDID` / `$SERIAL` name the device this run resolved rather than the
# one the flags asked for, and after `build_once`, so a building run reports the build it just made.
report_capture_target

if [[ "$SET_ARG" == "docs" ]]; then
  # A doc image is downscaled and content-addressed later, so it has no store slot to match.
  # The blank-frame floor below still applies; that one catches a half-loaded screen.
  EXPECTED_SIZE=""
else
  EXPECTED_SIZE="$(target_size "$TARGET")"
fi
captured=0
for locale in "${LOCALES[@]}"; do
  for screen in "${SCREENS[@]}"; do
    for appearance in $(appearances_for "$SET_ARG" "$screen"); do
      suffix="$(appearance_suffix "$appearance")"
      if [[ "$SET_ARG" == "docs" ]]; then
        out="$PLATFORM_OUT_DIR/$locale-$screen$suffix.png"
      else
        out="$PLATFORM_OUT_DIR/$(target_prefix "$TARGET")$locale-$screen$suffix.png"
      fi
      info "capturing $TARGET / $locale / $screen / $appearance"
      # Exported, not passed: macOS and Linux exec the binary and inherit this, the simulator hop
      # re-exports it with the SIMCTL_CHILD_ prefix, and Android turns it into an intent extra.
      # Windows takes it as a parameter; its driver scrubs the ambient MAILCAL_* deliberately.
      export MAILCAL_APPEARANCE="$appearance"
      case "$platform" in
        macos) macos_capture "$locale" "$screen" "$out" ;;
        iphone | ipad) sim_capture "$locale" "$screen" "$out" ;;
        android) android_capture "$locale" "$screen" "$out" ;;
        windows) windows_capture "$locale" "$screen" "$out" "$appearance" ;;
        linux) linux_capture "$locale" "$screen" "$out" ;;
      esac
      require_real_capture "$out" "$screen"
      [[ -z "$EXPECTED_SIZE" ]] || require_capture_size "$out" "$EXPECTED_SIZE"
      captured=$((captured + 1))
    done
  done
done
unset MAILCAL_APPEARANCE

case "$platform" in
  iphone | ipad) xcrun simctl status_bar "$SIM_UDID" clear ;;
  macos) pkill -f "$MACOS_BIN" 2>/dev/null || true ;;
  linux) pkill -f "$LINUX_BIN" 2>/dev/null || true ;;
  # Don't leave the last capture's window sitting on the developer's desktop.
  windows) "$WINDOWS_PS" -NoProfile -Command "Get-Process Mailcal -ErrorAction SilentlyContinue | Stop-Process -Force" >/dev/null 2>&1 || true ;;
esac

info "wrote $captured screenshot(s) to $PLATFORM_OUT_DIR"
