#!/usr/bin/env bash
# Drive the running client. The reliable, cross-platform control primitive is the deterministic
# MAILCAL_* launch hooks passed at boot (they put the app in a known state without pixel-tapping,
# e.g. `MAILCAL_OPEN_FIRST=1 scripts/dev/boot.sh macos` opens the first message). This script adds
# input injection where a stable CLI exists; Android via adb, the iOS simulator via idb if
# installed, macOS via the Accessibility API; and points to the hooks + screenshots elsewhere.
#
#   scripts/dev/control.sh android tap <x> <y>
#   scripts/dev/control.sh android text "<simple text>"
#   scripts/dev/control.sh android key back|enter|home|del|<KEYCODE|number>
#   scripts/dev/control.sh android swipe <x1> <y1> <x2> <y2> [ms]
#   scripts/dev/control.sh android ui-dump            # accessibility tree (XML), to find node coords
#   scripts/dev/control.sh iphone ui-dump             # the live accessibility tree (assertion oracle)
#   scripts/dev/control.sh iphone find "<label>"      # -> "<x> <y>" in points, pipe straight into tap
#   scripts/dev/control.sh iphone press "<label>"     # find + tap in one call (idb has no AXPress)
#   scripts/dev/control.sh iphone probe <x> <y>       # the element under a point; reads the nav bar,
#                                                     #   which ui-dump cannot enumerate (see below)
#   scripts/dev/control.sh iphone tap <x> <y> | text "<s>"    # all six need idb (https://fbidb.io)
#   scripts/dev/control.sh macos tap <x> <y> | text "<s>" | key return|escape|...
#   scripts/dev/control.sh macos drag <x1> <y1> <x2> <y2> [hold-ms]
#                                                     #   a REAL mouse drag (down, dragged*, up),
#                                                     #   the only way to exercise a drag gesture;
#                                                     #   hold-ms presses before moving, for anything
#                                                     #   gated on a long press
#   scripts/dev/control.sh macos find "<label>"       # -> "<x> <y>", pipe straight into tap
#   scripts/dev/control.sh macos press "<label>"      # semantic AXPress; prefer this for anything
#                                                     #   that acts; a tap is a pixel event and lands
#                                                     #   wherever that point happens to be
#   scripts/dev/control.sh macos ui-dump              # the live Accessibility tree (assertion oracle)
#   scripts/dev/control.sh linux activate "Reply"      # semantic AT-SPI action, no pixel coordinates
#   scripts/dev/control.sh linux set-text "Title" "Team planning"
#   scripts/dev/control.sh linux ui-dump               # the live GTK accessibility tree
#   scripts/dev/control.sh windows open-first|calendar|home   # relaunch into a known state (launch hooks)
#   scripts/dev/control.sh windows ui-dump            # the live window's UI Automation tree
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

[[ $# -ge 2 ]] || die "usage: control.sh <platform> <action> [args...]"
platform="$(normalize_platform "$1")"; shift
action="$1"; shift

android_keycode() {
  case "$1" in
    back) printf 'KEYCODE_BACK\n' ;;
    enter) printf 'KEYCODE_ENTER\n' ;;
    home) printf 'KEYCODE_HOME\n' ;;
    del|delete) printf 'KEYCODE_DEL\n' ;;
    *) printf '%s\n' "$1" ;;  # a raw KEYCODE_* name or numeric code
  esac
}

