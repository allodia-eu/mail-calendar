#!/usr/bin/env bash
# The Linux UI suite's calendar legs: the agenda, managing calendars, the meeting server, events,
# meetings, answering from the calendar and the invitation card. Sourced by test-linux-ui.sh.

# The calendar screen. A destination toggle that is already on stays on, so this is safe to repeat.
show_calendar() {
  ui_press text=Calendar role=toggle-button
  ui_wait text="New Event" role=button enabled=true timeoutMs=45000
}

# An agenda event's details. A saved event replaces its agenda row once the core snapshot arrives,
# and the agent can find the outgoing button just before that replacement, so the press is retried
# against the settled row until the details are on screen.
#
# ⚠️ **The press is inside the retry, not above it.** The row can be replaced between being found
# and being pressed, and the press then fails on a handle that is gone: with it above the loop,
# `set -e` ends the run on the first attempt and the retry never does the one thing it exists for.
open_calendar_event() { # <title>
  for _ in {1..5}; do
    if UI_QUIET=1 ui_press text="$1" role=button &&
      UI_QUIET=1 ui_wait text=Edit role=button window="$1" enabled=true timeoutMs=5000; then
      return
    fi
    sleep 1
  done
  die "calendar event details did not open"
}

# Calendar: the same process changes to the deterministic living-week calendar.
#
# This one switch is driven by a **real pointer**, and it is the suite's only guard on pointer input
# working at all. That input is fragile in a way nothing else here would notice: on a seat with no
# physical pointer it works only while one virtual pointer stays open for the whole gesture, so a
# regression in the tooling would silently leave the Linux client with no way to drive a gesture
# (crates/mailcal-vpointer). No stored coordinate: the agent measures the tab where it is drawn.
#
# `New Event` is what makes this a real assertion rather than a hopeful one. It exists only on the
# calendar, so it cannot be satisfied by the click having missed and the mail view still being up,
# which is how a pointer check quietly becomes a no-op.
leg_calendar() {
  ui_wait text=Calendar role=toggle-button
  local point
  point="$(ui_centre)"
  # shellcheck disable=SC2086
  linux_session_pointer click $point
  ui_wait text="New Event" role=button enabled=true timeoutMs=45000
  # AT-SPI: the seeded NEEDS-ACTION hold, on the surface with no border to dash. An agenda row
  # prints the disclosure instead and says it too (docs/calendar.md §4; the picture is per surface,
  # the spoken label is not), and the spoken half is a screen reader's to check.
  atspi wait --name-substring "$AWAITING_LABEL" --showing --timeout 45
  capture calendar-agenda

  # Calendar management is one round trip through the same cache the agenda draws. Toggle the
  # calendar away and back, apply a palette colour, reset it, then reach the default-calendar picker
  # through the shared Settings taxonomy.
  ui_press text="Manage calendars" role=button
  ui_wait type=GtkWindow text="Manage calendars"
  ui_wait text=alice@test.local window="Manage calendars"
  ui_press type=GtkSwitch window="Manage calendars" index=0
  ui_press type=GtkSwitch window="Manage calendars" index=0
  ui_press type=GtkMenuButton window="Manage calendars" index=0
  ui_press id=calendar-colour-4f5ba6
  eventually grep -Fq '#4f5ba6' "$CORE_PREFERENCES" || die "calendar colour was not persisted"
  ui_press text="Use the server's colour" role=button
  ui_press text=Done role=button window="Manage calendars"

  open_settings Calendar
  ui_wait text="Default calendar"
  # AT-SPI: the seed gives Alice two writable calendars, and both are offered. Grouped check buttons
  # speak as radio buttons, a lone one as a check box, and the role is what is asked.
  for calendar in "Stalwart Calendar (alice@test.local)" "Work"; do
    atspi wait --name "$calendar" --role "radio button" \
      --within "Settings" --within-role frame --showing --timeout 20
  done
  close_settings
  capture calendar-management
}

