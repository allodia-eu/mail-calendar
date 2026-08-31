//! Shared fixtures for the invitation-rule tests.
//!
//! Both halves: the card rules ([`crate::invitations_tests`]) and the diary rules
//! ([`crate::invitations_conflict_tests`]); build the same minimal `Event`, so the builder lives
//! here rather than being copied.

use engine_api::{
    CalendarDateTime, CalendarId, Event, EventId, LocalDateTime, Memberships, Participant,
    ParticipantRole, ParticipationStatus, Uid, UtcDateTime,
};

/// A minimal event carrying `participants`, enough for every rule under test.
pub(crate) fn event_with(participants: Vec<Participant>) -> Event {
    let mut event = Event::new(
        EventId::try_from("/cal/e.ics").unwrap(),
        Uid::new("meeting-1@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        CalendarDateTime::utc(LocalDateTime::new(2026, 7, 30, 14, 30, 0).unwrap()),
    );
    event.duration = "PT1H".parse().unwrap();
    event.participants = participants;
    event
}

pub(crate) fn attendee(email: &str, status: ParticipationStatus) -> Participant {
    let mut participant = Participant::attendee(email);
    participant.participation_status = status;
    participant
}

pub(crate) fn organizer(name: &str, email: &str) -> Participant {
    let mut participant = Participant::attendee(email);
    participant.name = Some(name.to_owned());
    participant.roles.insert(ParticipantRole::Owner);
    participant.participation_status = ParticipationStatus::Accepted;
    participant
}

pub(crate) fn instant(text: &str) -> UtcDateTime {
    text.parse().expect("instant")
}
