#!/usr/bin/env python3
"""What `devicectl` says about physical iOS/iPadOS devices: which are connected, and one field of
one device's details.

`scripts/dev/lib.sh`'s `list_connected_devices` and `device_detail` are the entry points; this file
reads the JSON on stdin and knows nothing about how it was fetched, so both rules are testable with
no device plugged in: `scripts/dev/tests/test_ios_devices.py`.

⚠️ **Read `devicectl`'s JSON, never the listing it prints for a person.** Xcode 27 reformatted that
listing, `developerModeStatus: enabled` becoming `• Developer Mode Status: Enabled (1)`, so a sed
over it answers the empty string: the Developer Mode precondition refused a device that had it on
and named the switch the reader had already flipped, which sends them to the phone rather than to
the tooling. The JSON is the interface; the listing is prose.

⚠️ **`xctrace list devices` is not a reliable answer to this question, which is why this exists.**
It sorts a device into an "== Devices Offline ==" section that a live, usable device can sit in:
measured on an iPhone 13 Pro on iOS 18.7.8, plugged in over USB, with Developer Mode enabled, where
`devicectl` reported it available and every build, install and launch worked. Auto-detection built
on that section therefore reports "no physical iOS device found" while the device is sitting there
working, and the person debugging goes looking at the phone rather than at the tooling.

`devicectl` is also what every operation in `scripts/dev/device.sh` already uses, so reading the
device list from anything else is asking two tools whether the same device exists and believing the
one that is not going to be used.

The rule, in order of what each part rules out:

- `platform` must be iOS or iPadOS. It is what excludes the Mac host, which `devicectl` lists too.
- `transportType` must be a live one (`wired` or `localNetwork`). This is what replaces xctrace's
  Offline section: a device the Mac merely remembers has no transport to reach it by. `tunnelState`
  looks like the same signal and is not: it read `disconnected` on the working device above.
- `pairingState` must be `paired`, or nothing can be installed to it anyway.
"""

from __future__ import annotations

import json
import sys

# The platforms this repository builds a device for. `devicectl` also lists the Mac it runs on.
DEVICE_PLATFORMS = {"iOS", "iPadOS"}

# A transport the Mac can actually reach the device over. Anything else, including a missing value,
# means the device is remembered rather than present.
LIVE_TRANSPORTS = {"wired", "localNetwork"}


def connected(payload: dict) -> list[tuple[str, str, str]]:
    """The `(udid, name, os version)` of every connected device, in `devicectl`'s own order."""
    found = []
    for device in payload.get("result", {}).get("devices", []):
        hardware = device.get("hardwareProperties", {})
        connection = device.get("connectionProperties", {})
        properties = device.get("deviceProperties", {})
        udid = hardware.get("udid")
        if not udid:
            continue
        if hardware.get("platform") not in DEVICE_PLATFORMS:
            continue
        if connection.get("transportType") not in LIVE_TRANSPORTS:
            continue
        if connection.get("pairingState") != "paired":
            continue
        found.append((udid, properties.get("name", ""), properties.get("osVersionNumber", "")))
    return found


# Where `devicectl device info details` puts each field this repository reads. Xcode 27 added the
# `properties` dictionary and deprecated `hardwareProperties`, `deviceProperties` and
# `connectionProperties` ("will be removed in a future release", says the payload's own
# `_deprecationNotice`), so each field names the current path first and the deprecated one second.
# Both are read because either alone breaks on a toolchain the other was written for, and the way
# this fails is an empty string that reads as a real answer.
DETAIL_PATHS = {
    "developer-mode": (
        ("properties", "state", "developerModeStatus"),
        ("deviceProperties", "developerModeStatus"),
    ),
    "model": (
        ("properties", "hardware", "marketingName"),
        ("hardwareProperties", "marketingName"),
    ),
    "os": (
        ("properties", "software", "osVersionNumber"),
        ("deviceProperties", "osVersionNumber"),
    ),
}


def _unwrap(value: object) -> str:
    """The plain text of a details field, whichever generation of `devicectl` wrote it.

    The deprecated fields state a value; the `properties` ones wrap it, in two different ways. A
    version became `{"stringValue": "18.7.8", "components": [...]}`, and a developer-mode status
    became `{"enabled": {"mode": 1}}`, where the status is the KEY rather than anything under it.
    """
    if isinstance(value, dict):
        if "stringValue" in value:
            return str(value["stringValue"])
        return next(iter(value), "")
    return "" if value is None else str(value)


def detail(payload: dict, field: str) -> str:
    """One field of a `device info details` payload, or "" if this `devicectl` reports none."""
    result = payload.get("result", {})
    for path in DETAIL_PATHS[field]:
        found: object = result
        for key in path:
            found = found.get(key, {}) if isinstance(found, dict) else {}
        text = _unwrap(found)
        if text:
            return text
    return ""


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, UnicodeDecodeError):
        # No devicectl, or output this does not understand. An empty answer reads as "no device
        # connected" or "field unavailable", both of which the callers already handle, rather than
        # a stack trace in the middle of a build.
        return 0
    if len(sys.argv) > 1:
        print(detail(payload, sys.argv[1]))
        return 0
    for udid, name, version in connected(payload):
        print(f"{udid}\t{name} ({version})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
