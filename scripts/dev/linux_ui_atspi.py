#!/usr/bin/env python3
"""Inspect and activate the Linux client's GTK accessibility tree through AT-SPI."""

from __future__ import annotations

import argparse
import sys
import time
from collections.abc import Callable, Iterator
from typing import Any

PREFERRED_ACTIONS = ("activate", "click", "press", "open", "toggle")
APPLICATION_MARKERS = ("mailcal", "allodia mail")


def node_name(node: Any) -> str:
    """Return an accessible name without letting a defunct node abort a tree walk."""
    try:
        return str(node.name or "")
    except Exception:  # AT-SPI reports defunct remote objects through D-Bus exceptions.
        return ""


def node_description(node: Any) -> str:
    """The node's accessible description, or "" when it has none or the call fails.

    Worth reading, and not decoration: on GTK a labelled control is `labelled-by` its own label, and
    by the ARIA rules GTK follows that relation **beats** an explicit accessible label; so the
    supplementary half of what a screen reader says ("Accept … this invitation", a hold's "Awaiting
    your response") lives here and nowhere else (`AGENTS.md`). A tree that printed only names would
    report a passing button for a control that says nothing about what it acts on.
    """
    try:
        return str(node.description or "")
    except Exception:  # noqa: BLE001 - a remote node can vanish mid-walk
        return ""


def node_role(node: Any) -> str:
    """Return the normalized AT-SPI role name for a node."""
    try:
        role = str(node.getRoleName() or "")
        # AT-SPI2 has no role value for GTK's `switch`, so current libadwaita publishes the
        # actionable child of `AdwSwitchRow` as ROLE_LAST_DEFINED. Its semantic toggle action is
        # unambiguous and is what assistive technology invokes.
        if role.casefold() == "last defined" and "toggle" in action_names(node):
            return "switch"
        return role
    except Exception:
        return ""


def children(node: Any) -> Iterator[Any]:
    """Yield live children while tolerating a tree changing during inspection."""
    try:
        count = int(node.childCount)
    except Exception:
        return
    for index in range(count):
        try:
            child = node.getChildAtIndex(index)
        except Exception:
            continue
        if child is not None:
            yield child


def walk(node: Any) -> Iterator[Any]:
    """Walk an accessibility subtree in stable pre-order."""
    yield node
    for child in children(node):
        yield from walk(child)


# How many same-named nodes a text command scans before giving up. A labelled field is two or
# three (libadwaita's entry row draws its title twice); more than this and the name is too
# ambiguous to drive by anyway.
TEXT_SCAN = 8


def _matches(value: str, expected: str | None) -> bool:
    return expected is None or value.casefold() == expected.casefold()


def _contains(value: str, expected: str | None) -> bool:
    return expected is None or expected.casefold() in value.casefold()


def find_nodes(
    root: Any,
    *,
    name: str | None = None,
    role: str | None = None,
    within: str | None = None,
    within_role: str | None = None,
    contains_name: str | None = None,
    name_substring: str | None = None,
    description: str | None = None,
    limit: int | None = None,
) -> list[Any]:
    """Find exact accessible names/roles, optionally below a named ancestor.

    `name_substring` is the one inexact matcher, and it exists for the labels a client *composes*
    at runtime: a calendar block speaks its title, its time range, its calendar and; on an
    unanswered invitation; "Awaiting your response" (docs/calendar.md §4). The disclosure is the
    part the contract binds; the sentence around it is the fixture's own words and a date. Matching
    the whole string would pin the fixture instead of the rule.
    """
    scope = root
    if within is not None:
        # By role as well, where a name is not enough: a window and the button that opens it can
        # carry the same title, and the button is the one walk order reaches first.
        scope = next(
            (
                candidate
                for candidate in walk(root)
                if _matches(node_name(candidate), within)
                and _matches(node_role(candidate), within_role)
            ),
            None,
        )
        if scope is None:
            return []
    matches: list[Any] = []
    for candidate in walk(scope):
        if (
            _matches(node_name(candidate), name)
            and _matches(node_role(candidate), role)
            and _contains(node_name(candidate), name_substring)
            and _matches(node_description(candidate), description)
            and (
                contains_name is None
                or any(
                    _matches(node_name(descendant), contains_name)
                    for descendant in walk(candidate)
                )
            )
        ):
            matches.append(candidate)
            if limit is not None and len(matches) >= limit:
                break
    return matches


