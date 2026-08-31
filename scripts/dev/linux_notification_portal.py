#!/usr/bin/env python3
"""Minimal notification portal boundary for the deterministic Linux runtime suite."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402


XML = """
<node>
  <interface name="org.freedesktop.portal.Notification">
    <property name="version" type="u" access="read"/>
    <method name="AddNotification">
      <arg type="s" name="id" direction="in"/>
      <arg type="a{sv}" name="notification" direction="in"/>
    </method>
    <method name="RemoveNotification">
      <arg type="s" name="id" direction="in"/>
    </method>
  </interface>
  <interface name="org.freedesktop.portal.NetworkMonitor">
    <property name="version" type="u" access="read"/>
    <method name="GetAvailable">
      <arg type="b" name="available" direction="out"/>
    </method>
    <method name="GetMetered">
      <arg type="b" name="metered" direction="out"/>
    </method>
    <method name="GetConnectivity">
      <arg type="u" name="connectivity" direction="out"/>
    </method>
    <method name="GetStatus">
      <arg type="a{sv}" name="status" direction="out"/>
    </method>
    <method name="CanReach">
      <arg type="s" name="hostname" direction="in"/>
      <arg type="u" name="port" direction="in"/>
      <arg type="b" name="reachable" direction="out"/>
    </method>
    <signal name="changed"/>
  </interface>
</node>
"""


def unpack(value: object) -> object:
    """Turn nested GLib variants into JSON-compatible fixture data."""
    if isinstance(value, GLib.Variant):
        return unpack(value.unpack())
    if isinstance(value, dict):
        return {str(key): unpack(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [unpack(item) for item in value]
    return value


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: linux_notification_portal.py <capture.jsonl>", file=sys.stderr)
        return 2
    capture = Path(sys.argv[1])
    connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    interfaces = Gio.DBusNodeInfo.new_for_xml(XML).interfaces

    def called(
        _connection: Gio.DBusConnection,
        _sender: str,
        _path: str,
        _interface: str,
        method: str,
        parameters: GLib.Variant,
        invocation: Gio.DBusMethodInvocation,
    ) -> None:
        if method == "GetAvailable" or method == "CanReach":
            invocation.return_value(GLib.Variant("(b)", (True,)))
            return
        if method == "GetMetered":
            invocation.return_value(GLib.Variant("(b)", (False,)))
            return
        if method == "GetConnectivity":
            invocation.return_value(GLib.Variant("(u)", (4,)))
            return
        if method == "GetStatus":
            status = {
                "available": GLib.Variant("b", True),
                "metered": GLib.Variant("b", False),
                "connectivity": GLib.Variant("u", 4),
            }
            invocation.return_value(GLib.Variant("(a{sv})", (status,)))
            return
        values = parameters.unpack()
        record = {"method": method, "id": values[0]}
        if method == "AddNotification":
            record["notification"] = unpack(values[1])
        with capture.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(record, sort_keys=True) + "\n")
        invocation.return_value(GLib.Variant("()", ()))

    def property_value(
        _connection: Gio.DBusConnection,
        _sender: str,
        _path: str,
        interface_name: str,
        _property: str,
    ) -> GLib.Variant:
        version = 3 if interface_name.endswith("NetworkMonitor") else 2
        return GLib.Variant("u", version)

    for interface in interfaces:
        connection.register_object(
            "/org/freedesktop/portal/desktop",
            interface,
            called,
            property_value,
            None,
        )
    reply = connection.call_sync(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "RequestName",
        GLib.Variant("(su)", ("org.freedesktop.portal.Desktop", 0)),
        GLib.VariantType.new("(u)"),
        Gio.DBusCallFlags.NONE,
        -1,
        None,
    )
    if reply.unpack()[0] not in (1, 4):
        print("could not own org.freedesktop.portal.Desktop", file=sys.stderr)
        return 1
    GLib.MainLoop().run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
