#!/usr/bin/env python3
"""Which physical iOS/iPadOS devices are connected right now, from `devicectl`.

`scripts/dev/lib.sh`'s `list_connected_devices` is the entry point; this file reads the JSON on
stdin and knows nothing about how it was fetched, so the selection rule is testable with no device
plugged in: `scripts/dev/tests/test_ios_devices.py`.

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


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, UnicodeDecodeError):
        # No devicectl, or output this does not understand. An empty list reads as "no device
        # connected", which is what the caller already handles, rather than a stack trace in the
        # middle of a build.
        return 0
    for udid, name, version in connected(payload):
        print(f"{udid}\t{name} ({version})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
