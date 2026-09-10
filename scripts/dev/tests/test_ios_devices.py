#!/usr/bin/env python3
"""Unit tests for the connected-device selection rule.

No device and no `devicectl`: the connected fixture has the shape of a real record captured from an
iPhone 13 Pro plugged in over USB, with the identifying values replaced, so the rule can fail here
rather than in the middle of someone's device build.

The case this exists for is the one that shipped broken: that device was reported by
`xctrace list devices` under "== Devices Offline ==" while it was working perfectly, so the
auto-detection that read that listing found nothing at all.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ios_devices as subject

# The shape `devicectl list devices --json-output -` reports for a phone on USB, trimmed to the
# fields the rule reads, with the name and udid replaced by stand-ins (a real device name is a
# person's, and check-public-hygiene.sh is right to refuse one). `tunnelState` is kept because it
# is the field that looks like the answer and is not.
CONNECTED_IPHONE = {
    "deviceProperties": {"name": "A test iPhone", "osVersionNumber": "18.7.8"},
    "hardwareProperties": {"udid": "00008110-000A1B2C3D4E0026", "platform": "iOS"},
    "connectionProperties": {
        "pairingState": "paired",
        "transportType": "wired",
        "tunnelState": "disconnected",
    },
}

# The Mac `devicectl` is running on, which it lists alongside the phones.
HOST_MAC = {
    "deviceProperties": {"name": "A host Mac", "osVersionNumber": "26.5"},
    "hardwareProperties": {"udid": "11111111-2222-3333-4444-555555555555", "platform": "macOS"},
    "connectionProperties": {"pairingState": "paired", "transportType": "wired"},
}


def payload(*devices: dict) -> dict:
    return {"result": {"devices": list(devices)}}


class ConnectedDevices(unittest.TestCase):
    def test_a_plugged_in_iphone_is_found_with_its_name_and_os(self):
        self.assertEqual(
            subject.connected(payload(CONNECTED_IPHONE)),
            [("00008110-000A1B2C3D4E0026", "A test iPhone", "18.7.8")],
        )

    def test_a_disconnected_tunnel_is_not_a_disconnected_device(self):
        # The field that looks like the signal and is not: the captured device reported
        # `tunnelState: disconnected` while builds, installs and launches all worked against it.
        self.assertEqual(len(subject.connected(payload(CONNECTED_IPHONE))), 1)

    def test_the_host_mac_is_not_a_device_to_build_for(self):
        self.assertEqual(subject.connected(payload(HOST_MAC)), [])

    def test_a_remembered_device_is_not_a_connected_one(self):
        # What replaces xctrace's Offline section: a device the Mac merely remembers has no live
        # transport to reach it by. Both shapes are refused, because which one `devicectl` emits
        # is its business rather than this rule's.
        absent = {**CONNECTED_IPHONE, "connectionProperties": {"pairingState": "paired"}}
        null = {
            **CONNECTED_IPHONE,
            "connectionProperties": {"pairingState": "paired", "transportType": None},
        }
        self.assertEqual(subject.connected(payload(absent)), [])
        self.assertEqual(subject.connected(payload(null)), [])

    def test_an_unpaired_device_is_refused_because_nothing_can_be_installed_to_it(self):
        unpaired = {
            **CONNECTED_IPHONE,
            "connectionProperties": {"pairingState": "unpaired", "transportType": "wired"},
        }
        self.assertEqual(subject.connected(payload(unpaired)), [])

    def test_an_ipad_over_the_network_counts(self):
        ipad = {
            "deviceProperties": {"name": "An iPad", "osVersionNumber": "18.6"},
            "hardwareProperties": {"udid": "00008103-001234567890001E", "platform": "iPadOS"},
            "connectionProperties": {"pairingState": "paired", "transportType": "localNetwork"},
        }
        self.assertEqual(
            subject.connected(payload(ipad)),
            [("00008103-001234567890001E", "An iPad", "18.6")],
        )

    def test_two_connected_devices_are_both_reported_so_the_caller_can_refuse(self):
        # `device_udid` refuses to guess between them and asks for MAILCAL_DEVICE; it can only do
        # that if both arrive here.
        second = {**CONNECTED_IPHONE, "hardwareProperties": {"udid": "OTHER", "platform": "iOS"}}
        self.assertEqual(len(subject.connected(payload(CONNECTED_IPHONE, second))), 2)

    def test_nothing_at_all_is_an_empty_list_rather_than_an_error(self):
        self.assertEqual(subject.connected(payload()), [])
        self.assertEqual(subject.connected({}), [])


if __name__ == "__main__":
    unittest.main()
