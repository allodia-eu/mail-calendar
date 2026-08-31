use std::collections::BTreeSet;

use engine_api::{
    CalendarDateTime, CalendarId, Event, EventId, LocalDateTime, Memberships, Participant,
    ParticipantRole, ParticipationStatus, Uid,
};

use super::{EventAttendee, effective_response, event_attendees};
use crate::ResponseStatus;

fn event_with(participants: Vec<Participant>) -> Event {
    let mut event = Event::new(
        EventId::try_from("/cal/e.ics").unwrap(),
        Uid::new("e@h").unwrap(),
        Memberships::of_one(CalendarId::try_from("work").unwrap()),
        CalendarDateTime::Floating(LocalDateTime::new(2026, 5, 1, 9, 0, 0).unwrap()),
    );
    event.participants = participants;
    event
}

/// An `ATTENDEE` line: an address, a status, and optionally a name.
fn attendee(email: &str, status: ParticipationStatus) -> Participant {
    let mut participant = Participant::attendee(email);
    participant.participation_status = status;
    participant
}

/// An `ORGANIZER` line: the `owner` role, and by default no `PARTSTAT` at all.
fn organizer(email: &str) -> Participant {
    let mut participant = Participant::attendee(email);
    participant.roles = BTreeSet::from([ParticipantRole::Owner]);
    participant
}

fn named(mut participant: Participant, name: &str) -> Participant {
    participant.name = Some(name.to_owned());
    participant
}

#[test]
fn an_attendees_name_address_and_answer_all_reach_the_row() {
    let event = event_with(vec![named(
        attendee("Anna@Example.COM", ParticipationStatus::Tentative),
        "Anna Jansen",
    )]);
    assert_eq!(
        event_attendees(&event),
        vec![EventAttendee {
            name: "Anna Jansen".to_owned(),
            email: "anna@example.com".to_owned(),
            is_organizer: false,
            response: ResponseStatus::Tentative,
        }],
        "the address is normalized for display, the name is carried verbatim"
    );
}

#[test]
fn an_unnamed_attendee_has_an_empty_name_rather_than_a_fabricated_one() {
    let event = event_with(vec![attendee(
        "b@example.com",
        ParticipationStatus::Accepted,
    )]);
    let rows = event_attendees(&event);
    assert_eq!(rows[0].name, "", "the client falls back to the address");
    assert_eq!(rows[0].email, "b@example.com");
}

#[test]
fn a_whitespace_only_name_counts_as_no_name() {
    let event = event_with(vec![named(
        attendee("b@example.com", ParticipationStatus::Accepted),
        "   ",
    )]);
    assert_eq!(event_attendees(&event)[0].name, "");
}

#[test]
fn the_organizer_sorts_first_and_everyone_else_keeps_the_events_order() {
    let event = event_with(vec![
        attendee("c@example.com", ParticipationStatus::Accepted),
        attendee("b@example.com", ParticipationStatus::Declined),
        organizer("a@example.com"),
    ]);
    let addresses: Vec<_> = event_attendees(&event)
        .into_iter()
        .map(|row| row.email)
        .collect();
    assert_eq!(
        addresses,
        ["a@example.com", "c@example.com", "b@example.com"]
    );
}

#[test]
fn an_organizer_with_no_answer_has_accepted_their_own_meeting() {
    // RFC 5546 §3.2.1: the same rule the invitation tally and the grid apply. Read literally,
    // the absent `PARTSTAT` would report the person who called the meeting as not having replied.
    let event = event_with(vec![organizer("a@example.com")]);
    let rows = event_attendees(&event);
    assert!(rows[0].is_organizer);
    assert_eq!(rows[0].response, ResponseStatus::Accepted);
}

#[test]
fn an_organizer_who_declined_their_own_meeting_keeps_that_answer() {
    let mut owner = organizer("a@example.com");
    owner.participation_status = ParticipationStatus::Declined;
    let rows = event_attendees(&event_with(vec![owner]));
    assert_eq!(rows[0].response, ResponseStatus::Declined);
}

#[test]
fn a_vendor_status_we_do_not_understand_reads_as_unanswered() {
    let event = event_with(vec![attendee(
        "b@example.com",
        ParticipationStatus::from_wire("snoozed"),
    )]);
    assert_eq!(
        event_attendees(&event)[0].response,
        ResponseStatus::NeedsAction
    );
}

#[test]
fn the_split_organizer_shape_is_one_row_not_two() {
    // A plain iCalendar server writes `ORGANIZER` and a matching `ATTENDEE` as two lines that
    // decode to two participants; JSCalendar merges them into one. Listing them verbatim would
    // print the organiser twice, and only on the servers that split them.
    let event = event_with(vec![
        organizer("a@example.com"),
        named(
            attendee("mailto:A@Example.com", ParticipationStatus::Accepted),
            "Anna",
        ),
        attendee("b@example.com", ParticipationStatus::NeedsAction),
    ]);
    let rows = event_attendees(&event);
    assert_eq!(rows.len(), 2, "one row per address: {rows:?}");
    assert_eq!(rows[0].email, "a@example.com");
    assert!(rows[0].is_organizer, "the ORGANIZER line's role sticks");
    assert_eq!(
        rows[0].name, "Anna",
        "the ATTENDEE line's name fills the gap"
    );
    assert_eq!(
        rows[0].response,
        ResponseStatus::Accepted,
        "the explicit PARTSTAT on the ATTENDEE line is the answer"
    );
}

