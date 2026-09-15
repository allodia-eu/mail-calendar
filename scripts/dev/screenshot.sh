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
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/linux_session.sh"

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
    # Two routes, and which one runs is decided by where the client is, never by preference.
    #
    # A client in a headless session (`build-and-run.sh --headless`) gets the good one: sway tiles
    # it full-bleed, so the compositor's output *is* the window, popovers included. On the
    # developer's own GNOME desktop no such capture exists at all, so the fallback is the portal's
    # whole screen. This never starts a session of its own: that would photograph a fresh app
    # rather than the one being debugged.
    if linux_session_attach; then
      linux_session_capture "$OUT"
    else
      warn "no headless session, so this is the WHOLE SCREEN: GNOME offers a script no
       per-window capture, and will not say where the window is either. For the window alone:
       clients/linux/build-and-run.sh --headless, then run this again"
      "${MAILCAL_PYTHON:-/usr/bin/python3}" "$DEV_LIB_DIR/linux_wayland_capture.py" \
        --out "$OUT" >/dev/null
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