def action_names(node: Any) -> list[str]:
    """Return the unlocalized semantic actions exposed by a node."""
    try:
        interface = node.queryAction()
        return [str(interface.getName(index)) for index in range(interface.nActions)]
    except Exception:
        return []


def state_names(node: Any) -> list[str]:
    """Return the normalized AT-SPI states exposed by a node."""
    try:
        import pyatspi

        known = {
            int(value): name.removeprefix("STATE_").casefold()
            for name, value in vars(pyatspi).items()
            if name.startswith("STATE_") and isinstance(value, int)
        }
        return sorted(known.get(int(state), str(int(state))) for state in node.getState().getStates())
    except Exception:
        return []


def node_enabled(node: Any) -> bool:
    """Report whether the node can currently accept an action."""
    if hasattr(node, "enabled"):
        return bool(node.enabled)
    try:
        import pyatspi

        state = node.getState()
        return bool(
            state.contains(pyatspi.STATE_ENABLED) or state.contains(pyatspi.STATE_SENSITIVE)
        )
    except Exception:
        return False


def node_showing(node: Any) -> bool:
    """Report whether AT-SPI says the node is visible in the current layout."""
    if hasattr(node, "showing"):
        return bool(node.showing)
    try:
        import pyatspi

        state = node.getState()
        return bool(
            state.contains(pyatspi.STATE_SHOWING) and state.contains(pyatspi.STATE_VISIBLE)
        )
    except Exception:
        return False


def activate_node(node: Any) -> bool:
    """Invoke an enabled semantic action exposed by the accessible node."""
    if not node_enabled(node):
        return False
    try:
        interface = node.queryAction()
        available = [str(interface.getName(index)).casefold() for index in range(interface.nActions)]
        for preferred in PREFERRED_ACTIONS:
            if preferred in available:
                return bool(interface.doAction(available.index(preferred)))
    except Exception:
        pass
    return False


def set_node_text(node: Any, value: str) -> bool:
    """Replace an editable control's contents through AT-SPI, without keyboard synthesis."""
    if not node_enabled(node):
        return False
    try:
        editable = node.queryEditableText()
        return bool(editable.setTextContents(value))
    except Exception:
        return False


def node_editable(node: Any) -> bool:
    """Whether a node is a *field* rather than a label that merely has text.

    A GTK label answers the text interface too, so "read the contents of X" has to prefer the
    editable one or it reads the field's own label back at the caller.
    """
    try:
        node.queryEditableText()
        return True
    except Exception:
        return False


def read_node_text(node: Any) -> str | None:
    """Read a control's complete contents through AT-SPI's text interface."""
    try:
        text = node.queryText()
        return str(text.getText(0, text.characterCount))
    except Exception:
        return None


def render_tree(root: Any) -> str:
    """Render a privacy-reviewable role/name/action tree for test artifacts."""
    lines: list[str] = []

    def append(node: Any, depth: int) -> None:
        role = node_role(node) or "unknown"
        name = node_name(node)
        actions = action_names(node)
        states = state_names(node)
        description = node_description(node)
        # The description is printed beside the name because on GTK it is the only place the
        # supplementary half of a spoken label can live; see `node_description`.
        suffix = f' description="{description}"' if description else ""
        suffix += f" actions={','.join(actions)}" if actions else ""
        suffix += f" states={','.join(states)}" if states else ""
        lines.append(f'{"  " * depth}{role} name="{name}"{suffix}')
        for child in children(node):
            append(child, depth + 1)

    append(root, 0)
    return "\n".join(lines)


def pick_application(applications: list[Any]) -> Any | None:
    """Choose the Allodia Mail & Calendar application from everything on the bus.

    An application that *names itself* wins over one that merely contains the name somewhere:
    on a real desktop the shell publishes our window title in its window list, so a plain
    subtree scan matches gnome-shell; a tree with no rows in it; before it reaches us. The
    subtree scan stays as the fallback, because a private test bus carries no shell.
    """
    for application in applications:
        if any(marker in node_name(application).casefold() for marker in APPLICATION_MARKERS):
            return application
    for application in applications:
        names = (node_name(candidate).casefold() for candidate in walk(application))
        if any(marker in name for name in names for marker in APPLICATION_MARKERS):
            return application
    return None