#[test]
fn a_bare_organizer_line_cannot_re_accept_a_meeting_its_attendee_line_declined() {
    // The guard on the merge: the ORGANIZER line has no `PARTSTAT`, so `effective_response`
    // infers `Accepted` for it. Testing the *mapped* status would let that inference overwrite a
    // real `DECLINED`; silently re-accepting a meeting the user declined.
    let event = event_with(vec![
        attendee("a@example.com", ParticipationStatus::Declined),
        organizer("a@example.com"),
    ]);
    let rows = event_attendees(&event);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_organizer);
    assert_eq!(rows[0].response, ResponseStatus::Declined);
}

/// A `ROLE=CHAIR` attendee; what a JMAP server decodes an `ORGANIZER` line into.
fn chair(email: &str, status: ParticipationStatus) -> Participant {
    let mut participant = attendee(email, status);
    participant.roles = BTreeSet::from([ParticipantRole::Chair]);
    participant
}

#[test]
fn on_a_jmap_meeting_with_no_owner_the_chair_is_the_organizer() {
    // Measured against the harness: Stalwart decodes ORGANIZER into `chair` and the attendees into
    // `required`/`optional`, with no `owner` anywhere. Asking only for `owner` left every JMAP
    // meeting with nobody marked: the feature silently doing nothing on a whole class of account.
    let event = event_with(vec![
        attendee("guest@example.com", ParticipationStatus::NeedsAction),
        chair("alice@test.local", ParticipationStatus::Accepted),
    ]);
    let rows = event_attendees(&event);
    assert_eq!(rows[0].email, "alice@test.local", "and sorts first");
    assert!(rows[0].is_organizer);
    assert!(!rows[1].is_organizer);
}

#[test]
fn a_real_owner_wins_and_a_chair_beside_it_is_an_ordinary_attendee() {
    // The fallback must stay a fallback: a meeting that names both must not mark two people as
    // having called it.
    let event = event_with(vec![
        chair("chairperson@example.com", ParticipationStatus::Accepted),
        organizer("boss@example.com"),
    ]);
    let rows = event_attendees(&event);
    assert_eq!(rows[0].email, "boss@example.com");
    assert!(rows[0].is_organizer);
    assert!(
        !rows[1].is_organizer,
        "the chair is chairing, not organizing: {rows:?}"
    );
}

#[test]
fn a_chair_with_no_answer_is_marked_organizer_but_still_reads_as_unanswered() {
    // The two questions are deliberately not the same test. RFC 5546 §3.2.1 gives the *organiser*
    // attendance by definition; a CHAIR attendee line is an ordinary answer slot. So the mark says
    // who called it and the answer says what the data says; inferring here would invent an answer.
    let rows = event_attendees(&event_with(vec![chair(
        "alice@test.local",
        ParticipationStatus::NeedsAction,
    )]));
    assert!(rows[0].is_organizer);
    assert_eq!(rows[0].response, ResponseStatus::NeedsAction);
}

#[test]
fn a_participant_with_no_address_is_skipped_so_the_roster_matches_the_tally() {
    let mut ghost = Participant::attendee("x@example.com");
    ghost.email = None;
    ghost.name = Some("Meeting room".to_owned());
    let event = event_with(vec![
        ghost,
        attendee("b@example.com", ParticipationStatus::Accepted),
    ]);
    let rows = event_attendees(&event);
    assert_eq!(rows.len(), 1, "an ATTENDEE with no cal-address is nobody");
    assert_eq!(rows[0].email, "b@example.com");
}

#[test]
fn a_name_carrying_a_bidi_override_is_sanitized_before_it_reaches_a_label() {
    let event = event_with(vec![named(
        attendee("b@example.com", ParticipationStatus::Accepted),
        "Anna\u{202E}\r\nJansen",
    )]);
    assert_eq!(event_attendees(&event)[0].name, "Anna Jansen");
}

#[test]
fn an_event_nobody_was_invited_to_has_no_attendees() {
    assert!(event_attendees(&event_with(vec![])).is_empty());
}

#[test]
fn effective_response_maps_every_status_a_card_renders() {
    for (wire, expected) in [
        (ParticipationStatus::Accepted, ResponseStatus::Accepted),
        (ParticipationStatus::Declined, ResponseStatus::Declined),
        (ParticipationStatus::Tentative, ResponseStatus::Tentative),
        (ParticipationStatus::Delegated, ResponseStatus::Delegated),
        (
            ParticipationStatus::NeedsAction,
            ResponseStatus::NeedsAction,
        ),
    ] {
        assert_eq!(
            effective_response(&attendee("b@example.com", wire)),
            expected
        );
    }
}