case "$platform" in
  android)
    adb="$(adb_bin)"
    case "$action" in
      tap)   [[ $# -ge 2 ]] || die "tap <x> <y>"; "$adb" shell input tap "$1" "$2" ;;
      # `input text` treats %s as space; single-quote the arg so the on-device shell doesn't
      # reinterpret metacharacters (& ; | < > ...). A literal single-quote in the text isn't supported.
      text)  [[ $# -ge 1 ]] || die "text <string>"; "$adb" shell input text "'${1// /%s}'" ;;
      key)   [[ $# -ge 1 ]] || die "key <back|enter|home|del|KEYCODE|number>"; "$adb" shell input keyevent "$(android_keycode "$1")" ;;
      swipe) [[ $# -ge 4 ]] || die "swipe <x1> <y1> <x2> <y2> [ms]"; "$adb" shell input swipe "$1" "$2" "$3" "$4" "${5:-300}" ;;
      ui-dump)
        "$adb" shell uiautomator dump >/dev/null 2>&1 || die "uiautomator dump failed (is the app foregrounded?)"
        "$adb" exec-out cat /sdcard/window_dump.xml ;;
      *) die "unknown android action '$action' (tap|text|key|swipe|ui-dump)" ;;
    esac
    ;;
  iphone|ipad)
    # Driven through Meta's idb (scripts/dev/ios_ui_idb.py formats what it reports). Same two
    # halves as macOS, and the read half is again the valuable one: `ui-dump` prints what the app
    # actually shows, so a flow is checked by grep rather than by eye; and it is the only way to
    # see that a card has collapsed its children into one accessibility node, which a screenshot
    # renders perfectly while VoiceOver can no longer reach the buttons inside it.
    #
    # `describe-all` frames and `idb ui tap` are both in POINTS, so `find` composes straight into
    # `tap` with no scaling and no Simulator-window arithmetic; which is what `press` does in one
    # call, since idb has no AXPress equivalent to invoke an element directly.
    #
    # Every call carries --udid and (re)starts the companion via lib.sh's idb_sim_udid; see there
    # for why an unqualified call fails outright. The udid comes from simctl, so this can only
    # ever drive a simulator. Unlike macOS's CGEvent path, nothing has to be frontmost on the Mac.
    #
    # KNOWN GAP: `describe-all` does not enumerate NAVIGATION-BAR / TOOLBAR items; the bar shows
    # up as one unlabelled `AXGroup [220,89]` and Compose / More / Send / Cancel are absent, so
    # `find` and `press` cannot reach them. They are in the accessibility tree all the same
    # (`probe` finds them by pixel, and idb stops the same way in Apple's own Settings app), so a
    # toolbar item missing from a dump is NOT a finding about the app. For the top bar use
    # `probe <x> <y>` to identify a control, then `tap` it; or a MAILCAL_* launch hook.
    if ! command -v idb >/dev/null 2>&1; then
      cat >&2 <<EOF
No stable input CLI is wired for the $platform simulator without idb. Prefer the deterministic
launch hooks to reach a known state, then screenshot:
  MAILCAL_OPEN_FIRST=1 scripts/dev/boot.sh $platform      # open the first message
  MAILCAL_CALENDAR=1  scripts/dev/boot.sh $platform       # start on the calendar
  scripts/dev/screenshot.sh $platform
Install Meta's idb (https://fbidb.io) to enable: control.sh $platform ui-dump|find|press|probe|tap|text.
  brew tap facebook/fb && brew install idb-companion && pip3 install fb-idb
The 'idb' CLI lands in ~/Library/Python/<ver>/bin, which is not on PATH by default: add it.
EOF
      exit 2
    fi
    udid="$(idb_sim_udid "$platform")"
    ui=(/usr/bin/python3 "$REPO_ROOT/scripts/dev/ios_ui_idb.py")
    case "$action" in
      ui-dump) idb ui describe-all --udid "$udid" | "${ui[@]}" dump ;;
      find)    [[ $# -ge 1 ]] || die "find <text> [--all]"
               idb ui describe-all --udid "$udid" | "${ui[@]}" find "$@" ;;
      press)
        [[ $# -ge 1 ]] || die "press <label>"
        # `find` exits non-zero when nothing matches, and pipefail carries that into the
        # assignment; so a press whose label isn't on screen stops the flow loudly instead of
        # tapping a stale coordinate. Echo where it landed: there is no other trace of the choice.
        point="$(idb ui describe-all --udid "$udid" | "${ui[@]}" find "$1")" ||
          die "nothing on screen matches '$1': see: scripts/dev/control.sh $platform ui-dump"
        read -r x y <<<"$point"
        info "press '$1' -> $x $y"
        idb ui tap --udid "$udid" "$x" "$y" ;;
      probe)   [[ $# -ge 2 ]] || die "probe <x> <y>"
               # The one element under a point, in the same shape as a dump line. This is how you
               # read a navigation bar, which `ui-dump` cannot enumerate (see above).
               idb ui describe-point --udid "$udid" "$1" "$2" | "${ui[@]}" dump ;;
      tap)     [[ $# -ge 2 ]] || die "tap <x> <y>"; idb ui tap --udid "$udid" "$1" "$2" ;;
      text)    [[ $# -ge 1 ]] || die "text <string>"; idb ui text --udid "$udid" "$1" ;;
      *) die "unknown $platform action '$action' (ui-dump|find|press|probe|tap|text)" ;;
    esac
    ;;
  macos)
    # Driven through the Accessibility API + CGEvent (scripts/dev/macos-ax.swift). `ui-dump` is the
    # assertion oracle; it prints what the app actually shows, so a flow can be checked by grep
    # rather than by eye (an `AXSheet` node means a dialog is up). `find` resolves a label to
    # coordinates, so flows don't hardcode pixels:
    #   scripts/dev/control.sh macos tap $(scripts/dev/control.sh macos find "Reply")
    # Needs Accessibility permission for the terminal's host app; the script says so if it's missing.
    # The launch hooks (MAILCAL_OPEN_FIRST=1 etc.) still reach a known state more cheaply; prefer
    # them to open the app, and use this to drive what a hook can't reach.
    is_macos || die "the macOS client can only be driven from a macOS host"
    script="$REPO_ROOT/scripts/dev/macos-ax.swift"
    case "$action" in
      ui-dump) exec xcrun swift "$script" dump ;;
      find)    [[ $# -ge 1 ]] || die "find <text> [--all]"; exec xcrun swift "$script" find "$@" ;;
      press)   [[ $# -ge 1 ]] || die "press <label>"; exec xcrun swift "$script" press "$1" ;;
      tap)     [[ $# -ge 2 ]] || die "tap <x> <y>"; exec xcrun swift "$script" tap "$1" "$2" ;;
      drag)    [[ $# -ge 4 ]] || die "drag <x1> <y1> <x2> <y2> [hold-ms]"; exec xcrun swift "$script" drag "$@" ;;
      text)    [[ $# -ge 1 ]] || die "text <string>"; exec xcrun swift "$script" text "$1" ;;
      key)     [[ $# -ge 1 ]] || die "key <return|escape|tab|delete|up|down|left|right>"; exec xcrun swift "$script" key "$1" ;;
      *) die "unknown macos action '$action' (press|tap|drag|text|key|find|ui-dump)" ;;
    esac
    ;;
  linux)
    python=/usr/bin/python3
    [[ -x "$python" ]] || die "Linux UI control requires the distro /usr/bin/python3"
    "$python" -c 'import pyatspi' 2>/dev/null ||
      die "Linux UI control requires python3-pyatspi (install the clients/linux/README.md prerequisites)"
    script="$REPO_ROOT/scripts/dev/linux_ui_atspi.py"
    case "$action" in
      ui-dump) exec "$python" "$script" dump ;;
      find)
        [[ $# -ge 1 ]] || die "find <accessible name>"
        exec "$python" "$script" wait --name "$1" ;;
      activate)
        [[ $# -ge 1 ]] || die "activate <accessible name>"
        exec "$python" "$script" activate --name "$1" ;;
      set-text)
        [[ $# -ge 2 ]] || die "set-text <accessible name> <value>"
        exec "$python" "$script" set-text --name "$1" --text "$2" ;;
      *) die "unknown linux action '$action' (activate|find|set-text|ui-dump)" ;;
    esac
    ;;
  windows)
    # The WinUI app is driven by the deterministic MAILCAL_* launch hooks (reliable and
    # layout-independent); synthetic pixel input doesn't drive WinUI dependably. Each state verb
    # relaunches the built exe into a known state (the app is single-instanced, so a hook needs a
    # fresh process). Delegates to the client's PowerShell helper. We're on the Windows host
    # (normalize_platform enforces it).
    ps="$(pwsh_bin)"; [[ -n "$ps" ]] || die "no PowerShell (pwsh/powershell) found to drive the Windows client"
    script="$(to_win_path "$REPO_ROOT/clients/windows/control.ps1")"
    case "$action" in
      open-first|calendar|home|relaunch|ui-dump)
        exec "$ps" -NoProfile -ExecutionPolicy Bypass -File "$script" "$action" ;;
      *)
        cat >&2 <<EOF
unknown windows action '$action'. The Windows client is driven by deterministic launch hooks
(relaunch into a known state), not pixel taps. Synthetic input doesn't reliably drive WinUI:
  scripts/dev/control.sh windows open-first   # open the first message
  scripts/dev/control.sh windows calendar     # start on the calendar
  scripts/dev/control.sh windows home         # default view; also re-syncs (picks up delivered mail)
  scripts/dev/control.sh windows ui-dump      # the live UI Automation tree
Then: scripts/dev/screenshot.sh windows
EOF
        exit 2 ;;
    esac
    ;;
esac
