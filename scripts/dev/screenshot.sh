#!/usr/bin/env bash
# Capture a screenshot of the running client. Fills the gap the Apple build-and-run.sh notes
# (it doesn't screenshot). Prints the output path so it can be opened/attached.
#
#   scripts/dev/screenshot.sh macos [out.png]
#   scripts/dev/screenshot.sh iphone
#   scripts/dev/screenshot.sh android /tmp/before.png
#   scripts/dev/screenshot.sh linux /tmp/mailcal-linux.png
#
# Default output: ${TMPDIR:-/tmp}/mailcal-<platform>.png (overwritten each run).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

# The client's X window, newest first. Only ever an answer on X11 or under Xvfb: a Wayland client
# has none, which is what the Wayland branch below exists for.
linux_x_window() {
  local windows=()
  mapfile -t windows < <(xdotool search --onlyvisible --class mailcal-linux 2>/dev/null || true)
  [[ ${#windows[@]} -gt 0 ]] ||
    mapfile -t windows < <(xdotool search --onlyvisible --name "Allodia Mail" 2>/dev/null || true)
  [[ ${#windows[@]} -gt 0 ]] || return 1
  printf '%s\n' "${windows[${#windows[@]}-1]}"
}

[[ $# -ge 1 ]] || die "usage: screenshot.sh <macos|iphone|ipad|android|windows|linux> [out.png]"
platform="$(normalize_platform "$1")"; shift
OUT_ARG="${1:-}"
OUT="${OUT_ARG:-${TMPDIR:-/tmp}/mailcal-$platform.png}"

case "$platform" in
  macos)
    # Full screen (robust; the app window may not be frontmost). -x = silent.
    require_cmd screencapture
    screencapture -x "$OUT"
    ;;
  iphone|ipad)
    udid="$(booted_sim_udid "$platform")" || die "no booted $platform simulator: boot one first: scripts/dev/boot.sh $platform"
    xcrun simctl io "$udid" screenshot "$OUT"
    ;;
  android)
    "$(adb_bin)" exec-out screencap -p >"$OUT"
    ;;
  linux)
    mkdir -p "$(dirname "$OUT")"
    # Which capture route exists is decided by the session, not by preference. A Wayland client has
    # no X window at all, so every X tool below finds nothing and then reports the DISPLAY it was
    # handed, which is set and reachable because XWayland is running.
    if [[ "${MAILCAL_LINUX_HEADLESS:-0}" == "1" ]]; then
      # Xvfb has no compositor, so the X backing pixels are the pixels under test, and they are this
      # window's own whatever is stacked over it. Avoid gnome-screenshot here: it expects a
      # desktop-shell screenshot service that a private headless session intentionally does not run.
      # This is the only route that captures the *window* rather than the screen.
      require_cmd xdotool
      require_cmd xwd
      magick="$(imagemagick_bin)" || die "ImageMagick is required (install 'imagemagick')"
      window="$(linux_x_window)" || die "no Allodia Mail & Calendar window on DISPLAY=${DISPLAY:-unset}"
      scratch="$(mktemp --suffix=.xwd)"
      trap 'rm -f "$scratch"' EXIT
      xwd -silent -id "$window" -out "$scratch"
      "$magick" "$scratch" "$OUT"
    elif [[ "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
      # The whole screen, deliberately: no per-window capture is available to a script here, and
      # neither the client's position nor which window is on top can be read. The reasoning, and
      # what to reach for when the window itself is needed, is in linux_wayland_capture.py.
      warn "Wayland session: capturing the whole screen (no per-window capture is available);
       for the window alone, use the Xvfb path: MAILCAL_LINUX_HEADLESS=1"
      "${MAILCAL_PYTHON:-/usr/bin/python3}" "$DEV_LIB_DIR/linux_wayland_capture.py" \
        --out "$OUT" >/dev/null
    else
      require_cmd xdotool
      require_cmd gnome-screenshot
      window="$(linux_x_window)" || die "no visible Allodia Mail & Calendar Linux window on DISPLAY=${DISPLAY:-unset}"
      xdotool windowactivate --sync "$window"
      gnome-screenshot --window --file "$OUT"
    fi
    ;;
  windows)
    # Capture the WinUI window via the client's PowerShell helper (PrintWindow). We're on the
    # Windows host (normalize_platform enforces it). Let the helper own the default path (under
    # %TEMP%) when the caller gave none, so we return a real Windows path rather than a POSIX one.
    ps="$(pwsh_bin)"; [[ -n "$ps" ]] || die "no PowerShell (pwsh/powershell) found to capture the Windows client"
    script="$(to_win_path "$REPO_ROOT/clients/windows/screenshot.ps1")"
    if [[ -n "$OUT_ARG" ]]; then
      OUT="$("$ps" -NoProfile -ExecutionPolicy Bypass -File "$script" -Out "$(to_win_path "$OUT_ARG")" | tail -1)"
    else
      OUT="$("$ps" -NoProfile -ExecutionPolicy Bypass -File "$script" | tail -1)"
    fi
    ;;
esac

info "screenshot: $OUT"
printf '%s\n' "$OUT"