def desktop_application() -> Any | None:
    """Find the Allodia Mail & Calendar application among those on the bus."""
    import pyatspi

    return pick_application(list(children(pyatspi.Registry.getDesktop(0))))


def wait_for_application(timeout: float) -> Any:
    """Wait for GTK to publish the application on AT-SPI."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        application = desktop_application()
        if application is not None:
            return application
        time.sleep(0.1)
    raise TimeoutError("Allodia Mail & Calendar did not appear on the AT-SPI bus")


def wait_for_no_application(timeout: float) -> None:
    """Wait until no client is on the accessibility bus at all.

    The barrier a relaunch needs. An app that has not left the bus is still the tree the driver
    reads, so the next launch's assertions would run against the *previous* instance; which is a
    pass or a fail decided by whichever process answered first.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if desktop_application() is None:
            return
        time.sleep(0.1)
    raise TimeoutError("Allodia Mail & Calendar is still on the AT-SPI bus")


def wait_for_nodes(
    root_provider: Callable[[], Any],
    *,
    name: str | None,
    role: str | None,
    within: str | None,
    within_role: str | None,
    contains_name: str | None,
    name_substring: str | None,
    description: str | None,
    enabled_only: bool,
    showing_only: bool,
    timeout: float,
    result_limit: int | None = None,
) -> list[Any]:
    """Poll a live remote tree until the requested accessible node exists and is enabled."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        matches = find_nodes(
            root_provider(),
            name=name,
            role=role,
            within=within,
            within_role=within_role,
            contains_name=contains_name,
            name_substring=name_substring,
            description=description,
            limit=result_limit,
        )
        eligible = [node for node in matches if node_enabled(node)] if enabled_only else matches
        if showing_only:
            eligible = [node for node in eligible if node_showing(node)]
        if eligible:
            return eligible
        time.sleep(0.1)
    target = name or role or "node"
    scope = f' within "{within}"' if within else ""
    outcome = "become enabled" if enabled_only else "appear"
    raise TimeoutError(f'accessibility target {target!r}{scope} did not {outcome}')


def wait_for_absence(
    root_provider: Callable[[], Any],
    *,
    name: str | None,
    role: str | None,
    within: str | None,
    within_role: str | None,
    contains_name: str | None,
    name_substring: str | None,
    description: str | None,
    timeout: float,
) -> None:
    """Poll a live remote tree until no matching accessible node remains."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not find_nodes(
            root_provider(),
            name=name,
            role=role,
            within=within,
            within_role=within_role,
            contains_name=contains_name,
            name_substring=name_substring,
            description=description,
            limit=1,
        ):
            return
        time.sleep(0.1)
    target = name or role or "node"
    scope = f' within "{within}"' if within else ""
    raise TimeoutError(f'accessibility target {target!r}{scope} did not disappear')


def parser() -> argparse.ArgumentParser:
    """Build the command-line parser used by control.sh and the headless acceptance test."""
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    dump = commands.add_parser("dump", help="print the live accessibility tree")
    dump.add_argument("--timeout", type=float, default=20.0)
    gone = commands.add_parser("gone", help="wait until no client is on the accessibility bus")
    gone.add_argument("--timeout", type=float, default=20.0)
    for command in ("wait", "activate", "measure", "set-text", "read-text"):
        target = commands.add_parser(command, help=f"{command} an exact accessible target")
        target.add_argument("--name")
        target.add_argument("--role")
        target.add_argument("--within")
        target.add_argument("--within-role")
        target.add_argument("--contains-name")
        target.add_argument("--name-substring")
        target.add_argument("--description")
        target.add_argument("--index", type=int, default=0)
        target.add_argument("--timeout", type=float, default=20.0)
        target.add_argument("--enabled", action="store_true")
        target.add_argument("--showing", action="store_true")
        target.add_argument("--absent", action="store_true")
        if command == "set-text":
            target.add_argument("--text", required=True)
        if command == "measure":
            target.add_argument("--until-name")
            target.add_argument("--until-role")
            target.add_argument("--until-within")
            target.add_argument("--until-within-role")
            target.add_argument("--until-absent", action="store_true")
            target.add_argument("--until-showing", action="store_true")
    return result


