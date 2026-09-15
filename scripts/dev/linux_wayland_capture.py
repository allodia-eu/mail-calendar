#!/usr/bin/env python3
"""Capture the whole screen through the desktop portal, for a client on the developer's desktop.

The fallback, not the good route. It exists for a client already running on the GNOME session,
where nothing smaller than the screen can be had:

- **No capture protocol.** `wayland-info` on a GNOME session lists neither
  `zwlr_screencopy_manager_v1` nor `ext_image_copy_capture_v1`, so grim and every tool like it
  reports "compositor doesn't support wlr-screencopy" and stops. That is a decision rather than a
  version: GNOME exposes capture over D-Bus instead, so no newer grim reaches it.
- **The D-Bus doors are shut.** `org.gnome.Shell.Screenshot.Screenshot` and `.ScreenshotWindow`
  both answer `AccessDenied`, and so does `org.gnome.Shell.Introspect.GetWindows`, which is the
  only thing that would say where a window is or which one is on top. `gnome-screenshot` is a
  caller of the first, and falls back to an X11 path GNOME 50 no longer has.
- **The ScreenCast portal is no way round it.** It can cast a single window, but only after a
  picker the user clicks, and its restore token is single-use and dies with the window it names.
  This client is relaunched on every launch hook, so every capture would raise the picker again.

So a crop to a guessed rectangle would produce a clean, correctly-sized PNG of whatever happened
to be stacked above the client. A full screen that obviously contains the wrong thing is the
honest failure mode.

**To capture the window itself, run the client on a compositor we start**:
`clients/linux/build-and-run.sh --headless`, after which `screenshot.sh linux` finds that session
and grim reads its output. That route also catches popovers, which no window capture can.
scripts/dev/linux_session.sh has the rest.
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
    away again is the caller's job. A capture loop that only copies leaves one behind on every
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
            f"the desktop portal refused the screenshot (response {outcome.get('code')}): the "
            "first capture on a machine raises a permission dialog that has to be approved once "
            "by hand; run it with a human present, or take the window instead of the screen with "
            "clients/linux/build-and-run.sh --headless, which needs no portal at all"
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
