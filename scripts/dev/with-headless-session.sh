#!/usr/bin/env bash
# Run a command against a private headless Wayland compositor, then take it away again.
#
#   scripts/dev/with-headless-session.sh cargo test -p mailcal-linux --all-features
#   scripts/dev/with-headless-session.sh dbus-run-session -- <something needing a session bus too>
#
# What the GTK widget tests need is a display that is **not** the developer's. Left on the desktop
# they drive the live compositor: windows flash on screen, and a test that pumps the main loop
# dispatches Wayland events for surfaces an earlier test already destroyed, which segfaults inside
# libwayland-client. A pipeline has no session at all and needs one made for it either way.
#
# A compositor rather than a bare X server, for three reasons: the tests run on the backend and the
# GSK renderer that **ship**, rather than on X11 through the cairo fallback; `grim` can photograph
# the result, which is what the acceptance suite keeps as artifacts; and GNOME 50 has no X11
# session left, so X11 on this desktop is only ever XWayland, which is exactly the thing the tests
# must not find.
#
# ⚠️ `DISPLAY` is cleared for the command, not merely ignored. GDK prefers Wayland when
# `WAYLAND_DISPLAY` is set, but a GTK test that asks for X11 explicitly, or any tool that reads
# `DISPLAY` itself, would otherwise reach the developer's desktop through XWayland.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/linux_session.sh"

[[ $# -ge 1 ]] || die "usage: with-headless-session.sh <command> [args...]"
is_linux || die "a headless Wayland session needs a Linux host"

linux_session_start "${MAILCAL_HEADLESS_SIZE:-1440x900}" "${MAILCAL_HEADLESS_SCALE:-1}" mailcal-linux
trap linux_session_stop EXIT INT TERM

status=0
WAYLAND_DISPLAY="$LINUX_SESSION_DISPLAY" env -u DISPLAY "$@" || status=$?
exit "$status"
