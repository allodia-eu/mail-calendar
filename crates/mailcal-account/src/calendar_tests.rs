//! Unit tests for the calendar-write builders ([`super::build_event_draft`] /
//! [`super::build_event_patch`]). Split into their own `#[path]` file to keep `calendar.rs`
//! under the 500-line limit.

use engine_core::{
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDate, CalendarDateTime, Duration, LocalDateTime, TimeZoneId, UtcDateTime},
    version::{ETag, RevisionTokens},
};

use super::*;

fn amsterdam(local: &str) -> CalendarDateTime {
    CalendarDateTime::Zoned {
        local: local.parse().unwrap(),
        zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
    }
}

fn stored_event() -> Event {
    let mut event = Event::new(
        EventId::try_from("/cal/standup.ics").unwrap(),
        Uid::new("standup@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        amsterdam("2026-01-05T09:30:00"),
    );
    event.title = "Standup".to_owned();
    event.duration = Duration::from_parts(0, 0, 0, 30, 0, 0).unwrap();
    event.revisions = RevisionTokens::from_etag(ETag::new("\"v7\""));
    event
}

fn now() -> UtcDateTime {
    UtcDateTime::new(2026, 2, 10, 11, 30, 0).unwrap()
}

fn local(text: &str) -> LocalDateTime {
    text.parse().unwrap()
}

#[test]
fn a_draft_carries_intent_but_no_document() {
    let draft = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "new-event@test.local",
        "Sprint planning",
        "2026-08-01T09:00:00Z",
        "2026-08-01T09:30:00Z",
        false,
        None,
        None,
        None,
        None,
        now(),
    )
    .unwrap();
    assert_eq!(draft.summary, "Sprint planning");
    assert_eq!(draft.uid.as_str(), "new-event@test.local");
    assert_eq!(draft.calendar.as_str(), "/cal/");
    assert!(matches!(draft.start, CalendarDateTime::Zoned { .. }));
    assert!(draft.description.is_none());
}

#[test]
fn a_timed_draft_with_a_zone_is_created_in_that_zone() {
    // The client passes a wall clock in the device's zone (not a UTC instant), so a
    // created event reads back the same clock the user typed: no UTC surprise on edit.
    let draft = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "zoned@test.local",
        "Lunch",
        "2026-08-01T12:30:00",
        "2026-08-01T13:00:00",
        false,
        Some("Europe/Amsterdam"),
        None,
        None,
        None,
        now(),
    )
    .unwrap();
    assert_eq!(draft.start, amsterdam("2026-08-01T12:30:00"));
    assert_eq!(draft.end, amsterdam("2026-08-01T13:00:00"));
}

#[test]
fn an_all_day_draft_is_a_zoneless_date_with_an_exclusive_end() {
    // A one-day all-day event on the 1st: start is the bare date, and the end is the
    // **exclusive** 2nd (RFC 5545): the client converts its inclusive on-screen end.
    let draft = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "holiday@test.local",
        "Vrij",
        "2026-08-01",
        "2026-08-02",
        true,
        None,
        None,
        None,
        None,
        now(),
    )
    .unwrap();
    assert_eq!(
        draft.start,
        CalendarDateTime::Date(CalendarDate::new(2026, 8, 1).unwrap())
    );
    assert_eq!(
        draft.end,
        CalendarDateTime::Date(CalendarDate::new(2026, 8, 2).unwrap())
    );
}

#[test]
fn notes_become_the_description_and_empty_notes_do_not() {
    let with_notes = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "n@test.local",
        "Sprint planning",
        "2026-08-01T09:00:00Z",
        "2026-08-01T09:30:00Z",
        false,
        None,
        Some("bring the roadmap"),
        None,
        None,
        now(),
    )
    .unwrap();
    assert_eq!(with_notes.description.as_deref(), Some("bring the roadmap"));

    let empty = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "e@test.local",
        "Sprint planning",
        "2026-08-01T09:00:00Z",
        "2026-08-01T09:30:00Z",
        false,
        None,
        Some(""),
        None,
        None,
        now(),
    )
    .unwrap();
    assert!(
        empty.description.is_none(),
        "empty notes add no description"
    );
}

#[test]
fn a_location_becomes_the_location_and_an_empty_one_does_not() {
    // The create is the one write that sets a location from nothing; an edit reshapes it
    // through `build_event_patch`. Empty stays absent, exactly as notes do.
    let with_location = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "loc@test.local",
        "Sprint planning",
        "2026-08-01T09:00:00Z",
        "2026-08-01T09:30:00Z",
        false,
        None,
        None,
        Some("Room 2B"),
        None,
        now(),
    )
    .unwrap();
    assert_eq!(with_location.location.as_deref(), Some("Room 2B"));

    let empty = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "loc-empty@test.local",
        "Sprint planning",
        "2026-08-01T09:00:00Z",
        "2026-08-01T09:30:00Z",
        false,
        None,
        None,
        Some(""),
        None,
        now(),
    )
    .unwrap();
    assert!(empty.location.is_none(), "empty location adds none");
}

