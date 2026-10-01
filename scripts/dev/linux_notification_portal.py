#!/usr/bin/env python3
"""Minimal desktop portal boundary for the deterministic Linux runtime suite.

Notifications and network status are recorded or answered; a print is accepted without a dialog
and the PDF the client hands over is kept beside the capture, as `printed-<n>.pdf`. A sandboxed
WebKitGTK prints only through this portal, and a real one would draw its dialog on whichever
desktop owns the session, so the suite answers it here.

Print dialogs are answered in pairs, or alone once `PRINT_PAIRING_SECONDS` have passed. A real
portal's dialog leaves the window usable, so a second print can start while the first is still
asking; pairing makes that overlap happen every time rather than whenever the driver is quick.
"""

from __future__ import annotations

import json
import os
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
  <interface name="org.freedesktop.portal.Print">
    <property name="version" type="u" access="read"/>
    <method name="PreparePrint">
      <arg type="s" name="parent_window" direction="in"/>
      <arg type="s" name="title" direction="in"/>
      <arg type="a{sv}" name="settings" direction="in"/>
      <arg type="a{sv}" name="page_setup" direction="in"/>
      <arg type="a{sv}" name="options" direction="in"/>
      <arg type="o" name="handle" direction="out"/>
    </method>
    <method name="Print">
      <arg type="s" name="parent_window" direction="in"/>
      <arg type="s" name="title" direction="in"/>
      <arg type="h" name="fd" direction="in"/>
      <arg type="a{sv}" name="options" direction="in"/>
      <arg type="o" name="handle" direction="out"/>
    </method>
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


PRINT_PAIRING_SECONDS = 5


def request_path(sender: str, options: GLib.Variant) -> str:
    """The Request object a portal call answers on, which the caller derives the same way."""
    token = options.lookup_value("handle_token", GLib.VariantType.new("s"))
    name = sender[1:].replace(".", "_")
    suffix = token.get_string() if token is not None else f"t{GLib.random_int()}"
    return f"/org/freedesktop/portal/desktop/request/{name}/{suffix}"


def emit_response(connection: Gio.DBusConnection, sender: str, path: str, results: dict) -> None:
    connection.emit_signal(
        sender,
        path,
        "org.freedesktop.portal.Request",
        "Response",
        GLib.Variant("(ua{sv})", (0, results)),
    )


def respond(connection: Gio.DBusConnection, sender: str, path: str, results: dict) -> None:
    """Emit the Request's Response once the method has returned, as a real portal does."""

    def emit() -> bool:
        emit_response(connection, sender, path, results)
        return False

    GLib.idle_add(emit)


class PrintDialogs:
    """The print dialogs still open, answered together; see the module's docstring."""

    def __init__(self, connection: Gio.DBusConnection) -> None:
        self.connection = connection
        self.waiting: list[tuple[str, str, dict]] = []

    def ask(self, sender: str, path: str, results: dict) -> None:
        self.waiting.append((sender, path, results))
        if len(self.waiting) >= 2:
            GLib.idle_add(self.answer)
        else:
            GLib.timeout_add_seconds(PRINT_PAIRING_SECONDS, self.answer)

    def answer(self) -> bool:
        waiting, self.waiting = self.waiting, []
        for sender, path, results in waiting:
            emit_response(self.connection, sender, path, results)
        return False


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: linux_notification_portal.py <capture.jsonl>", file=sys.stderr)
        return 2
    capture = Path(sys.argv[1])
    connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    interfaces = Gio.DBusNodeInfo.new_for_xml(XML).interfaces
    dialogs = PrintDialogs(connection)

    def called(
        connection: Gio.DBusConnection,
        sender: str,
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
        if method == "PreparePrint":
            # Accepted as asked: the settings and page setup the client proposed come back as the
            # user's choice, which is what pressing Print in an untouched dialog does.
            _parent, title, _settings, _page_setup, options = parameters.unpack()
            path = request_path(sender, parameters.get_child_value(4))
            record = {"method": method, "title": title, "options": unpack(options)}
            with capture.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(record, sort_keys=True) + "\n")
            invocation.return_value(GLib.Variant("(o)", (path,)))
            dialogs.ask(
                sender,
                path,
                {
                    "settings": parameters.get_child_value(2),
                    "page-setup": parameters.get_child_value(3),
                    "token": GLib.Variant("u", 1),
                },
            )
            return
        if method == "Print":
            fds = invocation.get_message().get_unix_fd_list()
            index = parameters.get_child_value(2).get_handle()
            printed = capture.parent / f"printed-{len(list(capture.parent.glob('printed-*.pdf')))}.pdf"
            with os.fdopen(fds.get(index), "rb") as source:
                printed.write_bytes(source.read())
            record = {"method": method, "title": parameters.unpack()[1], "file": str(printed)}
            with capture.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(record, sort_keys=True) + "\n")
            path = request_path(sender, parameters.get_child_value(3))
            invocation.return_value(GLib.Variant("(o)", (path,)))
            respond(connection, sender, path, {})
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
        version = 2 if interface_name.endswith("Notification") else 3
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
