#!/usr/bin/env python3
"""Unit tests for the connected-device selection rule and for one device's details.

No device and no `devicectl`: every fixture has the shape of a real record captured from an
iPhone 13 Pro plugged in over USB, with the identifying values replaced, so the rules can fail here
rather than in the middle of someone's device build.

Both rules exist because the obvious reading was wrong in the same way twice, and each time the
tooling blamed the phone. `xctrace list devices` reported that working device under
"== Devices Offline ==", so auto-detection found nothing at all; and the Developer Mode probe read
a listing `devicectl` prints for a person, which Xcode 27 reformatted under it.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ios_devices as subject

# The shape `devicectl list devices --json-output -` reports for a phone on USB, trimmed to the
# fields the rule reads, with the name and udid replaced by stand-ins (a real device name is a
# person's, and `cargo xtask check-public-hygiene` is right to refuse one). `tunnelState` is kept
# because it is the field that looks like the answer and is not.
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


# What `device info details --json-output -` reports under Xcode 27, trimmed to the fields the rule
# reads, identifying values replaced. The three `*Properties` dictionaries are still present and
# still correct here; the payload carries a `_deprecationNotice` saying they will be removed.
XCODE_27_DETAILS = {
    "result": {
        "properties": {
            "state": {"developerModeStatus": {"enabled": {"mode": 1}}, "name": "A test iPhone"},
            "hardware": {"marketingName": "iPhone 13 Pro", "platform": "iOS"},
            "software": {
                "osVersionNumber": {"stringValue": "18.7.8", "components": [18, 7, 8, 0, 0]}
            },
        },
        "deviceProperties": {"developerModeStatus": "enabled", "osVersionNumber": "18.7.8"},
        "hardwareProperties": {"marketingName": "iPhone 13 Pro"},
    }
}

# The same device as reported before `properties` existed: the deprecated dictionaries alone.
DEPRECATED_ONLY_DETAILS = {
    "result": {
        "deviceProperties": {"developerModeStatus": "enabled", "osVersionNumber": "18.7.8"},
        "hardwareProperties": {"marketingName": "iPhone 13 Pro"},
    }
}


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


class OneDeviceDetail(unittest.TestCase):
    """The fields `device.sh doctor` and the Developer Mode precondition read.

    The case this exists for: these were read with a sed over the listing `devicectl` prints for a
    person, and Xcode 27 reformatted it, `developerModeStatus: enabled` becoming
    `• Developer Mode Status: Enabled (1)`. Every such read began answering the empty string, so
    `test-ui.sh --device` refused a phone whose Developer Mode was on and told its owner to go and
    switch on the thing they had already switched on.
    """

    def test_developer_mode_is_read_from_the_properties_dictionary(self):
        self.assertEqual(subject.detail(XCODE_27_DETAILS, "developer-mode"), "enabled")

    def test_developer_mode_is_the_key_rather_than_anything_under_it(self):
        # `{"enabled": {"mode": 1}}`: reading a value here answers a dict, and `require_dev_mode`
        # compares against the string "enabled".
        off = {"result": {"properties": {"state": {"developerModeStatus": {"disabled": {}}}}}}
        self.assertEqual(subject.detail(off, "developer-mode"), "disabled")

    def test_a_version_is_unwrapped_to_the_string_the_deprecated_field_stated(self):
        self.assertEqual(subject.detail(XCODE_27_DETAILS, "os"), "18.7.8")

    def test_the_model_is_the_marketing_name(self):
        # Not `productType`, which is "iPhone14,2" and is not what a person recognises.
        self.assertEqual(subject.detail(XCODE_27_DETAILS, "model"), "iPhone 13 Pro")

    def test_the_deprecated_dictionaries_are_still_read_when_they_are_all_there_is(self):
        # They are announced for removal, not gone, and a toolchain predating `properties` reports
        # only these. Falling back is what keeps the answer from being a plausible empty string.
        for field, expected in (
            ("developer-mode", "enabled"),
            ("os", "18.7.8"),
            ("model", "iPhone 13 Pro"),
        ):
            self.assertEqual(subject.detail(DEPRECATED_ONLY_DETAILS, field), expected)

    def test_a_payload_carrying_neither_shape_answers_empty_rather_than_raising(self):
        # `require_dev_mode` then refuses, which is the safe direction: it cannot install anyway.
        self.assertEqual(subject.detail({}, "developer-mode"), "")
        self.assertEqual(subject.detail({"result": {}}, "model"), "")


if __name__ == "__main__":
    unittest.main()
