#!/usr/bin/env python3
"""Unit tests for the Linux AT-SPI tree-selection helpers."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import linux_ui_atspi as subject


class FakeAction:
    def __init__(self, names: list[str]) -> None:
        self.names = names
        self.called: list[int] = []

    @property
    def nActions(self) -> int:  # noqa: N802 - mirrors pyatspi
        return len(self.names)

    def getName(self, index: int) -> str:  # noqa: N802 - mirrors pyatspi
        return self.names[index]

    def doAction(self, index: int) -> bool:  # noqa: N802 - mirrors pyatspi
        self.called.append(index)
        return True


class FakeEditableText:
    def __init__(self) -> None:
        self.value = ""

    def setTextContents(self, value: str) -> bool:  # noqa: N802 - mirrors pyatspi
        self.value = value
        return True


class FakeText:
    def __init__(self, value: str) -> None:
        self.value = value

    @property
    def characterCount(self) -> int:  # noqa: N802 - mirrors pyatspi
        return len(self.value)

    def getText(self, start: int, end: int) -> str:  # noqa: N802 - mirrors pyatspi
        return self.value[start:end]


class FakeNode:
    def __init__(
        self,
        name: str,
        role: str,
        *children: "FakeNode",
        actions: list[str] | None = None,
        enabled: bool = True,
        description: str = "",
        text: str = "",
    ) -> None:
        self.name = name
        self.role = role
        self.description = description
        self.children = list(children)
        self.action = FakeAction(actions or [])
        self.editable = FakeEditableText()
        self.text = FakeText(text)
        self.enabled = enabled

    @property
    def childCount(self) -> int:  # noqa: N802 - mirrors pyatspi
        return len(self.children)

    def getChildAtIndex(self, index: int) -> "FakeNode":  # noqa: N802
        return self.children[index]

    def getRoleName(self) -> str:  # noqa: N802 - mirrors pyatspi
        return self.role

    def queryAction(self) -> FakeAction:  # noqa: N802 - mirrors pyatspi
        return self.action

    def queryEditableText(self) -> FakeEditableText:  # noqa: N802 - mirrors pyatspi
        return self.editable

    def queryText(self) -> FakeText:  # noqa: N802 - mirrors pyatspi
        return self.text


class CountingNode(FakeNode):
    def __init__(self, name: str, role: str, *children: FakeNode) -> None:
        super().__init__(name, role, *children)
        self.requested_children: list[int] = []

    def getChildAtIndex(self, index: int) -> FakeNode:  # noqa: N802
        self.requested_children.append(index)
        return super().getChildAtIndex(index)


class TreeSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.first = FakeNode("First subject", "list item", actions=["activate"])
        self.second = FakeNode("Second subject", "list item", actions=["activate"])
        self.messages = FakeNode("Message list", "list", self.first, self.second)
        self.reply = FakeNode("Reply", "push button", actions=["press"])
        self.root = FakeNode("Allodia Mail & Calendar", "application", self.messages, self.reply)

    def test_find_is_exact_and_case_insensitive(self) -> None:
        self.assertEqual(subject.find_nodes(self.root, name="reply"), [self.reply])
        self.assertEqual(subject.find_nodes(self.root, name="Rep"), [])

    def test_limited_find_stops_before_walking_the_rest_of_a_large_tree(self) -> None:
        banner = FakeNode("Message sent", "alert")
        root = CountingNode(
            "Allodia Mail & Calendar",
            "application",
            banner,
            FakeNode("Large message list", "list", *[FakeNode(str(i), "list item") for i in range(200)]),
        )

        self.assertEqual(subject.find_nodes(root, name="Message sent", limit=1), [banner])
        self.assertEqual(root.requested_children, [0])

    def test_find_can_scope_a_role_to_a_named_ancestor(self) -> None:
        self.assertEqual(
            subject.find_nodes(
                self.root,
                role="list item",
                within="message list",
            ),
            [self.first, self.second],
        )

    def test_find_can_match_a_descendants_name_on_an_actionable_ancestor(self) -> None:
        self.assertEqual(
            subject.find_nodes(
                self.root,
                role="list item",
                within="message list",
                contains_name="second subject",
            ),
            [self.second],
        )

    def test_activate_prefers_semantic_actions(self) -> None:
        self.assertTrue(subject.activate_node(self.reply))
        self.assertEqual(self.reply.action.called, [0])

    def test_activate_rejects_a_disabled_node(self) -> None:
        disabled = FakeNode("Send", "push button", actions=["press"], enabled=False)
        self.assertFalse(subject.activate_node(disabled))
        self.assertEqual(disabled.action.called, [])

    def test_libadwaita_switch_role_and_action_are_normalized(self) -> None:
        switch = FakeNode("Share usage statistics", "last defined", actions=["toggle"])

        self.assertEqual(subject.node_role(switch), "switch")
        self.assertTrue(subject.activate_node(switch))
        self.assertEqual(switch.action.called, [0])

    def test_a_non_actionable_last_defined_container_stays_distinct(self) -> None:
        row = FakeNode("Share usage statistics", "last defined")
        self.assertEqual(subject.node_role(row), "last defined")

    def test_set_text_uses_the_semantic_editable_text_interface(self) -> None:
        title = FakeNode("Title", "text")
        self.assertTrue(subject.set_node_text(title, "Linux acceptance event"))
        self.assertEqual(title.editable.value, "Linux acceptance event")

    def test_read_text_uses_the_semantic_text_interface(self) -> None:
        snippet = FakeNode("", "text", text='{"mcpServers": {}}')
        self.assertEqual(subject.read_node_text(snippet), '{"mcpServers": {}}')

    def test_dump_records_role_name_and_actions(self) -> None:
        rendered = subject.render_tree(self.root)
        self.assertIn('application name="Allodia Mail & Calendar"', rendered)
        self.assertIn('push button name="Reply" actions=press', rendered)

    def test_the_app_wins_over_a_shell_that_merely_shows_its_window_title(self) -> None:
        # On a live desktop the shell publishes our window title in its window list, so a plain
        # subtree scan picks gnome-shell; a tree with none of our rows in it.
        shell = FakeNode(
            "gnome-shell",
            "application",
            FakeNode("Allodia Mail & Calendar", "push button", actions=["press"]),
        )
        app = FakeNode("mailcal-linux", "application", self.messages)
        self.assertIs(subject.pick_application([shell, app]), app)

    def test_an_app_that_only_contains_the_name_still_matches(self) -> None:
        # A private test bus carries no shell, and the process there may not name itself.
        other = FakeNode("gsd-power", "application")
        app = FakeNode("", "application", FakeNode("Allodia Mail & Calendar", "frame"))
        self.assertIs(subject.pick_application([other, app]), app)

    def test_within_can_pick_its_ancestor_by_role(self) -> None:
        # The trap this exists for: a window and the button that opens it share a title, and walk
        # order reaches the button first; so scoping by name alone searches the wrong subtree.
        button = FakeNode("New signature", "push button")
        window = FakeNode("New signature", "frame", FakeNode("Save", "push button"))
        root = FakeNode("", "application", button, window)
        self.assertEqual(
            subject.find_nodes(root, name="Save", within="New signature"),
            [],
        )
        self.assertEqual(
            len(subject.find_nodes(root, name="Save", within="New signature", within_role="frame")),
            1,
        )

    def test_wait_for_absence_accepts_an_already_missing_target(self) -> None:
        subject.wait_for_absence(
            lambda: self.root,
            name="Missing banner",
            role=None,
            within=None,
            within_role=None,
            contains_name=None,
            name_substring=None,
            description=None,
            timeout=0.01,
        )

    def test_measure_command_keeps_action_and_outcome_targets_separate(self) -> None:
        args = subject.parser().parse_args(
            [
                "measure",
                "--name",
                "Archive",
                "--role",
                "push button",
                "--until-name",
                "Visible message",
                "--until-role",
                "list item",
                "--until-within",
                "Message list",
                "--until-absent",
            ]
        )
        self.assertEqual(args.name, "Archive")
        self.assertEqual(args.until_name, "Visible message")
        self.assertEqual(args.until_within, "Message list")
        self.assertTrue(args.until_absent)

    def test_a_substring_matches_a_label_the_client_composed_at_runtime(self) -> None:
        # A calendar block speaks its title, its time range, its calendar and; on an unanswered
        # invitation; the hold disclosure. The disclosure is what docs/calendar.md §4 binds; the
        # rest is the fixture's own words and a date, so an exact match would pin the fixture.
        spoken = FakeNode("Quarterly planning, 10:30-11:30, Work, Awaiting your response", "label")
        root = FakeNode("Allodia Mail & Calendar", "application", spoken)
        self.assertEqual(
            subject.find_nodes(root, name_substring="awaiting your response"), [spoken]
        )
        self.assertEqual(subject.find_nodes(root, name_substring="Declined"), [])

    def test_a_description_is_matched_and_is_where_a_labelled_control_says_more(self) -> None:
        # A GtkButton with a label is `labelled-by` that label, and a relation beats an explicit
        # accessible label; so what an answer button acts on can only live in the description.
        # A tree that read names alone would pass a button announcing a bare "Accept".
        accept = FakeNode("Accept", "push button", description="Accept this invitation")
        root = FakeNode("Allodia Mail & Calendar", "application", accept)
        self.assertEqual(
            subject.find_nodes(root, name="Accept", description="Accept this invitation"),
            [accept],
        )
        self.assertEqual(subject.find_nodes(root, name="Accept", description="Decline"), [])
        self.assertIn('description="Accept this invitation"', subject.render_tree(root))

    def test_a_substring_and_a_role_both_have_to_hold(self) -> None:
        spoken = FakeNode("Weekend walk, Awaiting your response", "push button")
        root = FakeNode("Allodia Mail & Calendar", "application", spoken)
        self.assertEqual(
            subject.find_nodes(root, role="push button", name_substring="Awaiting"), [spoken]
        )
        self.assertEqual(subject.find_nodes(root, role="label", name_substring="Awaiting"), [])


if __name__ == "__main__":
    unittest.main()
