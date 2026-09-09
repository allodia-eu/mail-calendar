//! Meeting tests for the calendar write builders.

use engine_api::{CalendarAddress, Invitee, InviteePatch, InviteeRole, SchedulingIdentity};
use engine_core::{
    calendar::Event,
    ids::{CalendarId, EventId, Uid},
    mail::EmailAddress,
    membership::Memberships,
    time::{CalendarDateTime, Duration, LocalDateTime},
};

use super::*;

fn address(value: &str) -> CalendarAddress {
    CalendarAddress::parse(value).unwrap()
}

fn required(email: &str, name: &str) -> Invitee {
    Invitee::required(SchedulingIdentity::named(address(email), name))
}

fn draft() -> EventDraft {
    build_event_draft(
        CalendarId::try_from("cal").unwrap(),
        "planning@test.local",
        "Planning",
        "2026-09-14T09:00:00Z",
        "2026-09-14T10:00:00Z",
        false,
        None,
        None,
        None,
        None,
        UtcDateTime::new(2026, 9, 8, 10, 0, 0).unwrap(),
    )
    .unwrap()
}

fn stored_event() -> Event {
    let mut event = Event::new(
        EventId::try_from("planning.ics").unwrap(),
        Uid::new("planning@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("cal").unwrap()),
        CalendarDateTime::utc(LocalDateTime::new(2026, 9, 14, 9, 0, 0).unwrap()),
    );
    event.duration = Duration::from_parts(0, 0, 0, 1, 0, 0).unwrap();
    event
}

fn now() -> UtcDateTime {
    UtcDateTime::new(2026, 9, 8, 10, 0, 0).unwrap()
}

#[test]
fn a_meeting_uses_the_core_selected_organiser() {
    let organiser = EmailAddress::named("Ada", "ada@example.com");
    let event = attach_meeting(
        draft(),
        &organiser,
        vec![required("Grace@Example.com", "Grace")],
    )
    .unwrap();

    let meeting = event.meeting.expect("the event is a meeting");
    assert_eq!(meeting.organiser().address().as_str(), "ada@example.com");
    assert_eq!(meeting.organiser().name(), Some("Ada"));
    assert_eq!(
        meeting.invitees()[0].identity().address().as_str(),
        "Grace@example.com"
    );
    assert_eq!(meeting.invitees()[0].role(), InviteeRole::Required);
}

#[test]
fn a_meeting_refuses_the_organiser_as_an_invitee() {
    let organiser = EmailAddress::new("ada@example.com");
    let result = attach_meeting(
        draft(),
        &organiser,
        vec![required("ADA@example.com", "Ada")],
    );

    assert!(result.is_err());
}

#[test]
fn an_event_patch_carries_only_the_invitee_changes() {
    let changes = InviteePatch::new()
        .upsert(required("new@example.com", "New"))
        .remove(address("old@example.com"));
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            invitees: Some(changes),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();

    let changes = patch.invitee_edit().expect("the roster changes");
    assert_eq!(
        changes.upserts()[0].identity().address().as_str(),
        "new@example.com"
    );
    assert_eq!(changes.removals()[0].as_str(), "old@example.com");
    assert!(patch.summary_edit().is_none());
}

#[test]
fn an_empty_invitee_patch_changes_nothing() {
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            invitees: Some(InviteePatch::new()),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();

    assert!(patch.invitee_edit().is_none());
    assert!(patch.is_empty());
}

#[test]
fn one_occurrence_cannot_have_a_different_roster() {
    let result = build_event_patch(
        &stored_event(),
        &EventEdit {
            invitees: Some(InviteePatch::new().upsert(required("new@example.com", "New"))),
            occurrence: Some(LocalDateTime::new(2026, 9, 14, 9, 0, 0).unwrap()),
            ..EventEdit::default()
        },
        now(),
    );

    assert!(result.is_err());
}