# Settings → Videoconferencing (docs/conferencing.md, rule 13). The field commits on Return and on
# losing focus, never per keystroke: an address the core refuses is said and stored nowhere, and a
# stored one is shown as the core keeps it, which is what makes the editor offer Jitsi.
leg_jitsi() {
  open_settings Videoconferencing
  ui_one text="Jitsi server" type=AdwEntryRow
  local field="$UI_H"
  # Focused first, as a person clicking into it would: the row offers its apply button only then.
  ui focus h="$field"
  ui set-text h="$field" text=meet.example.org activate=true
  ui_wait textLike="starts with https://" role=label
  grep -q '^jitsi_server' "$CORE_PREFERENCES" && die "a refused Jitsi server was stored"
  ui set-text h="$field" text=https://meet.example.org/ activate=true
  eventually grep -q '^jitsi_server = "https://meet.example.org"$' "$CORE_PREFERENCES" ||
    die "the Jitsi server was not stored"
  ui_wait text="Jitsi server" type=AdwEntryRow
  [[ "$(ui_value)" == "https://meet.example.org" ]] ||
    die "the Jitsi field does not show the stored address"
  capture settings-videoconferencing
  close_settings
}

# An event created, changed and deleted through the editor and the detail. The server the jitsi leg
# stored is what makes the editor offer a meeting; a new event has none.
leg_events() {
  show_calendar
  ui_press text="New Event" role=button
  ui_fill id=event-title value="$CALENDAR_CRUD_TITLE"
  ui_wait type=GtkDropDown has='{"text":"None"}' window="New event"
  ui_press text=Save role=button window="New event"
  ui_wait text="$CALENDAR_CRUD_TITLE" role=button enabled=true timeoutMs=45000
  capture calendar-created

  open_calendar_event "$CALENDAR_CRUD_TITLE"
  ui_press text=Edit role=button window="$CALENDAR_CRUD_TITLE"
  ui_fill id=event-title value="$CALENDAR_CRUD_TITLE updated"
  ui_press text=Save role=button window="Edit event"
  ui_wait text="$CALENDAR_CRUD_TITLE updated" role=button enabled=true timeoutMs=45000
  capture calendar-edited

  delete_calendar_event "$CALENDAR_CRUD_TITLE updated"
  capture calendar-deleted
}

delete_calendar_event() { # <title>
  open_calendar_event "$1"
  ui_press text=Delete role=button window="$1"
  ui_wait text="Delete this event?"
  ui_press text=Delete role=button window="Delete Event"
  ui_gone text="$1" role=button timeoutMs=45000
}

# A meeting, end to end through the organiser's editor (docs/calendar.md, "Organising a meeting").
# The harness calendar is JMAP, whose server sends the invitations, so the list is offered; the
# assertions are on Bob's and Dave's own mailboxes, because a meeting stored on Alice's calendar and
# sent to nobody looks exactly like a working one from her side.
leg_meeting() {
  show_calendar
  ui_press text="New Event" role=button
  ui_fill id=event-title value="$MEETING_TITLE"
  ui_wait text=Attendees
  for invitee in bob@test.local dave@test.local; do
    ui_fill id=event-invitee value="$invitee"
    ui_press text=Invite role=button
    ui_wait text="Remove $invitee" role=button
  done
  # Alice's own address adds nobody, and says why, rather than being dropped in silence.
  ui_fill id=event-invitee value=alice@test.local
  ui_press text=Invite role=button
  ui_wait text="That’s you: you attend as the organiser."
  ui_fill id=event-invitee value=""
  capture calendar-meeting-editor
  ui_press text=Save role=button window="New event"
  ui_wait text="$MEETING_TITLE" role=button enabled=true timeoutMs=45000
  wait_for_scheduling_mail bob@test.local "$MEETING_TITLE"
  wait_for_scheduling_mail dave@test.local "$MEETING_TITLE"

  # The organiser's edit: the roster opens with both, and taking Dave off cancels his invitation.
  open_calendar_event "$MEETING_TITLE"
  ui_wait textLike=Organiser role=list-item
  ui_press text=Edit role=button window="$MEETING_TITLE"
  ui_press text="Remove dave@test.local" role=button
  ui_gone text="Remove dave@test.local" role=button
  ui_press text=Save role=button window="Edit event"
  wait_for_scheduling_mail dave@test.local "Cancelled: $MEETING_TITLE"

  # Deleting the meeting tells whoever is still on it.
  delete_calendar_event "$MEETING_TITLE"
  wait_for_scheduling_mail bob@test.local "Cancelled: $MEETING_TITLE"
}

