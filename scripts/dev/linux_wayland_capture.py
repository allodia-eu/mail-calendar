#!/usr/bin/env python3
"""Capture the screen on a Wayland session, for the Linux client's debug tooling.

A Wayland client has no X window, so the X tools the X11 path uses find nothing: `xdotool search`
returns empty and the capture dies reporting the DISPLAY it was given — which is set, and
reachable, because XWayland is running. Nothing in that message names the actual cause.

**This captures the whole screen, not the client's window, and that is deliberate.** Three things
a window capture needs are unavailable to a script here, and each one fails silently rather than
loudly:

- Per-window pixels. The portal offers the screen; `org.gnome.Shell.Screenshot.ScreenshotWindow`
  would offer the focused window but answers `AccessDenied` to everything except the shell's own
  UI. `gnome-screenshot --window` is not a way around that — it is a caller of the same denied
  API, and it stopped working in GNOME 49.
- The window's position. Wayland does not tell a client where it is on screen, so AT-SPI cannot
  either: measured here, a maximised terminal and a maximised browser both report `x=0 y=0`.
- Which window is on top. `STATE_ACTIVE` does not decide it — the same two windows both report
  active at once.

So a crop to AT-SPI's rectangle would produce a clean, correctly-sized PNG of whatever happened to
be stacked above the client. A full screen that obviously contains the wrong thing is the honest
failure mode; use `MAILCAL_LINUX_HEADLESS=1` under Xvfb when the capture has to be the window
itself, where `xwd -id` reads that window's own backing pixels whatever is in front of it.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
from pathlib import Path
from typing import Any
from urllib.parse import unquote, urlparse

PORTAL_BUS = "org.freedesktop.portal.Desktop"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
PORTAL_IFACE = "org.freedesktop.portal.Screenshot"


def die(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def portal_screenshot(bus: Any, timeout: float) -> Path:
    """Ask the desktop portal for a screen capture; returns the file it wrote.

    The portal hands back a *file it owns*, written under the user's Pictures directory. Taking it
    away again is the caller's job — a capture loop that only copies leaves one behind on every
    iteration, in a directory that belongs to the user rather than to us.
    """
    from gi.repository import Gio, GLib

    outcome: dict[str, Any] = {}
    loop = GLib.MainLoop()

    def on_response(_c: Any, _s: Any, _p: Any, _i: Any, _sig: Any, params: Any) -> None:
        outcome["code"], outcome["results"] = params.unpack()
        loop.quit()

    token = f"mailcal_capture_{os.getpid()}"
    sender = bus.get_unique_name()[1:].replace(".", "_")
    handle = f"{PORTAL_PATH}/request/{sender}/{token}"
    bus.signal_subscribe(
        PORTAL_BUS, "org.freedesktop.portal.Request", "Response", handle, None,
        Gio.DBusSignalFlags.NONE, on_response,
    )
    options = {
        "handle_token": GLib.Variant("s", token),
        # False asks for the capture without a picker. It does not mean "never prompts": the
        # first call on a machine raises a one-time permission dialog, and every call after it is
        # silent because the grant is remembered. An unattended run therefore needs one approved
        # capture to have happened first, which is what the refusal below says.
        "interactive": GLib.Variant("b", False),
    }
    bus.call_sync(
        PORTAL_BUS, PORTAL_PATH, PORTAL_IFACE, "Screenshot",
        GLib.Variant("(sa{sv})", ("", options)), None,
        Gio.DBusCallFlags.NONE, int(timeout * 1000), None,
    )
    GLib.timeout_add_seconds(int(timeout), lambda: (loop.quit(), False)[1])
    loop.run()

    if outcome.get("code") != 0:
        die(
            f"the desktop portal refused the screenshot (response {outcome.get('code')}) — the "
            "first capture on a machine raises a permission dialog that has to be approved once "
            "by hand; run it with a human present, or use MAILCAL_LINUX_HEADLESS=1 under Xvfb, "
            "which needs no portal at all"
        )
    uri = (outcome.get("results") or {}).get("uri")
    if not uri:
        die("the desktop portal reported success but returned no file")
    return Path(unquote(urlparse(uri).path))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, help="destination PNG")
    parser.add_argument("--timeout", type=float, default=20.0)
    args = parser.parse_args()

    try:
        import gi

        gi.require_version("Gio", "2.0")
        from gi.repository import Gio
    except (ImportError, ValueError) as exc:
        die(f"python3-gi is required for the Wayland capture path ({exc})")

    out = Path(args.out).expanduser()
    out.parent.mkdir(parents=True, exist_ok=True)
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    raw = portal_screenshot(bus, args.timeout)
    # Move rather than copy, so the portal's copy does not stay in the user's Pictures directory.
    shutil.move(str(raw), str(out))
    print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