def main(argv: list[str] | None = None) -> int:
    """Run one AT-SPI inspection or activation command."""
    args = parser().parse_args(argv)
    try:
        if args.command == "gone":
            wait_for_no_application(args.timeout)
            print("gone")
            return 0
        root = wait_for_application(args.timeout)
        if args.command == "dump":
            print(render_tree(root))
            return 0
        if all(
            value is None
            for value in (args.name, args.role, args.name_substring, args.description)
        ):
            raise ValueError(
                "the command needs --name, --role, --name-substring, --description, or a mix"
            )
        if args.absent:
            if args.command != "wait":
                raise ValueError("--absent is valid only with wait")
            wait_for_absence(
                lambda: wait_for_application(args.timeout),
                name=args.name,
                role=args.role,
                within=args.within,
                within_role=args.within_role,
                contains_name=args.contains_name,
                name_substring=args.name_substring,
                description=args.description,
                timeout=args.timeout,
            )
            print(f'absent name="{args.name or args.name_substring or ""}" role="{args.role or ""}"')
            return 0
        # A text command names a *field*, and a labelled field is several nodes sharing one
        # name: libadwaita's `AdwEntryRow` draws its title as a label beside the entry, and the
        # label comes first in the tree. Asking for one match would therefore resolve "type
        # into the field called X" to a label, which can never accept text. So scan a few and
        # take the first that actually has the interface. A *non-zero* --index still names the
        # nth match exactly, for the caller who has already looked at the tree; `--index 0` is
        # the default, so it cannot be told apart from asking for no index at all.
        scanning = args.command in ("set-text", "read-text") and args.index == 0
        matches = wait_for_nodes(
            lambda: wait_for_application(args.timeout),
            name=args.name,
            role=args.role,
            within=args.within,
            within_role=args.within_role,
            contains_name=args.contains_name,
            name_substring=args.name_substring,
            description=args.description,
            enabled_only=args.command in ("activate", "measure") or args.enabled,
            showing_only=args.command in ("activate", "measure") or args.showing,
            timeout=args.timeout,
            result_limit=TEXT_SCAN if scanning else args.index + 1,
        )
        if args.index < 0 or args.index >= len(matches):
            raise IndexError(f"target index {args.index} is outside {len(matches)} matches")
        target = matches[args.index]
        if args.command == "read-text":
            candidates = [target]
            if scanning:
                fields = [node for node in matches if node_editable(node)]
                candidates = fields or matches
            for candidate in candidates:
                value = read_node_text(candidate)
                if value is not None:
                    print(value, end="")
                    return 0
            raise RuntimeError("target has no readable text interface")
        if args.command == "set-text":
            candidates = matches if scanning else [target]
            for candidate in candidates:
                if set_node_text(candidate, args.text):
                    print(f'{node_role(candidate)} name="{node_name(candidate)}"')
                    return 0
            raise RuntimeError("target has no enabled editable-text interface")
        print(
            f'{node_role(target)} name="{node_name(target)}"'
            f'{f" description={node_description(target)!r}" if node_description(target) else ""}'
        )
        if args.command == "measure":
            started = time.monotonic()
            if not activate_node(target):
                raise RuntimeError("target has no enabled activate/click/press/open action")
            if args.until_name is None and args.until_role is None:
                raise ValueError("measure needs --until-name, --until-role, or both")
            if args.until_absent:
                wait_for_absence(
                    lambda: root,
                    name=args.until_name,
                    role=args.until_role,
                    within=args.until_within,
                    within_role=args.until_within_role,
                    contains_name=None,
                    name_substring=None,
                    description=None,
                    timeout=args.timeout,
                )
            else:
                wait_for_nodes(
                    lambda: root,
                    name=args.until_name,
                    role=args.until_role,
                    within=args.until_within,
                    within_role=args.until_within_role,
                    contains_name=None,
                    name_substring=None,
                    description=None,
                    enabled_only=False,
                    showing_only=args.until_showing,
                    timeout=args.timeout,
                    result_limit=1,
                )
            elapsed_ms = round((time.monotonic() - started) * 1000)
            print(f"elapsed_ms={elapsed_ms}")
            return 0
        if args.command == "activate" and not activate_node(target):
            raise RuntimeError("target has no enabled activate/click/press/open action")
        return 0
    except (IndexError, RuntimeError, TimeoutError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