# An invitee answers from the event in her calendar, not only from the invitation email
# (docs/calendar.md, "Answering from the calendar"). Bob's own mailbox is the assertion: the answer
# a calendar stores and tells nobody about looks the same from Alice's side. The buttons' spoken
# descriptions are the invitations leg's to assert; here they only act.
leg_answer() {
  # Bob's invitation reaches Alice's calendar once the harness's scheduler has delivered it, which
  # can be after the calendar was last read; showing the calendar again reads it again.
  for _ in {1..6}; do
    ui_press text=Mail role=toggle-button
    show_calendar
    UI_QUIET=1 ui_wait text="$ANSWER_TITLE" role=button timeoutMs=5000 && break
  done
  open_calendar_event "$ANSWER_TITLE"
  ui_wait text="You haven't answered yet"
  ui_press text=Maybe role=button window="$ANSWER_TITLE"
  wait_for_scheduling_mail bob@test.local "Tentative: $ANSWER_TITLE"
  # The detail stays open and says what the server now holds.
  ui_wait text="You answered tentatively" timeoutMs=45000
  # A decline asks first, because the meeting then leaves the calendar, and the detail with it.
  ui_press text=Decline role=button window="$ANSWER_TITLE" enabled=true
  ui_one type=AdwAlertDialog text="Decline this meeting?"
  ui_press under="$UI_H" text=Decline role=button
  wait_for_scheduling_mail bob@test.local "Declined: $ANSWER_TITLE"
  ui_gone text="$ANSWER_TITLE" role=button timeoutMs=45000
}

# Meeting invitations. This leg runs AFTER the calendar leg on purpose: the card withholds its day
# preview until the calendar has actually been read (`conflicts_known`), and "we have not looked" is
# a different fact from "nothing else then"; so a run that asked before the agenda had events would
# assert the cold-start card and never see the rule. The agenda proves the read happened.
leg_invitations() {
  open_mail_message "$INVITE_SUBJECT"
  ui_wait text="Meeting invitation" timeoutMs=45000
  # AT-SPI: the two-condition gate passing, as three buttons that each say what they act on; three
  # bare verbs read out of context tell a screen-reader user nothing about which invitation they
  # belong to (docs/invitations.md). The qualifier is the DESCRIPTION and it has to be: a GtkButton
  # with a label is `labelled-by` that label, and a relation beats an explicit accessible label, so
  # an assertion on the name alone would pass over a button announcing a bare "Accept".
  atspi wait --name "Accept" --role "push button" \
    --description "Accept this invitation" --enabled --showing --timeout 20
  atspi wait --name "Maybe" --role "push button" \
    --description "Answer maybe to this invitation" --enabled --showing --timeout 20
  atspi wait --name "Decline" --role "push button" \
    --description "Decline this invitation" --enabled --showing --timeout 20
  # The iMIP body part is not a file: a card and NO attachment row. The published .ics below is the
  # other half of that contrast.
  ui_gone text=Attachments
  # The count in words, then the picture; docs/calendar.md §4: a grid the user has to read carefully
  # is not a disclosure. The Monday invitation lands in the review/triage overlap.
  ui_wait text="Around this meeting"
  # AT-SPI: the meeting itself, drawn on that preview as a hold and SAYING so. A dashed border is
  # invisible to a screen reader, and GTK exposes no getter for an accessible label (AGENTS.md).
  atspi wait --name-substring "$AWAITING_LABEL" --showing --timeout 20
  capture invitation-card

  # The published .ics, and the contrast the seed exists for: same media type, dispositioned as a
  # file, METHOD:PUBLISH, no ATTENDEE. No card, and the chip stays. Attendee-matching alone is not
  # the gate; nobody is waiting on a reply to this one.
  open_mail_message "$PUBLISH_SUBJECT"
  ui_wait text=Attachments timeoutMs=45000
  ui_gone text="Meeting invitation"
  capture invitation-published

  # The weekend invitation is the fixture that makes the preview rule able to FAIL. Its day is
  # deliberately empty, so under the rule this replaced ("open only when the count is non-zero") the
  # disclosure would be shut; a suite that only ever saw the conflicted Monday would pass under
  # both. Whenever the calendar was read, the grid is open.
  open_mail_message "$FREE_DAY_SUBJECT"
  ui_wait text="Meeting invitation" timeoutMs=45000
  ui_wait text="Around this meeting"
  # AT-SPI, for the reason above.
  atspi wait --name-substring "$AWAITING_LABEL" --showing --timeout 20
  capture invitation-free-day
  search_for ""
}