#[test]
fn a_retitle_patch_changes_only_the_summary() {
    let (target, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            title: Some("Standup (kort)".to_owned()),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(target, PatchTarget::Series);
    assert_eq!(patch.summary_edit(), Some("Standup (kort)"));
    assert!(patch.start_edit().is_none());
    assert!(patch.end_edit().is_none());
    assert!(!patch.is_significant());
}

#[test]
fn a_move_keeps_the_events_own_zone() {
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-05T10:00:00")),
            end: Some(local("2026-01-05T11:00:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-05T10:00:00")));
    assert_eq!(patch.end_edit(), Some(&amsterdam("2026-01-05T11:00:00")));
    assert!(patch.is_significant());
}

#[test]
fn an_edit_sets_and_clears_notes_and_location() {
    use engine_provider::TextEdit;

    // `Some(text)` sets both.
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            notes: Some("bring the roadmap".to_owned()),
            location: Some("Room 2".to_owned()),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert!(matches!(
        patch.description_edit(),
        Some(TextEdit::Set(s)) if s == "bring the roadmap"
    ));
    assert!(matches!(
        patch.location_edit(),
        Some(TextEdit::Set(s)) if s == "Room 2"
    ));

    // `Some("")` clears, rather than setting an empty value.
    let (_, cleared) = build_event_patch(
        &stored_event(),
        &EventEdit {
            notes: Some(String::new()),
            location: Some(String::new()),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert!(matches!(cleared.description_edit(), Some(TextEdit::Clear)));
    assert!(matches!(cleared.location_edit(), Some(TextEdit::Clear)));

    // `None` leaves both untouched.
    let (_, untouched) = build_event_patch(
        &stored_event(),
        &EventEdit {
            title: Some("x".to_owned()),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert!(untouched.description_edit().is_none());
    assert!(untouched.location_edit().is_none());
}

#[test]
fn editing_one_occurrence_targets_that_instance() {
    let (target, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-26T14:00:00")),
            end: Some(local("2026-01-26T14:30:00")),
            occurrence: Some(local("2026-01-26T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    // Named by its wall clock *and* by that moment as an instant. Google builds its
    // occurrence id from the latter and refuses a timed target without one, so a target
    // carrying only the wall clock fails every per-occurrence edit on a Gmail account.
    // Amsterdam is UTC+1 in January.
    assert_eq!(
        target,
        PatchTarget::Instance(Occurrence::at(
            amsterdam("2026-01-26T09:30:00"),
            UtcDateTime::new(2026, 1, 26, 8, 30, 0).unwrap(),
        ))
    );
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-26T14:00:00")));
    assert_eq!(patch.end_edit(), Some(&amsterdam("2026-01-26T14:30:00")));
}

#[test]
fn an_all_day_occurrence_is_named_without_an_instant() {
    // The one case with no instant to resolve, and none needed: Google addresses an all-day
    // occurrence by date. Resolving one would mean inventing a zone the event does not have.
    let mut event = Event::new(
        EventId::try_from("/cal/vrij.ics").unwrap(),
        Uid::new("vrij@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        CalendarDateTime::Date(CalendarDate::new(2026, 4, 1).unwrap()),
    );
    event.duration = Duration::from_parts(0, 1, 0, 0, 0, 0).unwrap();

    let (target, _) = build_event_patch(
        &event,
        &EventEdit {
            occurrence: Some(local("2026-04-08T00:00:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(
        target,
        PatchTarget::Instance(Occurrence::starting(CalendarDateTime::Date(
            CalendarDate::new(2026, 4, 8).unwrap()
        )))
    );
}

#[test]
fn an_all_day_event_stays_all_day() {
    let mut event = Event::new(
        EventId::try_from("/cal/vrij.ics").unwrap(),
        Uid::new("vrij@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        CalendarDateTime::Date(CalendarDate::new(2026, 4, 1).unwrap()),
    );
    event.title = "Vrij".to_owned();
    event.duration = Duration::from_parts(0, 1, 0, 0, 0, 0).unwrap();

    let (_, patch) = build_event_patch(
        &event,
        &EventEdit {
            start: Some(local("2026-04-08T00:00:00")),
            end: Some(local("2026-04-09T00:00:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(
        patch.start_edit(),
        Some(&CalendarDateTime::Date(
            CalendarDate::new(2026, 4, 8).unwrap()
        ))
    );
    assert_eq!(
        patch.end_edit(),
        Some(&CalendarDateTime::Date(
            CalendarDate::new(2026, 4, 9).unwrap()
        ))
    );
}

#[test]
fn an_inverted_edit_passes_through_as_intent_for_the_adapter_to_refuse() {
    // The builder deliberately does *not* re-validate the interval. Dragging the start
    // past the unchanged end is an inversion this builder cannot even see (it has no
    // stored end), so the guard lives in the engine's patcher, which validates against
    // the event's *effective* end and refuses it there (proven live in `live_jmap`).
    // A half-guard here would only catch the both-present case and give false comfort.
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-05T23:00:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .expect("the builder emits the intent without judging the interval");
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-05T23:00:00")));
    assert!(patch.end_edit().is_none());
}
